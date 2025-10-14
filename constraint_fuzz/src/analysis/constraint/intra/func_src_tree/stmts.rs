use color_eyre::eyre::Result;
use my_macros::EquivByLoc;
use serde::Serialize;
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use eyre::bail;

use crate::{
    analysis::constraint::{
        inter::loc::{SrcLocEnum, ValidSrcLoc},
        intra::func_src_tree::code_query::{
            for_query::{ForCondMap, ForInitMap, ForRecord, ForUpdateMap},
            if_query::{ElseRecMap, IfRecord},
            while_query::WhileRecord,
        },
    },
    config,
    deopt::Deopt,
};
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct QLLoc {
    pub file_path: PathBuf,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl QLLoc {
    /**
     * File IO related
     *  */

    pub fn loc_line_match(inner_loc: &Option<&SrcLocEnum>, line_num: usize) -> bool {
        match inner_loc {
            None => false,
            Some(src_loc) => match src_loc {
                SrcLocEnum::NullLoc => false,
                SrcLocEnum::Valid(valid_loc) => valid_loc.line == line_num,
            },
        }
    }

    /// transform inner loc to relative loc in the newly returned string
    /// TODO: to be tested, mainly focused on relative index construction
    fn get_content_impl(&self, inner_loc: Option<&SrcLocEnum>) -> Result<(String, Option<usize>)> {
        let file = File::open(&self.file_path)?;
        let reader = BufReader::new(file);

        let mut idx_op: Option<usize> = None;

        let mut content = String::new();
        for (idx, line) in reader.lines().enumerate() {
            let line_num = idx + 1;
            let line = line?;
            if line_num < self.start_line {
                continue;
            }
            if line_num > self.end_line {
                break;
            }

            if Self::loc_line_match(&inner_loc, line_num) {
                let pre_off = content.len();
                let cur_off = if line_num == self.start_line {
                    inner_loc.unwrap().get_col().unwrap() - self.start_column
                } else {
                    inner_loc.unwrap().get_col().unwrap() - 1
                };
                idx_op = Some(pre_off + cur_off);
            }

            if line_num == self.start_line && line_num == self.end_line {
                // start line and end line are the same
                // end inclusive
                let snippet = &line[self.start_column - 1..self.end_column];

                content.push_str(snippet);
            } else if line_num == self.start_line {
                // start line only
                let snippet = &line[self.start_column - 1..];
                content.push_str(snippet);
                content.push('\n');
            } else if line_num == self.end_line {
                // end line only
                // end inclusive
                let snippet = &line[..self.end_column];
                content.push_str(snippet);
            } else {
                // inner line
                content.push_str(&line);
                content.push('\n');
            }
        }
        Ok((content, idx_op))
    }

    pub fn get_content_with_loc_conversion(
        &self,
        inner_loc: Option<&SrcLocEnum>,
    ) -> Result<(String, Option<usize>)> {
        self.get_content_impl(inner_loc)
    }

    pub fn get_content(&self) -> Result<String> {
        let (content, _) = self.get_content_impl(None)?;
        Ok(content)
    }
}

impl QLLoc {
    /**
     * SrcLoc comparison related
     */

    pub fn start_match(&self, src_loc: &ValidSrcLoc) -> bool {
        self.file_path == src_loc.file_path
            && self.start_line == src_loc.line
            && self.start_column == src_loc.col
    }

    pub fn compare_line_and_col(&self, line: usize, col: usize) -> Ordering {
        if self.start_line > line || (self.start_line == line && self.start_column > col) {
            return Ordering::Greater;
        }
        if self.end_line < line || (self.end_line == line && self.end_column < col) {
            return Ordering::Less;
        }

        Ordering::Equal
    }

    /// compare with src loc that are in the same file_path
    pub fn compare_src_loc(&self, src_loc: &SrcLocEnum) -> Option<Ordering> {
        match src_loc {
            SrcLocEnum::NullLoc => {
                log::warn!("Compare QLLoc with an invalid SrcLoc");
                None
            }
            SrcLocEnum::Valid(valid_loc) => match self.file_path.cmp(&valid_loc.file_path) {
                Ordering::Equal => {
                    if self.start_line > valid_loc.line
                        || (self.start_line == valid_loc.line && self.start_column > valid_loc.col)
                    {
                        return Some(Ordering::Greater);
                    }
                    if self.end_line < valid_loc.line
                        || (self.end_line == valid_loc.line && self.end_column < valid_loc.col)
                    {
                        return Some(Ordering::Less);
                    }

                    Some(Ordering::Equal)
                }
                ord => {
                    log::warn!(
                        "Comparing QL ans Src locations from different files: {} and {}",
                        self.file_path.display(),
                        valid_loc.file_path.display()
                    );
                    Some(ord)
                }
            },
        }
    }
}

pub enum LocParseError {
    FormatErr(String),
    ValueErr(String),
    ZeroErr,
}

pub enum LocTypeParseError {
    FormatErr(String),
    ValueErr(String),
}

impl From<LocParseError> for LocTypeParseError {
    fn from(err: LocParseError) -> Self {
        match err {
            LocParseError::FormatErr(msg) => LocTypeParseError::FormatErr(msg),
            LocParseError::ValueErr(msg) => LocTypeParseError::ValueErr(msg),
            LocParseError::ZeroErr => {
                LocTypeParseError::ValueErr("Location values cannot be zero".to_string())
            }
        }
    }
}

impl PartialOrd for QLLoc {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.compare(other))
    }
}

