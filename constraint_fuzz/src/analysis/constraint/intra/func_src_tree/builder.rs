use std::{collections::HashMap, path::Path, rc::Rc, sync::OnceLock};

use crate::analysis::constraint::{
    exec_rec::case_map,
    intra::func_src_tree::{
        code_query::{
            block_query::{BlockMap, BlockPool},
            file_func_query::{FuncInfo, FuncInfoTable, FuncLocMap, FuncMap},
            for_query::{ForPool, ForSet},
            func_invoc_query::FuncInvocMap,
            if_query::{IfPool, IfSet},
            scope_var_query::{FuncScopeMap, StmtScopeMap},
            switch_query::{SwitchMap, SwitchPool},
            while_query::{WhilePool, WhileSet},
            CodeQLRunner, FuncTable,
        },
        nodes::{FuncSrcTree, SharedStmtNodePtr, StmtNode},
        stmts::{ChildEntry, StmtType},
    },
};
use color_eyre::eyre::Result;
use eyre::bail;

pub struct ProjectInfo {
    pub func_info_table: FuncInfoTable,
    pub func_loc_map: FuncLocMap,
    pub block_pool: BlockPool,
    pub if_pool: IfPool,
    pub switch_pool: SwitchPool,
    pub while_pool: WhilePool,
    pub for_pool: ForPool,
    pub func_invoc_map: FuncInvocMap,
    pub func_scope_map: FuncScopeMap,
    pub stmt_scope_map: StmtScopeMap,
}

impl ProjectInfo {
    pub fn from_codeql_runner(runner: &CodeQLRunner) -> Result<Self> {
        let (func_info_table, func_loc_map) = runner.get_func_info_map()?;
        let block_pool = runner.get_block_pool()?;
        let if_pool = runner.get_if_pool()?;
        let switch_pool = runner.get_switch_pool()?;
        let while_pool = runner.get_while_pool()?;
        let for_pool = runner.get_for_pool()?;
        let func_invoc_map = runner.get_func_invoc_map()?;
        let func_scope_map = runner.get_func_scope_map()?;
        let stmt_scope_map = runner.get_stmt_scope_map()?;

        Ok(Self {
            func_info_table,
            func_loc_map,
            block_pool,
            if_pool,
            switch_pool,
            while_pool,
            for_pool,
            func_invoc_map,
            func_scope_map,
            stmt_scope_map,
        })
    }

    pub fn new() -> Result<Self> {
        let runner = CodeQLRunner::new();
        Self::from_codeql_runner(&runner)
    }
}

static PROJECT_INFO: OnceLock<ProjectInfo> = OnceLock::new();

pub fn get_project_info() -> &'static ProjectInfo {
    PROJECT_INFO.get_or_init(|| ProjectInfo::new().expect("Failed to initialize ProjectInfo"))
}

pub struct SrcForestBuilder<'a> {
    proj_info: &'a ProjectInfo,
}

pub type FuncSrcForest = FuncTable<FuncSrcTree>;

impl<'a> SrcForestBuilder<'a> {
    // other methods
    pub fn new(proj_info: &'a ProjectInfo) -> Self {
        Self { proj_info }
    }

