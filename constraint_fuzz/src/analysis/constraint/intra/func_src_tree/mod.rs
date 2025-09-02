use color_eyre::eyre::Result;
use std::path::PathBuf;

use eyre::bail;

pub mod code_query;

#[derive(PartialEq, Eq, Hash)]
struct QLLoc {
    file_path: PathBuf,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

pub enum LocParseError {
    FormatErr(String),
    ValueErr(String),
}

impl QLLoc {
    fn parse_num(num_str: &str, num_name: &str) -> std::result::Result<usize, LocParseError> {
        let num: usize = num_str.parse::<usize>().map_err(|e| {
            LocParseError::FormatErr(format!(
                "Failed to parse {} from string '{}': {}",
                num_name, num_str, e
            ))
        })?;

        if num == 0 {
            return Err(LocParseError::ValueErr(format!(
                "{} must be greater than 0, got {}",
                num_name, num
            )));
        }
        Ok(num)
    }

    pub fn from_str(loc_str: &str) -> std::result::Result<Self, LocParseError> {
        const LOC_PREFIX: &str = "file://";
        assert!(loc_str.starts_with(LOC_PREFIX));
        let loc_str = &loc_str[LOC_PREFIX.len()..];
        let parts: Vec<&str> = loc_str.split(':').collect();
        if parts.len() != 5 {
            return Err(LocParseError::FormatErr(format!(
                "Location string does not have 5 parts separated by ':': {}",
                loc_str
            )));
        }

        let file_path = PathBuf::from(parts[0]);
        // judge exists
        if !file_path.exists() {
            return Err(LocParseError::ValueErr(format!(
                "File path does not exist: {}",
                file_path.display()
            )));
        }

        let start_line = Self::parse_num(parts[1], "start_line")?;
        let start_column = Self::parse_num(parts[2], "start_column")?;
        let end_line = Self::parse_num(parts[3], "end_line")?;
        let end_column = Self::parse_num(parts[4], "end_column")?;

        if start_line > end_line || (start_line == end_line && start_column > end_column) {
            return Err(LocParseError::ValueErr(format!(
                "Start location must be before end location: start=({},{}) end=({}, {})",
                start_line, start_column, end_line, end_column
            )));
        }

        Ok(Self {
            file_path,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }
}

#[derive(PartialEq, Eq, Hash)]
enum StmtType {
    If,
    Switch,
    For,
    While,
    Do,
    Block,
    Decl,
    Expr,
    Return,
    Other,
}

impl StmtType {
    pub fn from_str(type_str: &str) -> Self {
        match type_str {
            "IfStmt" => StmtType::If,
            "SwitchStmt" => StmtType::Switch,
            "ForStmt" => StmtType::For,
            "WhileStmt" => StmtType::While,
            "DoStmt" => StmtType::Do,
            "BlockStmt" => StmtType::Block,
            "DeclStmt" => StmtType::Decl,
            "ExprStmt" => StmtType::Expr,
            "ReturnStmt" => StmtType::Return,
            _ => StmtType::Other,
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub enum BLockType {
    If,
    Else,
    Switch,
    For,
    While,
    Do,
    Function,
    Scoped,
}

impl BLockType {
    pub fn from_str(type_str: &str) -> Result<Self> {
        match type_str {
            "IfBlock" => Ok(BLockType::If),
            "ElseBlock" => Ok(BLockType::Else),
            "SwitchBlock" => Ok(BLockType::Switch),
            "ForBlock" => Ok(BLockType::For),
            "WhileBlock" => Ok(BLockType::While),
            "DoBlock" => Ok(BLockType::Do),
            "FunctionBlock" => Ok(BLockType::Function),
            "ScopedBlock" => Ok(BLockType::Scoped),
            _ => bail!("Unknown block type: {}", type_str),
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct BlockStmt {
    loc: QLLoc,
    block_type: BLockType,
}

impl BlockStmt {
    pub fn from_loc_and_type(
        loc_str: &str,
        type_str: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc_str)?;
        let block_type =
            BLockType::from_str(type_str).map_err(|e| LocParseError::FormatErr(e.to_string()))?;
        Ok(Self { loc, block_type })
    }
}

pub enum StmtNode {
    // to be continued
}
