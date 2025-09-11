use std::{collections::HashMap, path::PathBuf};

use color_eyre::eyre::Result;

use eyre::bail;
use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::CodeQLRunner,
    stmts::{LocParseError, QLLoc},
};

const FUNC_INVOC_QUERY: &str = "func_invoc.ql";

#[derive(Deserialize)]
pub struct FuncInocRecord {
    pub func_name: String,
    pub loc: String,
}

#[derive(Clone, Debug)]
pub struct FuncInvoc {
    pub loc: QLLoc,
    pub func_name: String,
}

impl FuncInvoc {
    pub fn from_rec(rec: FuncInocRecord) -> Result<Option<Self>> {
        let loc = match QLLoc::from_str(&rec.loc) {
            Ok(l) => l,
            Err(e) => match e {
                LocParseError::FormatErr(msg) => {
                    bail!("Failed to parse loc {}: {}", rec.loc, msg)
                }
                LocParseError::ValueErr(msg) => {
                    log::warn!("Skipping loc {} due to value error: {}", rec.loc, msg);
                    return Ok(None);
                }
            },
        };
        Ok(Some(Self {
            loc,
            func_name: rec.func_name.clone(),
        }))
    }

    pub fn get_file_path(&self) -> &PathBuf {
        &self.loc.file_path
    }
}

/// file_path -> sorted func_invoc_list
pub type FuncInvocMap = HashMap<PathBuf, Vec<FuncInvoc>>;

impl CodeQLRunner {
    pub fn get_func_invoc_map(&self) -> Result<FuncInvocMap> {
        let records: Vec<FuncInocRecord> = self.run_query_and_parse(FUNC_INVOC_QUERY)?;

        let mut func_invoc_map: FuncInvocMap = HashMap::new();
        for rec in records.into_iter() {
            let func_invoc = match FuncInvoc::from_rec(rec)? {
                Some(fi) => fi,
                None => continue,
            };

            let file_path = func_invoc.get_file_path().clone();
            func_invoc_map
                .entry(file_path)
                .or_insert_with(Vec::new)
                .push(func_invoc);
        }

        for invoc_vec in func_invoc_map.values_mut() {
            invoc_vec.sort_by(|a, b| a.loc.cmp(&b.loc));
        }

        Ok(func_invoc_map)
    }
}
