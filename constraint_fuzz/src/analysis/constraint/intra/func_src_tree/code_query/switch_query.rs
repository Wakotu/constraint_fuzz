use color_eyre::eyre::Result;
use eyre::bail;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FileFuncTable},
    stmts::{ChildEntry, LocParseError, QLLoc, SwitchStmt},
};

const SWITCH_QUERY_NAME: &str = "switch_stmt.ql";

pub type CaseMap = HashMap<QLLoc, HashSet<ChildEntry>>;
pub type SwitchMap = HashMap<SwitchStmt, CaseMap>;
pub type SwitchPool = FileFuncTable<SwitchMap>;

#[derive(Deserialize, Debug)]
pub struct SwitchRecord {
    loc: String,
    expr_loc: String,
    case_expr_loc: String,
    case_stmt_loc: String,
    case_stmt_type: String,
    func_name: String,
    file_path: String,
}

/// switch statement, case expr location, case statement entry
pub type SwitchEntry = (SwitchStmt, QLLoc, ChildEntry);

impl SwitchRecord {
    pub fn to_entry(&self) -> std::result::Result<SwitchEntry, LocParseError> {
        let switch_stmt = SwitchStmt::from_loc_and_expr(&self.loc, &self.expr_loc)?;
        let case_expr_loc = QLLoc::from_str(&self.case_expr_loc)?;
        let child_entry = ChildEntry::from_loc_and_type(&self.case_stmt_loc, &self.case_stmt_type)?;
        Ok((switch_stmt, case_expr_loc, child_entry))
    }
}

impl CodeQLRunner {
    pub fn get_switch_pool(&self) -> Result<SwitchPool> {
        let records: Vec<SwitchRecord> = self.run_query_and_parse(SWITCH_QUERY_NAME)?;

        let mut switch_pool: SwitchPool = FileFuncTable::new();
        for record in records.into_iter() {
            let switch_map = switch_pool.get_value_mut(&record.file_path, &record.func_name);
            let (switch_stmt, case_expr_loc, case_stmt_entry) = match record.to_entry() {
                Ok(e) => e,
                Err(e) => match e {
                    LocParseError::ValueErr(msg) => {
                        log::warn!("Failed to parse switch record: {:?}, err: {}", record, msg);
                        continue;
                    }
                    LocParseError::FormatErr(msg) => {
                        bail!("Failed to parse switch record: {:?}, err: {}", record, msg);
                    }
                },
            };

            switch_map
                .entry(switch_stmt)
                .or_insert_with(HashMap::new)
                .entry(case_expr_loc)
                .or_insert_with(HashSet::new)
                .insert(case_stmt_entry);
        }

        Ok(switch_pool)
    }
}