impl Ord for QLLoc {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.compare(other)
    }
}

impl QLLoc {
    pub fn end_before(&self, other: &QLLoc) -> bool {
        self.end_line < other.start_line
            || (self.end_line == other.start_line && self.end_column < other.start_column)
    }

    pub fn start_after(&self, other: &QLLoc) -> bool {
        self.start_line > other.end_line
            || (self.start_line == other.end_line && self.start_column > other.end_column)
    }

    pub fn start_before(&self, other: &QLLoc) -> bool {
        self.start_line < other.start_line
            || (self.start_line == other.start_line && self.start_column < other.start_column)
    }

    pub fn end_after(&self, other: &QLLoc) -> bool {
        self.end_line > other.end_line
            || (self.end_line == other.end_line && self.end_column > other.end_column)
    }

    pub fn contains(&self, other: &QLLoc) -> bool {
        self.start_before(other) && self.end_after(other)
    }

    pub fn compare(&self, other: &QLLoc) -> std::cmp::Ordering {
        match self.file_path.cmp(&other.file_path) {
            std::cmp::Ordering::Equal => match self.start_line.cmp(&other.start_line) {
                std::cmp::Ordering::Equal => match self.start_column.cmp(&other.start_column) {
                    std::cmp::Ordering::Equal => match self.end_line.cmp(&other.end_line) {
                        std::cmp::Ordering::Equal => self.end_column.cmp(&other.end_column),
                        ord => ord,
                    },
                    ord => ord,
                },
                ord => ord,
            },
            ord => ord,
        }
    }

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
        if loc_str == "file://:0:0:0:0" {
            return Err(LocParseError::ZeroErr);
        }
        let loc_str = &loc_str[LOC_PREFIX.len()..];
        let parts: Vec<&str> = loc_str.split(':').collect();
        if parts.len() != 5 {
            return Err(LocParseError::FormatErr(format!(
                "Location string does not have 5 parts separated by ':': {}",
                loc_str
            )));
        }

        let file_path_str = parts[0];

        // check validity of file path
        let deopt = Deopt::new(config::get_library_name()).unwrap();

        let proj_name = deopt.project_name;
        if !file_path_str.contains(&proj_name) {
            return Err(LocParseError::ValueErr(format!(
                "File path does not contain project name '{}': {}",
                proj_name, file_path_str
            )));
        }

        if let Some(ignore_dirs) = &deopt.config.ignore_dir {
            for ignore_dir in ignore_dirs {
                if file_path_str.contains(ignore_dir) {
                    return Err(LocParseError::ValueErr(format!(
                        "File path is in ignored directory '{}': {}",
                        ignore_dir, file_path_str
                    )));
                }
            }
        }

