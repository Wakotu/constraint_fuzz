use color_eyre::eyre::Result;
use eyre::bail;
use my_macros::EquivByLoc;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FileFuncTable},
    stmts::{IfStmt, LocParseError, QLLoc, StmtType},
};

const IF_QUERY_NAME: &str = "if_query.ql";
const ELSE_QUERY_NAME: &str = "else_query.ql";

#[derive(Deserialize)]
pub struct IfRecord {
    pub loc: String,
    pub if_type: String,
    pub condition_loc: String,
    pub then_stmt_loc: String,
    pub then_stmt_type: String,
    pub function: String,
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct ElseRecord {
    pub loc: String,
    pub else_stmt_loc: String,
    pub else_stmt_type: String,
    pub function: String,
    pub file_path: String,
}

type IfSet = HashSet<IfStmt>;
type IfPool = FileFuncTable<IfSet>;

pub type ElseRecMap = HashMap<String, ElseRecord>; // key is IfRecord.loc

impl CodeQLRunner {
    pub fn get_if_pool(&self) -> Result<IfPool> {
        let if_records: Vec<IfRecord> = self.run_query_and_parse(IF_QUERY_NAME)?;
        let else_records: Vec<ElseRecord> = self.run_query_and_parse(ELSE_QUERY_NAME)?;

        let mut else_map: ElseRecMap = HashMap::new();
        for else_rec in else_records.into_iter() {
            else_map.insert(else_rec.loc.clone(), else_rec);
        }

        let mut if_pool: IfPool = IfPool::new();

        for if_record in if_records.into_iter() {
            let if_set = if_pool.get_value_mut(&if_record.file_path, &if_record.function);
            let if_stmt = match IfStmt::from_if_else_record(if_record, &else_map) {
                Ok(s) => s,
                Err(e) => match e {
                    LocParseError::ValueErr(msg) => {
                        log::warn!("Warning: Skipping IfStmt due to value error: {}", msg);
                        continue;
                    }
                    LocParseError::FormatErr(msg) => {
                        bail!("Error: Failed to parse IfStmt due to format error: {}", msg)
                    }
                },
            };
            if_set.insert(if_stmt);
        }
        Ok(if_pool)
    }
}
