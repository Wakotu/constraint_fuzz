use std::collections::HashMap;

use color_eyre::eyre::Result;
use eyre::bail;
use my_macros::EquivByLoc;
use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{custom_class_query::VarType, CodeQLRunner},
    stmts::{LocParseError, QLLoc},
};

const FUNC_SCOPE_QUERY: &str = "func_scope.ql";
const STMT_SCOPE_QUERY: &str = "stmt_scope.ql";

type FuncScopeEntry = (String, SrcVar);

#[derive(Deserialize)]
pub struct FuncScopeRec {
    var_name: String,
    var_loc: String,
    var_type_name: String,
    var_type_loc: String,
    func_name: String,
}

impl FuncScopeRec {
    pub fn to_entry(&self) -> std::result::Result<FuncScopeEntry, LocParseError> {
        // constructs the variable instance
        let var = SrcVar::new(
            &self.var_name,
            &self.var_loc,
            &self.var_type_name,
            &self.var_type_loc,
        )?;
        Ok((self.func_name.to_owned(), var))
    }
}

type StmtScopeEntry = (QLLoc, SrcVar);

#[derive(Deserialize)]
pub struct StmtScopeRec {
    var_name: String,
    var_loc: String,
    var_type_name: String,
    var_type_loc: String,
    stmt_loc: String,
}

impl StmtScopeRec {
    pub fn to_entry(&self) -> std::result::Result<StmtScopeEntry, LocParseError> {
        // constructs the variable type instance
        let var = SrcVar::new(
            &self.var_name,
            &self.var_loc,
            &self.var_type_name,
            &self.var_type_loc,
        )?;
        // constructs the statement location instance
        let stmt_loc = QLLoc::from_str(&self.stmt_loc)?;
        Ok((stmt_loc, var))
    }
}

#[derive(Debug, Clone, EquivByLoc)]
pub struct SrcVar {
    pub loc: QLLoc,
    pub name: String,
    pub var_type: VarType,
}

impl SrcVar {
    pub fn new(
        var_name: &str,
        var_loc: &str,
        var_type_name: &str,
        var_type_loc: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(var_loc)?;
        let var_type = VarType::new(var_type_name, var_type_loc)?;
        Ok(Self {
            loc,
            name: var_name.to_owned(),
            var_type,
        })
    }
}

pub type FuncScopeMap = HashMap<String, Vec<SrcVar>>;
pub type StmtScopeMap = HashMap<QLLoc, Vec<SrcVar>>;

impl CodeQLRunner {
    pub fn get_func_scope_map(&self) -> Result<FuncScopeMap> {
        let func_scope_rec_vec: Vec<FuncScopeRec> = self.run_query_and_parse(FUNC_SCOPE_QUERY)?;
        let mut func_scope_map: FuncScopeMap = HashMap::new();

        for rec in func_scope_rec_vec.into_iter() {
            let (func_name, var) = match rec.to_entry() {
                Ok(entry) => entry,
                Err(e) => match e {
                    LocParseError::FormatErr(msg) => {
                        bail!(
                            "Failed to parse location in func_scope query result: {}",
                            msg
                        )
                    }
                    LocParseError::ValueErr(_) | LocParseError::ZeroErr => {
                        continue;
                    }
                },
            };
            func_scope_map.entry(func_name).or_default().push(var);
        }
        Ok(func_scope_map)
    }

    pub fn get_stmt_scope_map(&self) -> Result<StmtScopeMap> {
        let stmt_scope_rec_vec: Vec<StmtScopeRec> = self.run_query_and_parse(STMT_SCOPE_QUERY)?;

        let mut stmt_scope_map: StmtScopeMap = HashMap::new();
        for rec in stmt_scope_rec_vec.into_iter() {
            let (stmt_loc, var_type) = match rec.to_entry() {
                Ok(entry) => entry,
                Err(e) => match e {
                    LocParseError::FormatErr(msg) => {
                        bail!(
                            "Failed to parse location in stmt_scope query result: {}",
                            msg
                        )
                    }
                    LocParseError::ValueErr(_) | LocParseError::ZeroErr => {
                        continue;
                    }
                },
            };
            stmt_scope_map.entry(stmt_loc).or_default().push(var_type);
        }
        Ok(stmt_scope_map)
    }
}
