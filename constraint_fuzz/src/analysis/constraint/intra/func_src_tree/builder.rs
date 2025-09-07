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

pub struct SrcTreeBuilder {
    func_map: FuncMap,
    block_pool: BlockPool,
    if_pool: IfPool,
    switch_poo: SwitchPool,
    while_pool: WhilePool,
    for_pool: ForPool,
}

pub type FuncSrcForest = FileFuncTable<FuncSrcTree>;

impl SrcTreeBuilder {
    // TODO: initialization method

    // other methods

    fn create_node(
        cur_entry: &ChildEntry,
        block_map: &BlockMap,
        if_set_op: Option<&IfSet>,
        switch_map_op: Option<&SwitchMap>,
        while_set_op: Option<&WhileSet>,
        for_set_op: Option<&ForSet>,
    ) -> Result<SharedStmtNodePtr> {
        todo!()
    }

    pub fn build_tree(&self, file_path: &Path, func_name: &str) -> Result<Option<FuncSrcTree>> {
        let block_map_op = self.block_pool.get_value(file_path, func_name);
        let if_set_op = self.if_pool.get_value(file_path, func_name);
        let switch_map_op = self.switch_poo.get_value(file_path, func_name);
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

        todo!()
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
