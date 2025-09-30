use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
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
    body_loc: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncInfo {
    pub name: String,
    pub body_loc: QLLoc,
}

impl FuncInfo {
    pub fn compare_line_and_col(&self, line: usize, col: usize) -> std::cmp::Ordering {
        self.body_loc.compare_line_and_col(line, col)
    }
}

impl Hash for FuncInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialOrd for FuncInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.body_loc.cmp(&other.body_loc))
    }
}

impl Ord for FuncInfo {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.body_loc.cmp(&other.body_loc)
    }
}

/// file_path -> func_name mapping
pub type FuncMap = HashMap<PathBuf, HashSet<FuncInfo>>;

pub type FuncInfoTable = HashMap<PathBuf, Vec<FuncInfo>>;
pub type FuncLocMap = HashMap<String, QLLoc>;

impl CodeQLRunner {
    pub fn get_func_info_map(&self) -> Result<(FuncInfoTable, FuncLocMap)> {
        let func_records: Vec<FuncRecord> = self.run_query_and_parse(FUNC_QUERY_NAME)?;

        let mut file_func_map: FuncMap = HashMap::new();
        let mut func_loc_map = HashMap::new();
        for rec in func_records.into_iter() {
            let body_loc = match QLLoc::from_str(&rec.body_loc) {
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
                    LocParseError::ZeroErr => {
                        log::warn!("Skipping function {} due to zero loc value", rec.func_name);
                        continue;
                    }
                    LocParseError::FormatErr(msg) => {
                        bail!("Function {} has invalid loc format: {}", rec.func_name, msg);
                    }
                },
            };

            func_loc_map.insert(rec.func_name.clone(), body_loc.clone());

            let file_path = &body_loc.file_path;
            file_func_map
                .entry(file_path.to_owned())
                .or_insert_with(HashSet::new)
                .insert(FuncInfo {
                    name: rec.func_name,
                    body_loc,
                });
        }
        let mut func_info_table = HashMap::new();
        for (fpath, func_set) in file_func_map.into_iter() {
            func_info_table
                .entry(fpath)
                .or_insert_with(Vec::new)
                .extend(func_set);
        }
        for (_fpath, func_vec) in func_info_table.iter_mut() {
            func_vec.sort();
        }

        Ok((func_info_table, func_loc_map))
    }
}
