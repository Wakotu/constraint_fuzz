use std::{collections::HashMap, path::Path, rc::Rc};

use crate::analysis::constraint::{
    exec_rec::case_map,
    intra::func_src_tree::{
        code_query::{
            block_query::{BlockMap, BlockPool},
            file_func_query::FuncMap,
            for_query::{ForPool, ForSet},
            if_query::{IfPool, IfSet},
            switch_query::{SwitchMap, SwitchPool},
            while_query::{WhilePool, WhileSet},
            CodeQLRunner, FileFuncTable,
        },
        nodes::{FuncSrcTree, SharedStmtNodePtr, StmtNode},
        stmts::{ChildEntry, StmtType},
    },
};
use color_eyre::eyre::Result;
use eyre::bail;

pub struct SrcForestBuilder {
    func_map: FuncMap,
    block_pool: BlockPool,
    if_pool: IfPool,
    switch_pool: SwitchPool,
    while_pool: WhilePool,
    for_pool: ForPool,
}

pub type FuncSrcForest = FileFuncTable<FuncSrcTree>;

impl SrcForestBuilder {
    pub fn from_codeql_runner(runner: &CodeQLRunner) -> Result<Self> {
        let func_map = runner.get_func_map()?;
        let block_pool = runner.get_block_pool()?;
        let if_pool = runner.get_if_pool()?;
        let switch_pool = runner.get_switch_pool()?;
        let while_pool = runner.get_while_pool()?;
        let for_pool = runner.get_for_pool()?;

        Ok(Self {
            func_map,
            block_pool,
            if_pool,
            switch_pool,
            while_pool,
            for_pool,
        })
    }

    // other methods

