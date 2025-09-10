use crate::analysis::constraint::intra::func_src_tree::{
    builder::{FuncSrcForest, SrcForestBuilder},
    code_query::CodeQLRunner,
};

pub mod code_query;
pub mod stmts;

pub mod builder;
pub mod nodes;

use color_eyre::eyre::Result;

pub fn build_func_src_forest() -> Result<FuncSrcForest> {
    let runner = CodeQLRunner::new();
    let builder = SrcForestBuilder::from_codeql_runner(&runner)?;
    builder.build_forest()
}