        let file_path = PathBuf::from(file_path_str);
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

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum StmtType {
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

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum BlockType {
    If,
    Else,
    Switch,
    For,
    While,
    Do,
    Function,
    Scoped,
}

impl BlockType {
    pub fn from_str(type_str: &str) -> Result<Self> {
        match type_str {
            "IfBlock" => Ok(BlockType::If),
            "ElseBlock" => Ok(BlockType::Else),
            "SwitchBlock" => Ok(BlockType::Switch),
            "ForBlock" => Ok(BlockType::For),
            "WhileBlock" => Ok(BlockType::While),
            "DoBlock" => Ok(BlockType::Do),
            "FunctionBlock" => Ok(BlockType::Function),
            "ScopedBlock" => Ok(BlockType::Scoped),
            _ => bail!("Unknown block type: {}", type_str),
        }
    }
}

#[derive(EquivByLoc, Debug)]
pub struct ChildEntry {
    pub loc: QLLoc,
    pub stmt_type: StmtType,
}

impl PartialOrd for ChildEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.loc.cmp(&other.loc))
    }
}

impl Ord for ChildEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.loc.cmp(&other.loc)
    }
}

impl ChildEntry {
    pub fn from_loc_and_type(
        loc_str: &str,
        type_str: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc_str)?;
        let stmt_type = StmtType::from_str(type_str);
        Ok(Self { loc, stmt_type })
    }

    pub fn from_block_stmt(block: &BlockStmt) -> Self {
        Self {
            loc: block.loc.clone(),
            stmt_type: StmtType::Block,
        }
    }
}

/// data stmt
#[derive(EquivByLoc, Debug)]
pub struct BlockStmt {
    pub loc: QLLoc,
    pub block_type: BlockType,
}

impl Borrow<QLLoc> for BlockStmt {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

impl BlockStmt {
    pub fn from_loc_and_type(
        loc_str: &str,
        type_str: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc_str)?;
        let block_type =
            BlockType::from_str(type_str).map_err(|e| LocParseError::FormatErr(e.to_string()))?;
        Ok(Self { loc, block_type })
    }

    pub fn is_function_block(&self) -> bool {
        matches!(self.block_type, BlockType::Function)
    }
}

#[derive(PartialEq, Eq, Hash)]
pub enum IfType {
    If,
    IfElse,
}

impl IfType {
    pub fn from_str(type_str: &str) -> Result<Self> {
        match type_str {
            "If" => Ok(IfType::If),
            "If-Else" => Ok(IfType::IfElse),
            _ => bail!("Unknown if type: {}", type_str),
        }
    }
}

/// Struct stmt
#[derive(EquivByLoc)]
pub struct IfStmt {
    pub loc: QLLoc,
    pub if_type: IfType,
    pub cond_loc: QLLoc,
    pub then_entry: ChildEntry,
    pub else_entry: Option<ChildEntry>,
}

