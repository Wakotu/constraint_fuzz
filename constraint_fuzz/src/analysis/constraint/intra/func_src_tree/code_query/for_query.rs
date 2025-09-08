use color_eyre::eyre::Result;
use eyre::bail;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FileFuncTable},
    stmts::{ForStmt, LocParseError},
};

const FOR_QUERY_NAME: &str = "for_stmt.ql";
const FOR_INIT_QUERY_NAME: &str = "for_init_expr.ql";
const FOR_COND_QUERY_NAME: &str = "for_cond_expr.ql";
const FOR_UPDATE_QUERY_NAME: &str = "for_update_expr.ql";

#[derive(Deserialize)]
pub struct ForRecord {
    pub loc: String,
    pub body_loc: String,
    pub body_type: String,
    pub func_name: String,
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct ForInitRecord {
    loc: String,
    init_loc: String,
    func_name: String,
    file_path: String,
}

#[derive(Deserialize)]
pub struct ForCondRecord {
    loc: String,
    cond_loc: String,
    func_name: String,
    file_path: String,
}

#[derive(Deserialize)]
pub struct ForUpdateRecord {
    loc: String,
    update_loc: String,
    func_name: String,
    file_path: String,
}
pub type ForSet = HashSet<ForStmt>;
pub type ForPool = FileFuncTable<ForSet>;

pub type ForInitMap = HashMap<String, String>; // Map from for loc to init loc
pub type ForCondMap = HashMap<String, String>; // Map from for loc to cond loc
pub type ForUpdateMap = HashMap<String, String>; // Map from for loc to update loc

impl CodeQLRunner {
    pub fn get_for_pool(&self) -> Result<ForPool> {
        let for_recs: Vec<ForRecord> = self.run_query_and_parse(FOR_QUERY_NAME)?;
        let init_recs: Vec<ForInitRecord> = self.run_query_and_parse(FOR_INIT_QUERY_NAME)?;
        let cond_recs: Vec<ForCondRecord> = self.run_query_and_parse(FOR_COND_QUERY_NAME)?;
        let update_recs: Vec<ForUpdateRecord> = self.run_query_and_parse(FOR_UPDATE_QUERY_NAME)?;

        let mut init_map: ForInitMap = HashMap::new();
        for rec in init_recs.into_iter() {
            init_map.insert(rec.loc, rec.init_loc);
        }

        let mut cond_map: ForCondMap = HashMap::new();
        for rec in cond_recs.into_iter() {
            cond_map.insert(rec.loc, rec.cond_loc);
        }

        let mut update_map: ForUpdateMap = HashMap::new();
        for rec in update_recs.into_iter() {
            update_map.insert(rec.loc, rec.update_loc);
        }

        let mut for_pool: ForPool = FileFuncTable::new();

        for rec in for_recs.into_iter() {
            let for_set = for_pool.get_value_mut(&rec.file_path, &rec.func_name);
            let for_stmt =
                match ForStmt::from_for_record_and_maps(&rec, &init_map, &cond_map, &update_map) {
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
