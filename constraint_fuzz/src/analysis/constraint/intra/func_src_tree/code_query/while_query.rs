use color_eyre::eyre::Result;
use eyre::bail;
use std::collections::HashSet;

use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FuncTable},
    stmts::{LocParseError, WhileStmt},
};

const WHILE_QUERY_NAME: &str = "while_stmt.ql";

pub type WhileSet = HashSet<WhileStmt>;
pub type WhilePool = FuncTable<WhileSet>;

#[derive(Deserialize, Debug)]
pub struct WhileRecord {
    pub loc: String,
    pub while_type: String,
    pub cond_loc: String,
    pub body_loc: String,
    pub body_type: String,
    pub func_name: String,
    pub file_path: String,
}

impl CodeQLRunner {
    pub fn get_while_pool(&self) -> Result<WhilePool> {
        let records: Vec<WhileRecord> = self.run_query_and_parse(WHILE_QUERY_NAME)?;

        let mut while_pool: WhilePool = FuncTable::new();
        for record in records.into_iter() {
            let while_set = while_pool.get_value_mut(&record.func_name);
            let while_stmt = match WhileStmt::from_record(&record) {
                Ok(s) => s,
                Err(e) => match e {
                    LocParseError::ValueErr(msg) => {
                        log::warn!("Failed to parse while record: {:?}, err: {}", record, msg);
                        continue;
                    }
                    LocParseError::FormatErr(msg) => {
                        bail!("Failed to parse while record: {:?}, err: {}", record, msg);
                    }
                },
            };
            while_set.insert(while_stmt);
        }
        Ok(while_pool)
    }
}
