use color_eyre::eyre::{self, Result};
use std::fmt;
use std::path::Path;

use color_eyre::eyre::bail;

use crate::analysis::constraint::inter::{
    error::ActrecParseError,
    exec_tree::{
        action::lock::{LockAction, LoopLockAct, RecurLockAct},
        thread_tree::{incre_dot_counter, DotId, SharedFuncNodePtr, Tid, UBVHit},
    },
    loc::SrcLocEnum,
};

use rollback::RollbackAction;

pub mod lock;
pub mod rollback;

/// Get the prefix of a line, which is the substring from the start to the first occurrence of ':'.
/// Contains `:` at the end.
pub fn get_prefix(line: &str) -> std::result::Result<&str, ActrecParseError> {
    // get position of ':' in the line
    if let Some(pos) = line.find(':') {
        // return the substring from the start to the position of ':'
        Ok(&line[..pos + 1])
    } else {
        Err(ActrecParseError::as_prefix_err(eyre::eyre!(
            "Line does not contain a colon: {}",
            line
        )))
    }
}

#[derive(Clone)]
pub struct FuncCallAction {
    pub func_name: String,
    pub child_ptr: SharedFuncNodePtr,
    pub invoc_loc_op: Option<SrcLocEnum>,
}

#[derive(Clone)]
pub enum FuncAction {
    Call(FuncCallAction),
    Return { func_name: String },
    Unwind { loc: SrcLocEnum },
}

impl FuncAction {
    const ENT_PREFIX: &'static str = "enter ";
    const INVOC_PREFIX: &'static str = "Function Invocation:";
    const RET_PREFIX: &'static str = "return from ";
    const UNWIND_PREFIX: &'static str = "Longjmp Invocation:";

    // pub fn is_call_guard(line: &str) -> bool {
    //     line.starts_with(Self::ENT_PREFIX)
    // }

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

    pub fn get_func_name_from_return_guard(line: &str) -> Result<&str> {
        Self::get_func_name_from_line(line, Self::RET_PREFIX)
    }
}

impl FuncAction {
    pub fn get_dot_id(&self, cnt: usize) -> String {
        match &self {
            FuncAction::Call(_) => format!("Function_Call_Action_{}", cnt),
            FuncAction::Return { func_name: _ } => format!("Function_Return_Action_{}", cnt),
            FuncAction::Unwind { loc: _ } => format!("Function_Unwind_Action_{}", cnt),
        }
    }
}

impl fmt::Debug for FuncAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            FuncAction::Call(func_call_act) => {
                write!(
                    f,
                    "Call({}) -> Child({:?}) at {:?}",
                    func_call_act.func_name,
                    func_call_act.child_ptr.borrow(),
                    func_call_act.invoc_loc_op
                )
            }
            FuncAction::Return { func_name } => write!(f, "Return({})", func_name),
            FuncAction::Unwind { loc } => write!(f, "Unwind({:?})", loc),
        }
    }
}

impl FuncAction {
    pub fn get_func_name(&self) -> &str {
        match self {
            FuncAction::Call(call_act) => &call_act.func_name,
            // actually operation name
            FuncAction::Unwind { loc: _ } => "Unwind",
            FuncAction::Return { func_name } => func_name,
        }
    }

    pub fn is_call(&self) -> bool {
        matches!(self, FuncAction::Call { .. })
    }

    pub fn is_return(&self) -> bool {
        matches!(self, FuncAction::Return { .. })
    }

    pub fn get_child_ptr(&self) -> Option<SharedFuncNodePtr> {
        if let FuncAction::Call(call_act) = self {
            Some(call_act.child_ptr.clone())
        } else {
            None
        }
    }

    pub fn parse_return_guard(line: &str) -> Result<Self> {
        let func_name = FuncAction::get_func_name_from_return_guard(line)?;
        Ok(Self::Return {
            func_name: func_name.to_owned(),
        })
    }

