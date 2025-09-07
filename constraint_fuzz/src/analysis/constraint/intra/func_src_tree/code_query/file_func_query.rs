use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::CodeQLRunner,
    stmts::{LocParseError, QLLoc},
};
use color_eyre::eyre::Result;
use eyre::bail;
use log::Record;
use serde::Deserialize;

const FUNC_QUERY_NAME: &str = "func.ql";

#[derive(Deserialize)]
pub struct FuncRecord {
    func_name: String,
    func_loc: String,
}

/// file_path -> func_name mapping
pub type FuncMap = HashMap<PathBuf, HashSet<String>>;

impl CodeQLRunner {
    pub fn get_func_map(&self) -> Result<FuncMap> {
        let func_records: Vec<FuncRecord> = self.run_query_and_parse(FUNC_QUERY_NAME)?;

        let mut func_map: FuncMap = HashMap::new();
        for rec in func_records.into_iter() {
            let func_loc = match QLLoc::from_str(&rec.func_loc) {
                Ok(loc) => loc,
                Err(e) => match e {
                    LocParseError::ValueErr(msg) => {
                        log::warn!(
                            "Skipping function {} due to loc parse error: {}",
                            rec.func_name,
                            msg
                        );
                        continue;
                    }
                    LocParseError::FormatErr(msg) => {
                        bail!("Function {} has invalid loc format: {}", rec.func_name, msg);
                    }
                },
            };

            let file_path = func_loc.file_path;
            func_map
                .entry(file_path)
                .or_insert_with(HashSet::new)
                .insert(rec.func_name);
        }
        Ok(func_map)
    }
}
