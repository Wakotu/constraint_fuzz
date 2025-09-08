use std::path::Path;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{
        block_query::{BlockMap, BlockPool},
        file_func_query::FuncMap,
        for_query::{ForPool, ForSet},
        if_query::{IfPool, IfSet},
        switch_query::{SwitchMap, SwitchPool},
        while_query::{WhilePool, WhileSet},
        FileFuncTable,
    },
    nodes::{FuncSrcTree, SharedStmtNodePtr},
    stmts::ChildEntry,
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
    // TODO: initialization method

    // other methods

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

        let builder = SrcTreeBuilder::new(
            &root_entry,
            block_map,
            if_set_op,
            switch_map_op,
            while_set_op,
            for_set_op,
        );

        let root_ptr = builder.create_node_recur()?;
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

struct SrcTreeBuilder<'a> {
    cur_entry: &'a ChildEntry,
    block_map: &'a BlockMap,
    if_set_op: Option<&'a IfSet>,
    switch_map_op: Option<&'a SwitchMap>,
    while_set_op: Option<&'a WhileSet>,
    for_set_op: Option<&'a ForSet>,
}

impl<'a> SrcTreeBuilder<'a> {
    fn new(
        cur_entry: &'a ChildEntry,
        block_map: &'a BlockMap,
        if_set_op: Option<&'a IfSet>,
        switch_map_op: Option<&'a SwitchMap>,
        while_set_op: Option<&'a WhileSet>,
        for_set_op: Option<&'a ForSet>,
    ) -> Self {
        Self {
            cur_entry,
            block_map,
            if_set_op,
            switch_map_op,
            while_set_op,
            for_set_op,
        }
    }

    pub fn create_node_recur(&self) -> Result<SharedStmtNodePtr> {
        // TODO: recursive creation
        todo!()
    }
}