impl Borrow<QLLoc> for IfStmt {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

impl IfStmt {
    pub fn from_if_else_record(
        if_record: IfRecord,
        else_map: &ElseRecMap,
    ) -> std::result::Result<Self, LocTypeParseError> {
        let loc = QLLoc::from_str(&if_record.loc)?;
        let if_type_res = IfType::from_str(&if_record.if_type);
        let if_type = match if_type_res {
            Ok(t) => t,
            Err(e) => {
                return Err(LocTypeParseError::FormatErr(format!(
                    "Failed to parse if_type: {}",
                    e
                )));
            }
        };
        let condition_loc = QLLoc::from_str(&if_record.condition_loc)?;
        let then_entry =
            ChildEntry::from_loc_and_type(&if_record.then_stmt_loc, &if_record.then_stmt_type)?;

        let else_entry = if let IfType::IfElse = if_type {
            if let Some(else_record) = else_map.get(&if_record.loc) {
                Some(ChildEntry::from_loc_and_type(
                    &else_record.else_stmt_loc,
                    &else_record.else_stmt_type,
                )?)
            } else {
                return Err(LocTypeParseError::ValueErr(format!(
                    "If-Else statement at {} does not have a corresponding ElseRecord",
                    if_record.loc
                )));
            }
        } else {
            None
        };

        Ok(Self {
            loc,
            if_type,
            cond_loc: condition_loc,
            then_entry,
            else_entry,
        })
    }
}

/// data stmt
#[derive(EquivByLoc)]
pub struct SwitchStmt {
    pub loc: QLLoc,
    pub expr_loc: QLLoc,
}

impl Borrow<QLLoc> for SwitchStmt {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

impl SwitchStmt {
    pub fn from_loc_and_expr(
        loc_str: &str,
        expr_loc_str: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc_str)?;
        let expr_loc = QLLoc::from_str(expr_loc_str)?;
        Ok(Self { loc, expr_loc })
    }
}

#[derive(Clone)]
pub enum WhileType {
    While,
    Do,
}

impl WhileType {
    pub fn from_str(type_str: &str) -> Result<Self> {
        match type_str {
            "While" => Ok(WhileType::While),
            "Do" => Ok(WhileType::Do),
            _ => bail!("Unknown while type: {}", type_str),
        }
    }
}

#[derive(EquivByLoc)]
pub struct WhileStmt {
    pub loc: QLLoc,
    pub while_type: WhileType,
    pub cond_loc: QLLoc,
    pub body_entry: ChildEntry,
}

impl Borrow<QLLoc> for WhileStmt {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

impl WhileStmt {
    pub fn from_record(record: &WhileRecord) -> std::result::Result<Self, LocTypeParseError> {
        let loc = QLLoc::from_str(&record.loc)?;
        let while_type = WhileType::from_str(&record.while_type)
            .map_err(|e| LocParseError::FormatErr(e.to_string()))?;
        let cond_loc = QLLoc::from_str(&record.cond_loc)?;
        let body_entry = ChildEntry::from_loc_and_type(&record.body_loc, &record.body_type)?;

        Ok(Self {
            loc,
            while_type,
            cond_loc,
            body_entry,
        })
    }
}

pub enum ForType {
    InitFor,
    NoInitFor,
}

impl ForType {
    pub fn from_str(type_str: &str) -> Result<Self> {
        match type_str {
            "InitFor" => Ok(ForType::InitFor),
            "NoInitFor" => Ok(ForType::NoInitFor),
            _ => bail!("Unknown for type: {}", type_str),
        }
    }
}

#[derive(EquivByLoc)]
pub struct ForStmt {
    pub loc: QLLoc,
    pub init_loc: Option<QLLoc>,
    pub cond_loc: Option<QLLoc>,
    pub update_loc: Option<QLLoc>,
    pub body_entry: ChildEntry,
}

impl Borrow<QLLoc> for ForStmt {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

impl ForStmt {
    pub fn from_for_record_and_maps(
        record: &ForRecord,
        init_map: &ForInitMap,
        cond_map: &ForCondMap,
        update_map: &ForUpdateMap,
    ) -> std::result::Result<Self, LocTypeParseError> {
        let loc = QLLoc::from_str(&record.loc)?;

        let body_entry = ChildEntry::from_loc_and_type(&record.body_loc, &record.body_type)?;

        let init_loc = match init_map.get(&record.loc) {
            Some(init_loc_str) => Some(QLLoc::from_str(init_loc_str)?),
            None => None,
        };

        let cond_loc = match cond_map.get(&record.loc) {
            Some(cond_loc_str) => Some(QLLoc::from_str(cond_loc_str)?),
            None => None,
        };
        let update_loc = match update_map.get(&record.loc) {
            Some(update_loc_str) => Some(QLLoc::from_str(update_loc_str)?),
            None => None,
        };
        Ok(Self {
            loc,
            init_loc,
            cond_loc,
            update_loc,
            body_entry,
        })
    }
}
