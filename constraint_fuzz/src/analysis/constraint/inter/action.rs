use color_eyre::eyre::{self, Result};
use std::fmt;

use color_eyre::eyre::bail;

use crate::analysis::constraint::inter::{
    error::GuardParseError,
    loc::SrcLoc,
    tree::{DotId, SharedFuncNodePtr},
};

/// Get the prefix of a line, which is the substring from the start to the first occurrence of ':'.
/// Contains `:` at the end.
pub fn get_prefix(line: &str) -> std::result::Result<&str, GuardParseError> {
    // get position of ':' in the line
    if let Some(pos) = line.find(':') {
        // return the substring from the start to the position of ':'
        Ok(&line[..pos + 1])
    } else {
        Err(GuardParseError::to_prefix_err(eyre::eyre!(
            "Line does not contain a colon: {}",
            line
        )))
    }
}

#[derive(Clone)]
pub enum FuncActionType {
    Call { child_ptr: SharedFuncNodePtr },
    Return,
}

impl FuncActionType {
    const ENT_PREFIX: &'static str = "enter ";
    const RET_PREFIX: &'static str = "return from ";

    pub fn is_call_guard(line: &str) -> bool {
        line.starts_with(Self::ENT_PREFIX)
    }

    pub fn is_return_guard(line: &str) -> bool {
        line.starts_with(Self::RET_PREFIX)
    }

    fn get_func_name_from_line<'a>(line: &'a str, prefix: &'a str) -> Result<&'a str> {
        if !line.starts_with(prefix) {
            bail!("Line does not start with expected prefix: {}", line);
        }

        // extract func_name: get rid of prefix and read until char '('
        let start = prefix.len();
        let end = line.find('(').unwrap_or_else(|| line.len());
        let func_name = &line[start..end];
        Ok(func_name)
    }

    pub fn get_func_name(line: &str) -> Result<&str> {
        if !Self::is_call_guard(line) && !Self::is_return_guard(line) {
            bail!("Line does not match function action type: {}", line);
        }

        if Self::is_call_guard(line) {
            Self::get_func_name_from_line(line, Self::ENT_PREFIX)
        } else {
            Self::get_func_name_from_line(line, Self::RET_PREFIX)
        }
    }
}

#[derive(Clone)]
pub struct FuncAction {
    act_type: FuncActionType,
    func_name: String,
}

impl DotId for FuncAction {
    fn get_dot_id(&self) -> &str {
        match &self.act_type {
            FuncActionType::Call { child_ptr: _ } => "Function Call Action",
            FuncActionType::Return => "Function Return Action",
        }
    }
}

impl fmt::Debug for FuncAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.act_type {
            FuncActionType::Call { child_ptr } => {
                write!(
                    f,
                    "Call({}) -> Child({:?})",
                    self.func_name,
                    child_ptr.borrow()
                )
            }
            FuncActionType::Return => write!(f, "Return({})", self.func_name),
        }
    }
}

impl FuncAction {
    pub fn new(act_type: FuncActionType, func_name: String) -> Self {
        Self {
            act_type,
            func_name,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.func_name
    }

    pub fn is_call(&self) -> bool {
        matches!(self.act_type, FuncActionType::Call { .. })
    }

    pub fn is_return(&self) -> bool {
        matches!(self.act_type, FuncActionType::Return)
    }

    pub fn get_child_ptr(&self) -> Option<SharedFuncNodePtr> {
        if let FuncActionType::Call { child_ptr } = &self.act_type {
            Some(child_ptr.clone())
        } else {
            None
        }
    }
}

// impl ActionTrait for FuncAction {
//     fn from_line(line: &str) -> Result<Self> {
//         let (act_type, pref_len) = FuncActionType::from_line(line)?;

//         let func_name = get_func_namne_from_line(line, &line[0..pref_len])?;
//         Ok(Self {
//             act_type,
//             func_name: func_name.to_owned(),
//         })
//     }
// }

#[derive(Clone)]
enum IntraActionType {
    BrGuard,
    SwitchGuard,
    IndirectGuard,
}

impl IntraActionType {
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "Merge Br Guard:" => Some(IntraActionType::BrGuard),
            "Switch Guard:" => Some(IntraActionType::SwitchGuard),
            "IndirectBr Guard:" => Some(IntraActionType::IndirectGuard),
            _ => None,
        }
    }
}

