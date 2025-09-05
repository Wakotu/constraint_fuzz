use color_eyre::eyre::Result;
use eyre::bail;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FileFuncTable},
    stmts::{ForStmt, LocParseError},
};

const FOR_QUERY_NAME: &str = "for_stmt.ql";
const INIT_FOR_QUERY_NAME: &str = "init_for_stmt.ql";

#[derive(Deserialize)]
pub struct ForRecord {
    pub loc: String,
    pub for_type: String,
    pub cond_loc: String,
    pub update_loc: String,
    pub body_loc: String,
    pub body_type: String,
    pub func_name: String,
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct InitForRecord {
    loc: String,
    init_loc: String,
    func_name: String,
    file_path: String,
}

pub type ForSet = HashSet<ForStmt>;
pub type ForPool = FileFuncTable<ForSet>;

pub type InitForMap = HashMap<String, String>; // Map from for loc to init loc

impl CodeQLRunner {
    pub fn get_for_pool(&self) -> Result<ForPool> {
        let for_recs: Vec<ForRecord> = self.run_query_and_parse(FOR_QUERY_NAME)?;
        let init_recs: Vec<InitForRecord> = self.run_query_and_parse(INIT_FOR_QUERY_NAME)?;

        let mut init_map: InitForMap = HashMap::new();
        for rec in init_recs.into_iter() {
            init_map.insert(rec.loc, rec.init_loc);
        }

        let mut for_pool: ForPool = FileFuncTable::new();

        for rec in for_recs.into_iter() {
            let for_set = for_pool.get_value_mut(&rec.file_path, &rec.func_name);
            let for_stmt = match ForStmt::from_for_init_record(&rec, &init_map) {
                Ok(s) => s,
                Err(e) => match e {
                    LocParseError::FormatErr(msg) => {
                        bail!("Error parsing ForStmt record at loc {}: {}", rec.loc, msg);
                    }
                    LocParseError::ValueErr(msg) => {
                        log::warn!(
                            "Warning: Skipping ForStmt record at loc {}: {}",
                            rec.loc,
                            msg
                        );
                        continue;
                    }
                },
            };

            for_set.insert(for_stmt);
        }

        Ok(for_pool)
    }
}