    pub fn parse_unwind_guard(line: &str) -> Result<Self> {
        if !line.starts_with(FuncAction::UNWIND_PREFIX) {
            bail!("Line does not start with unwind prefix: {}", line);
        }

        let invoc_part = &line[FuncAction::UNWIND_PREFIX.len() + 1..];
        let loc_end_pos = invoc_part
            .find(char::is_whitespace)
            .ok_or_else(|| eyre::eyre!("No whitespace found in unwind part: {}", invoc_part))?;
        let loc_str = &invoc_part[..loc_end_pos];
        let loc = SrcLocEnum::from_str(loc_str)?;

        Ok(Self::Unwind { loc })
    }

    /// return invoc_loc extracted and number of characters consumed(including the trailing space)
    fn parse_invoc_part(line: &str) -> Result<(SrcLocEnum, usize)> {
        if !line.starts_with(FuncAction::INVOC_PREFIX) {
            bail!("Line does not start with invocation prefix: {}", line);
        }

        // + 1 for the space after the prefix
        let invoc_part = &line[FuncAction::INVOC_PREFIX.len() + 1..];
        let loc_end_pos = invoc_part
            .find(char::is_whitespace)
            .ok_or_else(|| eyre::eyre!("No whitespace found in invocation part: {}", invoc_part))?;
        let loc_str = &invoc_part[..loc_end_pos];
        let loc = SrcLocEnum::from_str(loc_str)?;
        Ok((loc, loc_end_pos + 1 + FuncAction::INVOC_PREFIX.len() + 1))
    }

    fn parse_entry_part(slice: &str) -> Result<String> {
        let func_name = Self::get_func_name_from_line(slice, Self::ENT_PREFIX)?;
        Ok(func_name.to_owned())
    }

    /// parse a line of call guard to get (invoc_loc_op, func_name)
    pub fn parse_call_guard(
        line: &str,
    ) -> std::result::Result<(Option<SrcLocEnum>, String), ActrecParseError> {
        let (invoc_loc_op, pref_len) = match Self::parse_invoc_part(line) {
            Ok((loc, len)) => (Some(loc), len),
            Err(e) => {
                log::warn!("Failed to parse invocation part: {}", e);
                (None, 0)
            }
        };

        let entry_part = &line[pref_len..];
        let func_name = match Self::parse_entry_part(entry_part) {
            Ok(name) => name,
            Err(e) => {
                log::warn!("Failed to parse entry part: {}", e);
                return Err(ActrecParseError::as_skip_err(e, pref_len));
            }
        };

        Ok((invoc_loc_op, func_name))
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
pub enum JumpActionType {
    Br { val_loc: SrcLocEnum },
    MergeBr,
    Switch,
    IndirectBr,
}

impl JumpActionType {
    pub fn from_simple_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            // Only Merge Br Guard is in simple version
            "Merge Br Guard:" => Some(JumpActionType::MergeBr),
            "Switch Guard:" => Some(JumpActionType::Switch),
            "IndirectBr Guard:" => Some(JumpActionType::IndirectBr),
            _ => None,
        }
    }
}

impl fmt::Debug for JumpActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JumpActionType::Br { val_loc } => {
                write!(f, "BrGuard with value loc {:?}", val_loc)
            }
            JumpActionType::MergeBr => write!(f, "MergeBrGuard"),
            JumpActionType::Switch => write!(f, "SwitchGuard"),
            JumpActionType::IndirectBr => write!(f, "IndirectGuard"),
        }
    }
}

#[derive(Clone)]
pub struct JumpAction {
    pub jump_variants: JumpActionType,
    pub from_loc: SrcLocEnum,
    pub cond_val: bool,
    pub dest_loc: SrcLocEnum,
}

impl JumpAction {
    fn get_dot_id(&self, cnt: usize) -> String {
        match self.jump_variants {
            JumpActionType::Br { .. } => {
                format!("Branch_Guard_Action_{}", cnt)
            }
            JumpActionType::MergeBr => format!("Branch_Merge_Guard_Action_{}", cnt),
            JumpActionType::Switch => format!("Switch_Guard_Action_{}", cnt),
            JumpActionType::IndirectBr => format!("Indirect_Branch_Guard_Action_{}", cnt),
        }
    }
}