impl fmt::Debug for IntraActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntraActionType::BrGuard => write!(f, "BrGuard"),
            IntraActionType::SwitchGuard => write!(f, "SwitchGuard"),
            IntraActionType::IndirectGuard => write!(f, "IndirectGuard"),
        }
    }
}

#[derive(Clone)]
pub struct IntraAction {
    intra_type: IntraActionType,
    cond_loc: SrcLoc,
    cond_val: bool,
    dest_loc: SrcLoc,
}

impl DotId for IntraAction {
    fn get_dot_id(&self) -> &str {
        match self.intra_type {
            IntraActionType::BrGuard => "Branch Guard Action",
            IntraActionType::SwitchGuard => "Switch Guard Action",
            IntraActionType::IndirectGuard => "Indirect Branch Guard Action",
        }
    }
}

impl fmt::Debug for IntraAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at {:?} with value {} to {:?}",
            self.intra_type, self.cond_loc, self.cond_val, self.dest_loc
        )
    }
}

impl IntraAction {
    pub fn parse_simple_guard(line: &str) -> std::result::Result<Self, GuardParseError> {
        let prefix = get_prefix(line)?;
        let intra_type = IntraActionType::from_prefix(prefix).ok_or_else(|| {
            GuardParseError::to_prefix_err(eyre::eyre!(
                "Unknown intra action type prefix: {}",
                prefix
            ))
        })?;

        let line_cont = line[prefix.len()..].trim();
        let mut iter = line_cont.split_whitespace();
        let cond_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition location"))?;
        let cond_loc = SrcLoc::from_str(cond_loc_str)?;

        let cond_val_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition value"))?;
        let cond_val = match cond_val_str {
            "1" => true,
            "0" => false,
            _ => {
                return Err(GuardParseError::from(eyre::eyre!(
                    "Unexpected condition value: {}",
                    cond_val_str
                )));
            }
        };
        let dest_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing destination location"))?;
        let dest_loc = SrcLoc::from_str(dest_loc_str)?;

        Ok(Self {
            intra_type,
            cond_loc,
            cond_val,
            dest_loc,
        })
    }

    pub fn from_slice(slice: &str) -> Result<Self> {
        // example: /path/to/file.c:123:45 1 /path/to/dest.c:67:89
        let parts: Vec<&str> = slice.split_whitespace().collect();
        if parts.len() != 3 {
            bail!("Expected 3 parts in intra action, found {}", parts.len());
        }

        let cond_loc = SrcLoc::from_str(parts[0])?;
        let cond_val = match parts[1] {
            "1" => true,
            "0" => false,
            _ => bail!("Unexpected condition value: {}", parts[1]),
        };
        let dest_loc = SrcLoc::from_str(parts[2])?;

        Ok(Self {
            intra_type: IntraActionType::BrGuard, // default type, can be changed later
            cond_loc,
            cond_val,
            dest_loc,
        })
    }
}

#[derive(Clone)]
enum LoopEntryType {
    Hit,
    Exceed,
}

#[derive(Clone)]
enum LoopEndType {
    Out { count: usize },
    NoStart,
}

#[derive(Clone)]
enum LoopActionType {
    LoopEntry {
        count: usize,
        entry_type: LoopEntryType,
    },
    LoopEnd(LoopEndType),
}

#[derive(Clone)]
pub struct LoopAction {
    la_type: LoopActionType,
    header_loc: SrcLoc,
}

impl LoopAction {
    // Loop Entry Prefix
    const HIT_PREFIX: &'static str = "Loop Hit:";
    const EXCEED_PREFIX: &'static str = "Loop Limit Exceed:";

    // Loop End Prefix
    const OUT_PREFIX: &'static str = "Out of Loop:";
    const NO_START_PREFIX: &'static str = "Loop end without loop start:";

    fn parse_loop_cnt(slice: &str) -> Result<usize> {
        const LOOP_CNT_PREFIX: &str = "at count";
        let slice = slice.trim();
        let cnt_slice = &slice[LOOP_CNT_PREFIX.len()..].trim();
        cnt_slice
            .parse::<usize>()
            .map_err(|_| eyre::eyre!("Failed to parse loop count from slice: {}", slice))
    }