    pub fn create_node_recur(
        cur_entry: &ChildEntry,
        block_map: &BlockMap,
        if_set_op: Option<&IfSet>,
        switch_map_op: Option<&SwitchMap>,
        while_set_op: Option<&WhileSet>,
        for_set_op: Option<&ForSet>,
        // parent_ptr: SharedStmtNodePtr,
    ) -> Result<SharedStmtNodePtr> {
        match cur_entry.stmt_type {
            StmtType::Block => {
                if let Some((block_stmt, child_set)) = block_map.get_key_val(&cur_entry.loc) {
                    let mut child_ptr_vec = Vec::new();

                    let mut child_entry_vec = child_set.iter().collect::<Vec<_>>();
                    child_entry_vec.sort();
                    for child_entry in child_entry_vec {
                        let child_ptr = Self::create_node_recur(
                            child_entry,
                            block_map,
                            if_set_op,
                            switch_map_op,
                            while_set_op,
                            for_set_op,
                        )?;
                        child_ptr_vec.push(child_ptr);
                    }
                    let cur_ptr = StmtNode::create_block_ptr(block_stmt, child_ptr_vec.clone());
                    // parent ptr setting
                    for (idx, child_ptr) in child_ptr_vec.into_iter().enumerate() {
                        child_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        child_ptr.borrow_mut().parent_idx = Some(idx);
                    }
                    Ok(cur_ptr)
                } else {
                    bail!(
                        "Block statement at {:?} not found in block map",
                        cur_entry.loc
                    );
                }
            }
            StmtType::If => {
                if let Some(if_set) = if_set_op {
                    if let Some(if_stmt) = if_set.get(&cur_entry.loc) {
                        let then_ptr = Self::create_node_recur(
                            &if_stmt.then_entry,
                            block_map,
                            if_set_op,
                            switch_map_op,
                            while_set_op,
                            for_set_op,
                        )?;
                        let else_ptr_op = match &if_stmt.else_entry {
                            Some(else_entry) => Some(Self::create_node_recur(
                                else_entry,
                                block_map,
                                if_set_op,
                                switch_map_op,
                                while_set_op,
                                for_set_op,
                            )?),
                            None => None,
                        };
                        let cur_ptr =
                            StmtNode::create_if_ptr(if_stmt, then_ptr.clone(), else_ptr_op.clone());
                        // parent ptr setting
                        then_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        if let Some(else_ptr) = else_ptr_op {
                            else_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        }
                        Ok(cur_ptr)
                    } else {
                        bail!("If statement at {:?} not found in if set", cur_entry.loc);
                    }
                } else {
                    bail!(
                        "If set is None when processing If statement at {:?}",
                        cur_entry.loc
                    );
                }
            }
            StmtType::Switch => {
                if let Some(switch_map) = switch_map_op {
                    if let Some((switch_stmt, case_map)) = switch_map.get_key_value(&cur_entry.loc)
                    {
                        let mut case_ptr_map = HashMap::new();
                        for (case_loc, case_stmt_set) in case_map {
                            // case ptr vec construction
                            let case_ptr_vec = case_ptr_map
                                .entry(case_loc.clone())
                                .or_insert_with(Vec::new);

                            let mut case_entry_vec = case_stmt_set.iter().collect::<Vec<_>>();
                            case_entry_vec.sort();
                            for case_entry in case_entry_vec {
                                let case_ptr = Self::create_node_recur(
                                    case_entry,
                                    block_map,
                                    if_set_op,
                                    switch_map_op,
                                    while_set_op,
                                    for_set_op,
                                )?;
                                case_ptr_vec.push(case_ptr);
                            }
                        }

                        let cur_ptr =
                            StmtNode::create_switch_ptr(switch_stmt, case_ptr_map.clone());
                        // parent ptr setting
                        for (case_loc, case_ptr_vec) in case_ptr_map.into_iter() {
                            for (idx, case_ptr) in case_ptr_vec.into_iter().enumerate() {
                                case_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                                case_ptr.borrow_mut().parent_idx = Some(idx);
                                case_ptr.borrow_mut().parent_case_loc = Some(case_loc.clone());
                            }
                        }
                        Ok(cur_ptr)
                    } else {
                        bail!(
                            "Switch statement at {:?} not found in switch map",
                            cur_entry.loc
                        );
                    }
                } else {
                    bail!(
                        "Switch map is None when processing Switch statement at {:?}",
                        cur_entry.loc
                    );
                }
            }
            StmtType::While => {
                if let Some(while_set) = while_set_op {
                    if let Some(while_stmt) = while_set.get(&cur_entry.loc) {
                        let body_ptr = Self::create_node_recur(
                            &while_stmt.body_entry,
                            block_map,
                            if_set_op,
                            switch_map_op,
                            while_set_op,
                            for_set_op,
                        )?;
                        let cur_ptr = StmtNode::create_while_ptr(while_stmt, body_ptr.clone());
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        Ok(cur_ptr)
                    } else {
                        bail!(
                            "While statement at {:?} not found in while set",
                            cur_entry.loc
                        );
                    }
                } else {
                    bail!(
                        "While set is None when processing While statement at {:?}",
                        cur_entry.loc
                    );
                }
            }
            StmtType::Do => {
                if let Some(while_set) = while_set_op {
                    if let Some(while_stmt) = while_set.get(&cur_entry.loc) {
                        let body_ptr = Self::create_node_recur(
                            &while_stmt.body_entry,
                            block_map,
                            if_set_op,
                            switch_map_op,
                            while_set_op,
                            for_set_op,
                        )?;
                        let cur_ptr = StmtNode::create_while_ptr(while_stmt, body_ptr.clone());
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        Ok(cur_ptr)
                    } else {
                        bail!(
                            "While statement at {:?} not found in while set",
                            cur_entry.loc
                        );
                    }
                } else {
                    bail!(
                        "While set is None when processing While statement at {:?}",
                        cur_entry.loc
                    );
                }
            }
            StmtType::For => {
                if let Some(for_set) = for_set_op {
                    if let Some(for_stmt) = for_set.get(&cur_entry.loc) {
                        let body_ptr = Self::create_node_recur(
                            &for_stmt.body_entry,
                            block_map,
                            if_set_op,
                            switch_map_op,
                            while_set_op,
                            for_set_op,
                        )?;
                        let cur_ptr = StmtNode::create_for_ptr(for_stmt, body_ptr.clone());
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr = Some(Rc::downgrade(&cur_ptr));
                        Ok(cur_ptr)
                    } else {
                        bail!("For statement at {:?} not found in for set", cur_entry.loc);
                    }
                } else {
                    bail!(
                        "For set is None when processing For statement at {:?}",
                        cur_entry.loc
                    );
                }
            }
            _ => {
                // For Plain Stmt.
                Ok(StmtNode::create_plain_ptr(cur_entry))
            }
        }
    }

    pub fn build_tree(&self, file_path: &Path, func_name: &str) -> Result<Option<FuncSrcTree>> {
        let block_map_op = self.block_pool.get_value(file_path, func_name);
        let if_set_op = self.if_pool.get_value(file_path, func_name);
        let switch_map_op = self.switch_pool.get_value(file_path, func_name);
        let while_set_op = self.while_pool.get_value(file_path, func_name);
        let for_set_op = self.for_pool.get_value(file_path, func_name);

        let block_map = match block_map_op {
            Some(m) => m,
            None => return Ok(None),
        };

        // find root
        let root_entry = match block_map.get_root_entry()? {
            Some(e) => e,
            None => {
                bail!(
                    "Function {} in file {:?} has no root block",
                    func_name,
                    file_path
                );
            }
        };

        let root_ptr = Self::create_node_recur(
            &root_entry,
            block_map,
            if_set_op,
            switch_map_op,
            while_set_op,
            for_set_op,
        )?;
        Ok(Some(FuncSrcTree::new(root_ptr)))
    }

    pub fn build_forest(&self) -> Result<FuncSrcForest> {
        let mut forest = FileFuncTable::new();
        for (file_path, func_names) in &self.func_map {
            for func_name in func_names {
                let tree_op = self.build_tree(file_path, func_name)?;
                let tree = match tree_op {
                    Some(t) => t,
                    None => continue,
                };
                forest.insert(file_path, func_name, tree);
            }
        }
        Ok(forest)
    }
}