impl fmt::Debug for JumpAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at {:?} with value {} to {:?}",
            self.jump_variants, self.from_loc, self.cond_val, self.dest_loc
        )
    }
}

impl JumpAction {
    fn parse_simple_guard(line: &str) -> std::result::Result<Self, ActrecParseError> {
        let prefix = get_prefix(line)?;
        let intra_type = JumpActionType::from_simple_prefix(prefix).ok_or_else(|| {
            ActrecParseError::as_prefix_err(eyre::eyre!(
                "Unknown intra action type prefix: {}",
                prefix
            ))
        })?;

        let line_cont = line[prefix.len()..].trim();
        let mut iter = line_cont.split_whitespace();
        let cond_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition location"))?;
        let cond_loc = SrcLocEnum::from_str(cond_loc_str)?;

        let cond_val_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition value"))?;
        let cond_val = match cond_val_str {
            "1" => true,
            "0" => false,
            _ => {
                return Err(ActrecParseError::from(eyre::eyre!(
                    "Unexpected condition value: {}",
                    cond_val_str
                )));
            }
        };
        let dest_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing destination location"))?;
        let dest_loc = SrcLocEnum::from_str(dest_loc_str)?;

        Ok(Self {
            jump_variants: intra_type,
            from_loc: cond_loc,
            cond_val,
            dest_loc,
        })
    }

    pub fn parse_jump_act_rec(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if let Some(simple_action) = ActrecParseError::to_eyre(Self::parse_simple_guard(line))? {
            return Ok(simple_action);
        }

        const REG_BR_PREFIX: &str = "Br Guard:";
        let prefix = get_prefix(line)?;
        if prefix != REG_BR_PREFIX {
            return Err(ActrecParseError::as_prefix_err(eyre::eyre!(
                "Line does not match regular branch guard prefix: {}",
                line
            )));
        }

        let line_cont = line[prefix.len()..].trim();
        if let Some(idx) = line_cont.find(char::is_whitespace) {
            let value_hit_str = &line_cont[..idx];
            let intra_act_str = line_cont[idx..].trim();

            // parse value hit
            let va_loc = SrcLocEnum::from_str(value_hit_str)?;
            let br_act_type = JumpActionType::Br { val_loc: va_loc };

            // parse intra action
            let br_act = JumpAction::from_slice(intra_act_str, br_act_type)?;

            return Ok(br_act);
        }

        // if line does not match any known action, return error
        Err(ActrecParseError::from(eyre::eyre!(
            "Line does not match any known action format: {}",
            line
        )))
    }

    pub fn from_slice(slice: &str, act_type: JumpActionType) -> Result<Self> {
        // example: /path/to/file.c:123:45 1 /path/to/dest.c:67:89
        let parts: Vec<&str> = slice.split_whitespace().collect();
        if parts.len() != 3 {
            bail!("Expected 3 parts in intra action, found {}", parts.len());
        }

        let cond_loc = SrcLocEnum::from_str(parts[0])?;
        let cond_val = match parts[1] {
            "1" => true,
            "0" => false,
            _ => bail!("Unexpected condition value: {}", parts[1]),
        };
        let dest_loc = SrcLocEnum::from_str(parts[2])?;

        Ok(Self {
            jump_variants: act_type,
            from_loc: cond_loc,
            cond_val,
            dest_loc,
        })
    }
}

#[derive(Clone)]
pub struct ThreadAction {
    pub loc: SrcLocEnum,
    tid: Tid, // using String for simplicity, can be changed to a more appropriate type
}

impl ThreadAction {
    const THREAD_ACTION_PREFIX: &'static str = "Thread Creation:";

    pub fn get_thread_id(&self) -> Tid {
        self.tid
    }