    pub fn create_node_recur(
        cur_entry: &ChildEntry,
        block_map: &BlockMap,
        if_set_op: Option<&IfSet>,
        switch_map_op: Option<&SwitchMap>,
        while_set_op: Option<&WhileSet>,
        for_set_op: Option<&ForSet>,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
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
                            func_invoc_map,
                            stmt_scope_map,
                        )?;
                        child_ptr_vec.push(child_ptr);
                    }
                    let cur_ptr = StmtNode::create_block_ptr(
                        block_stmt,
                        child_ptr_vec.clone(),
                        stmt_scope_map,
                    );
                    // parent ptr setting
                    for (idx, child_ptr) in child_ptr_vec.into_iter().enumerate() {
                        child_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
                        child_ptr.borrow_mut().parent_idx_op = Some(idx);
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
                            func_invoc_map,
                            stmt_scope_map,
                        )?;
                        let else_ptr_op = match &if_stmt.else_entry {
                            Some(else_entry) => Some(Self::create_node_recur(
                                else_entry,
                                block_map,
                                if_set_op,
                                switch_map_op,
                                while_set_op,
                                for_set_op,
                                func_invoc_map,
                                stmt_scope_map,
                            )?),
                            None => None,
                        };
                        let cur_ptr = StmtNode::create_if_ptr(
                            if_stmt,
                            then_ptr.clone(),
                            else_ptr_op.clone(),
                            func_invoc_map,
                            stmt_scope_map,
                        );
                        // parent ptr setting
                        then_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
                        if let Some(else_ptr) = else_ptr_op {
                            else_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
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
                                    func_invoc_map,
                                    stmt_scope_map,
                                )?;
                                case_ptr_vec.push(case_ptr);
                            }
                        }

                        let cur_ptr = StmtNode::create_switch_ptr(
                            switch_stmt,
                            case_ptr_map.clone(),
                            func_invoc_map,
                            stmt_scope_map,
                        );
                        // parent ptr setting
                        for (case_loc, case_ptr_vec) in case_ptr_map.into_iter() {
                            for (idx, case_ptr) in case_ptr_vec.into_iter().enumerate() {
                                case_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
                                case_ptr.borrow_mut().parent_idx_op = Some(idx);
                                case_ptr.borrow_mut().parent_case_loc_op = Some(case_loc.clone());
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
                            func_invoc_map,
                            stmt_scope_map,
                        )?;
                        let cur_ptr = StmtNode::create_while_ptr(
                            while_stmt,
                            body_ptr.clone(),
                            func_invoc_map,
                            stmt_scope_map,
                        );
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
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
                            func_invoc_map,
                            stmt_scope_map,
                        )?;
                        let cur_ptr = StmtNode::create_while_ptr(
                            while_stmt,
                            body_ptr.clone(),
                            func_invoc_map,
                            stmt_scope_map,
                        );
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
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
                            func_invoc_map,
                            stmt_scope_map,
                        )?;
                        let cur_ptr = StmtNode::create_for_ptr(
                            for_stmt,
                            body_ptr.clone(),
                            func_invoc_map,
                            stmt_scope_map,
                        );
                        // parent ptr setting
                        body_ptr.borrow_mut().parent_ptr_op = Some(Rc::downgrade(&cur_ptr));
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
                Ok(StmtNode::create_plain_ptr(
                    cur_entry,
                    func_invoc_map,
                    stmt_scope_map,
                    cur_entry,
                ))
            }
        }
    }

    pub fn build_tree(
        &self,
        file_path: &Path,
        func_info: &FuncInfo,
    ) -> Result<Option<FuncSrcTree>> {
        let func_name = &func_info.name;
        let block_map_op = self.proj_info.block_pool.get_value(func_name);
        let if_set_op = self.proj_info.if_pool.get_value(func_name);
        let switch_map_op = self.proj_info.switch_pool.get_value(func_name);
        let while_set_op = self.proj_info.while_pool.get_value(func_name);
        let for_set_op = self.proj_info.for_pool.get_value(func_name);

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
            &self.proj_info.func_invoc_map,
            &self.proj_info.stmt_scope_map,
        )?;
        Ok(Some(FuncSrcTree::new(
            root_ptr,
            func_name,
            &self.proj_info.func_scope_map,
            func_info.ret_type.clone(),
        )))
    }

    pub fn build_forest(&self) -> Result<FuncSrcForest> {
        let mut forest = FuncTable::new();
        for (file_path, func_invo_vec) in &self.proj_info.func_info_table {
            for func_info in func_invo_vec {
                let tree_op = self.build_tree(file_path, &func_info)?;
                let tree = match tree_op {
                    Some(t) => t,
                    None => continue,
                };
                forest.insert(&func_info.name, tree);
            }
        }
        Ok(forest)
    }
}