    /// Parse content part for header_loc and count.
    fn parse_content_with_count(content_slice: &str) -> Result<(SrcLoc, usize)> {
        let content_slice = content_slice.trim();
        let pos = content_slice.find(char::is_whitespace).ok_or_else(|| {
            eyre::eyre!(
                "Content slice does not contain whitespace: {}",
                content_slice
            )
        })?;
        let loc_part = &content_slice[..pos];
        let header_loc = SrcLoc::from_str(loc_part)?;
        let cnt_part = &content_slice[pos..];
        let count = Self::parse_loop_cnt(cnt_part)?;

        Ok((header_loc, count))
    }

    fn parse_content_wo_count(content_slice: &str) -> Result<SrcLoc> {
        let content_slice = content_slice.trim();
        if content_slice.is_empty() {
            bail!("Content slice is empty, cannot parse header location");
        }
        SrcLoc::from_str(content_slice)
    }

    pub fn parse_loop_guard(line: &str) -> std::result::Result<Self, GuardParseError> {
        let prefix = get_prefix(line)?;

        if prefix.starts_with(Self::HIT_PREFIX) {
            let (header_loc, count) = Self::parse_content_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEntry {
                    count,
                    entry_type: LoopEntryType::Hit,
                },
                header_loc,
            });
        } else if prefix.starts_with(Self::EXCEED_PREFIX) {
            let (header_loc, count) = Self::parse_content_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEntry {
                    count,
                    entry_type: LoopEntryType::Exceed,
                },
                header_loc,
            });
        } else if prefix.starts_with(Self::OUT_PREFIX) {
            let (header_loc, count) = Self::parse_content_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEnd(LoopEndType::Out { count }),
                header_loc,
            });
        } else if prefix.starts_with(Self::NO_START_PREFIX) {
            let header_loc = Self::parse_content_wo_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEnd(LoopEndType::NoStart),
                header_loc,
            });
        }

        Err(GuardParseError::to_prefix_err(eyre::eyre!(
            "Line does not match any known loop action type: {}",
            line
        )))
    }

    pub fn get_header_loc(&self) -> &SrcLoc {
        &self.header_loc
    }

    pub fn get_count(&self) -> Option<usize> {
        match self.la_type {
            LoopActionType::LoopEntry {
                count,
                entry_type: _,
            } => Some(count),
            LoopActionType::LoopEnd(LoopEndType::Out { count }) => Some(count),
            _ => None,
        }
    }

    pub fn get_type_name(&self) -> &'static str {
        match &self.la_type {
            LoopActionType::LoopEntry {
                count: _,
                entry_type,
            } => match entry_type {
                LoopEntryType::Exceed => "LoopEntryExceed",
                LoopEntryType::Hit => "LoopEntryHit",
            },

            LoopActionType::LoopEnd(end_type) => match end_type {
                LoopEndType::NoStart => "LoopEndNoStart",
                LoopEndType::Out { .. } => "LoopEndOut",
            },
        }
    }
}

impl fmt::Debug for LoopAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count_op = self.get_count();
        match count_op {
            Some(count) => {
                write!(
                    f,
                    "{}(header_loc: {:?}, count: {})",
                    self.get_type_name(),
                    self.get_header_loc(),
                    count
                )
            }
            None => {
                write!(
                    f,
                    "{}(header_loc: {:?})",
                    self.get_type_name(),
                    self.get_header_loc()
                )
            }
        }
    }
}

#[derive(Clone)]
pub enum ExecAction {
    Func(FuncAction),
    Intra(IntraAction),
    Loop(LoopAction),
}

impl ExecAction {
    pub fn is_func_call(&self) -> bool {
        match self {
            ExecAction::Func(func_act) => func_act.is_call(),
            ExecAction::Intra(_) => false,
            ExecAction::Loop(_) => false,
        }
    }

    pub fn get_func_call_act(&self) -> Option<&FuncAction> {
        if let ExecAction::Func(func_act) = self {
            Some(func_act)
        } else {
            None
        }
    }
}

impl DotId for ExecAction {
    fn get_dot_id(&self) -> &str {
        match self {
            ExecAction::Func(func_act) => func_act.get_dot_id(),
            ExecAction::Intra(intra_act) => intra_act.get_dot_id(),
            ExecAction::Loop(_) => "Loop Action",
        }
    }
}

impl fmt::Debug for ExecAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecAction::Func(func_act) => write!(f, "FuncAction: {:?}", func_act),
            ExecAction::Intra(intra_act) => write!(f, "IntraAction: {:?}", intra_act),
            ExecAction::Loop(loop_act) => write!(f, "LoopAction: {:?}", loop_act),
        }
    }
}