    pub fn parse_thread_act_rec(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if !line.starts_with(Self::THREAD_ACTION_PREFIX) {
            return Err(ActrecParseError::as_prefix_err(eyre::eyre!(
                "Line does not start with 'Thread Creation:': {}",
                line
            )));
        }

        let content = line[Self::THREAD_ACTION_PREFIX.len()..].trim();

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(ActrecParseError::as_parse_err(eyre::eyre!(
                "Expected at least 3 parts in thread guard, found {}: {}",
                parts.len(),
                line
            )));
        }

        let loc = SrcLocEnum::from_str(parts[0])?;
        let tid = parts[1].parse::<Tid>().map_err(|_| {
            ActrecParseError::as_parse_err(eyre::eyre!(
                "Failed to parse thread ID from part: {}",
                parts[2]
            ))
        })?;

        Ok(Self { loc, tid })
    }
}

#[derive(Clone)]
enum LoopEntryType {
    Hit,
    Exceed,
}

#[derive(Clone)]
enum LoopOutType {
    Out { count: usize },
    NoStart,
}

#[derive(Clone)]
pub struct LoopEntryAct {
    count: usize,
    entry_type: LoopEntryType,
}

impl LoopEntryAct {
    pub fn is_exceed(&self) -> bool {
        match &self.entry_type {
            LoopEntryType::Exceed => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct LoopOutAct {
    out_loc: SrcLocEnum,
    end_type: LoopOutType,
}

#[derive(Clone)]
pub enum LoopActionType {
    LoopEntry(LoopEntryAct),
    LoopEnd(LoopOutAct),
}

#[derive(Clone)]
pub struct LoopAction {
    pub la_type: LoopActionType,
    pub header_loc: SrcLocEnum,
}

impl LoopAction {
    pub fn is_nostart_out(&self) -> bool {
        match &self.la_type {
            LoopActionType::LoopEnd(loop_end) => match &loop_end.end_type {
                LoopOutType::NoStart => true,
                _ => false,
            },
            _ => false,
        }
    }
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
    fn parse_header_loc_with_count(content_slice: &str) -> Result<(SrcLocEnum, usize)> {
        let content_slice = content_slice.trim();
        let pos = content_slice.find(char::is_whitespace).ok_or_else(|| {
            eyre::eyre!(
                "Content slice does not contain whitespace: {}",
                content_slice
            )
        })?;
        let loc_part = &content_slice[..pos];
        let header_loc = SrcLocEnum::from_str(loc_part)?;
        let cnt_part = &content_slice[pos..];
        let count = Self::parse_loop_cnt(cnt_part)?;

        Ok((header_loc, count))
    }

    fn parse_header_out_loc_with_count(
        content_slice: &str,
    ) -> Result<(SrcLocEnum, SrcLocEnum, usize)> {
        let content_slice = content_slice.trim();
        let pos = content_slice.find(char::is_whitespace).ok_or_else(|| {
            eyre::eyre!(
                "Content slice does not contain whitespace: {}",
                content_slice
            )
        })?;
        let loc_part = &content_slice[..pos];
        let header_loc = SrcLocEnum::from_str(loc_part)?;

        let content_slice = content_slice[pos..].trim();
        let pos = content_slice.find(char::is_whitespace).ok_or_else(|| {
            eyre::eyre!(
                "Content slice does not contain whitespace: {}",
                content_slice
            )
        })?;
        let loc_part = &content_slice[..pos];
        let out_loc = SrcLocEnum::from_str(loc_part)?;

        let cnt_part = &content_slice[pos..];
        let count = Self::parse_loop_cnt(cnt_part)?;

        Ok((header_loc, out_loc, count))
    }

    fn parse_header_out_loc_wo_count(content_slice: &str) -> Result<(SrcLocEnum, SrcLocEnum)> {
        let content_slice = content_slice.trim();
        // header loc part
        let pos = content_slice.find(char::is_whitespace).ok_or_else(|| {
            eyre::eyre!(
                "Content slice does not contain whitespace: {}",
                content_slice
            )
        })?;
        let loc_part = &content_slice[..pos];
        let header_loc = SrcLocEnum::from_str(loc_part)?;

        // out_loc part
        let content_slice = content_slice[pos..].trim();
        let pos = content_slice
            .find(char::is_whitespace)
            .unwrap_or(content_slice.len());
        let loc_part = &content_slice[..pos];

        let out_loc = SrcLocEnum::from_str(loc_part)?;

        Ok((header_loc, out_loc))
    }

    // fn parse_header_loc_wo_count(content_slice: &str) -> Result<SrcLoc> {
    //     let content_slice = content_slice.trim();
    //     if content_slice.is_empty() {
    //         bail!("Content slice is empty, cannot parse header location");
    //     }
    //     SrcLoc::from_str(content_slice)
    // }

    pub fn parse_loop_act_rec(line: &str) -> std::result::Result<Self, ActrecParseError> {
        let prefix = get_prefix(line)?;

        // Loop Entry Guards
        if prefix.starts_with(Self::HIT_PREFIX) {
            let (header_loc, count) = Self::parse_header_loc_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEntry(LoopEntryAct {
                    count,
                    entry_type: LoopEntryType::Hit,
                }),
                header_loc,
            });
        } else if prefix.starts_with(Self::EXCEED_PREFIX) {
            let (header_loc, count) = Self::parse_header_loc_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEntry(LoopEntryAct {
                    count,
                    entry_type: LoopEntryType::Exceed,
                }),
                header_loc,
            });
        }
        // Loop End Guards
        else if prefix.starts_with(Self::OUT_PREFIX) {
            let (header_loc, out_loc, count) =
                Self::parse_header_out_loc_with_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEnd(LoopOutAct {
                    out_loc,
                    end_type: LoopOutType::Out { count },
                }),
                header_loc,
            });
        } else if prefix.starts_with(Self::NO_START_PREFIX) {
            let (header_loc, out_loc) = Self::parse_header_out_loc_wo_count(&line[prefix.len()..])?;
            return Ok(Self {
                la_type: LoopActionType::LoopEnd(LoopOutAct {
                    out_loc,
                    end_type: LoopOutType::NoStart,
                }),
                header_loc,
            });
        }

        Err(ActrecParseError::as_prefix_err(eyre::eyre!(
            "Line does not match any known loop action type: {}",
            line
        )))
    }

    pub fn get_header_loc(&self) -> &SrcLocEnum {
        &self.header_loc
    }

    pub fn get_out_loc(&self) -> Option<&SrcLocEnum> {
        match &self.la_type {
            LoopActionType::LoopEntry(_) => None,
            LoopActionType::LoopEnd(loop_end) => Some(&loop_end.out_loc),
        }
    }

    pub fn get_count(&self) -> Option<usize> {
        match &self.la_type {
            LoopActionType::LoopEntry(loop_entry) => Some(loop_entry.count),
            LoopActionType::LoopEnd(LoopOutAct {
                out_loc: _,
                end_type: LoopOutType::Out { count },
            }) => Some(*count),
            _ => None,
        }
    }

    pub fn get_type_name(&self) -> &'static str {
        match &self.la_type {
            LoopActionType::LoopEntry(loop_entry) => match &loop_entry.entry_type {
                LoopEntryType::Exceed => "LoopEntryExceed",
                LoopEntryType::Hit => "LoopEntryHit",
            },

            LoopActionType::LoopEnd(loop_end) => match &loop_end.end_type {
                LoopOutType::NoStart => "LoopEndNoStart",
                LoopOutType::Out { .. } => "LoopEndOut",
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
pub struct SelectAction {
    pub loc: SrcLocEnum,
    pub val: bool,
}

impl fmt::Debug for SelectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SelectAction({:?})", self.loc)
    }
}

impl SelectAction {
    pub fn get_loc(&self) -> &SrcLocEnum {
        &self.loc
    }
    pub fn parse_select_act_rec(line: &str) -> std::result::Result<Self, ActrecParseError> {
        const SEL_PREFIX: &str = "Select Guard: ";
        if !line.starts_with(SEL_PREFIX) {
            return Err(ActrecParseError::as_prefix_err(eyre::eyre!(
                "
                Line does not start with select guard prefix: {}",
                line
            )));
        }

        let content = line[SEL_PREFIX.len()..].trim();
        let seg_vec: Vec<&str> = content.split_whitespace().collect();
        let loc = SrcLocEnum::from_str(seg_vec[0])?;

        let val_str = seg_vec[1];
        let val = match val_str {
            "0" => false,
            "1" => true,
            _ => {
                return Err(ActrecParseError::as_parse_err(eyre::eyre!(
                    "Select Action Record parse: invalid val string"
                )))
            }
        };
        Ok(Self { loc, val })
    }
}

#[derive(Clone)]
pub enum ExecAction {
    Func(FuncAction),
    Intra(JumpAction),
    /// Unconditional Branch Value
    UBV(UBVHit),
    Select(SelectAction),
    Loop(LoopAction),
    // Recur(RecurLock),
    Thread(ThreadAction),
    Rollback(RollbackAction),
    Lock(LockAction),
}

impl ExecAction {
    pub fn get_switch_act(&self) -> Option<&JumpAction> {
        match self {
            ExecAction::Intra(jump_act) => match jump_act.jump_variants {
                JumpActionType::Switch => Some(jump_act),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_outer_destloc(&self) -> Result<&SrcLocEnum> {
        let jump_act = match self {
            ExecAction::Intra(jump_act) => jump_act,
            act => bail!(
                "Cond Expr Outer action of If Node should not be of action type {:?}",
                act
            ),
        };

        let dest_loc = match &jump_act.jump_variants {
            JumpActionType::Br { val_loc: _ } => &jump_act.dest_loc,
            JumpActionType::MergeBr => &jump_act.dest_loc,
            jump_var => bail!(
                "Cond Expr Outer action of If Node should not be of action type {:?}",
                jump_var
            ),
        };
        Ok(dest_loc)
    }

    /**
     * Statement-Action match related methods
     */

    pub fn get_loopentry_act(&self) -> Option<&LoopEntryAct> {
        match self {
            Self::Loop(loop_act) => match &loop_act.la_type {
                LoopActionType::LoopEntry(act) => Some(act),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn is_normal_loopend(&self) -> bool {
        match self {
            Self::Loop(loop_act) => match &loop_act.la_type {
                LoopActionType::LoopEnd(end_act) => match &end_act.end_type {
                    LoopOutType::Out { count: _ } => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_loop_entry(&self) -> bool {
        match self {
            Self::Loop(loop_act) => match loop_act.la_type {
                LoopActionType::LoopEntry(_) => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn is_nostart_loopout(&self) -> bool {
        let loop_act = match self {
            Self::Loop(loop_act) => loop_act,
            _ => return false,
        };
        loop_act.is_nostart_out()
    }

    pub fn get_match_loc(&self) -> Option<&SrcLocEnum> {
        match self {
            ExecAction::UBV(ubv_hit) => Some(ubv_hit.get_loc()),
            ExecAction::Select(select_act) => Some(&select_act.loc),
            ExecAction::Func(func_act) => match &func_act {
                FuncAction::Call(call_act) => call_act.invoc_loc_op.as_ref(),
                FuncAction::Return { .. } => None,
                FuncAction::Unwind { loc } => Some(loc),
            },
            ExecAction::Intra(intra_act) => Some(&intra_act.from_loc),
            ExecAction::Loop(loop_act) => Some(&loop_act.header_loc),
            ExecAction::Lock(_) => None,
            ExecAction::Thread(thread_act) => Some(&thread_act.loc),
            ExecAction::Rollback(rb_act) => rb_act.get_loc(),
        }
    }

    pub fn plain_stmt_suitable(&self) -> bool {
        match self {
            ExecAction::UBV(_) => true,
            ExecAction::Select(_) => true,
            ExecAction::Func(func_act) => match func_act {
                FuncAction::Call(_) => true,
                FuncAction::Return { .. } => false,
                FuncAction::Unwind { .. } => false,
            },
            ExecAction::Intra(jump_act) => match jump_act.jump_variants {
                JumpActionType::Br { .. } => true,
                JumpActionType::MergeBr => true,
                JumpActionType::Switch => false,
                JumpActionType::IndirectBr => false,
            },
            ExecAction::Loop(_) => false,
            ExecAction::Thread(_) => true,
            ExecAction::Rollback(rb_act) => rb_act.plain_stmt_suitable(),
            ExecAction::Lock(_) => false,
        }
    }
}

impl ExecAction {
    pub fn is_func_call(&self) -> bool {
        match self {
            ExecAction::Func(func_act) => func_act.is_call(),
            _ => false,
        }
    }

    pub fn get_func_call_act(&self) -> Option<&FuncAction> {
        if let ExecAction::Func(func_act) = self {
            if func_act.is_call() {
                Some(func_act)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl DotId for ExecAction {
    fn get_dot_id(&self) -> String {
        let cnt = incre_dot_counter();
        match self {
            ExecAction::UBV(_) => format!("UBVHit_Action_{}", cnt),
            ExecAction::Select(_) => format!("Select_Action_{}", cnt),
            ExecAction::Func(func_act) => func_act.get_dot_id(cnt),
            ExecAction::Intra(intra_act) => intra_act.get_dot_id(cnt),
            ExecAction::Loop(_) => format!("Loop_Action_{}", cnt),
            ExecAction::Lock(lock_act) => match lock_act {
                LockAction::Recur(RecurLockAct::Locked) => {
                    format!("Recur_Lock_Action_{}", cnt)
                }
                LockAction::Recur(RecurLockAct::Released) => {
                    format!("Recur_Unlock_Action_{}", cnt)
                }
                LockAction::Loop(LoopLockAct::Locked) => {
                    format!("Loop_Lock_Action_{}", cnt)
                }
                LockAction::Loop(LoopLockAct::Released) => {
                    format!("Loop_Unlock_Action_{}", cnt)
                }
            },

            ExecAction::Thread(_) => format!("Thread_Action_{}", cnt),
            ExecAction::Rollback(_) => format!("Rollback_Action_{}", cnt),
        }
    }
}

impl fmt::Debug for ExecAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecAction::UBV(ubv_hit) => write!(f, "UBVHit: {:?}", ubv_hit),
            ExecAction::Select(select_act) => write!(f, "SelectAction: {:?}", select_act),
            ExecAction::Func(func_act) => write!(f, "FuncAction: {:?}", func_act),
            ExecAction::Intra(intra_act) => write!(f, "IntraAction: {:?}", intra_act),
            ExecAction::Loop(loop_act) => write!(f, "LoopAction: {:?}", loop_act),
            ExecAction::Lock(lock_act) => match lock_act {
                LockAction::Loop(loop_act) => match loop_act {
                    LoopLockAct::Locked => write!(f, "LoopAction: Locked"),
                    LoopLockAct::Released => write!(f, "LoopAction: Released"),
                },
                LockAction::Recur(recur_act) => match recur_act {
                    RecurLockAct::Locked => write!(f, "RecurAction: Locked"),
                    RecurLockAct::Released => write!(f, "RecurAction: Released"),
                },
            },
            ExecAction::Thread(thread_act) => {
                write!(
                    f,
                    "ThreadAction: loc: {:?}, tid: {}",
                    thread_act.loc, thread_act.tid
                )
            }
            ExecAction::Rollback(rb_act) => {
                write!(f, "RollbackAction: {:?}", rb_act)
            }
        }
    }
}
