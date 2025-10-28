use eyre::eyre;

use crate::analysis::constraint::inter::{error::ActrecParseError, loc::SrcLocEnum};

#[derive(Clone, Debug)]
pub struct LongjmpAction {
    pub invoc_loc: SrcLocEnum,
}

impl LongjmpAction {
    const LONGJMP_PREFIX: &'static str = "Longjmp Invocation: ";
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if !line.starts_with(Self::LONGJMP_PREFIX) {
            return Err(ActrecParseError::to_prefix_err(
                "Longjmp action prefix error",
            ));
        }
        let content = line[Self::LONGJMP_PREFIX.len()..].trim();
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() != 1 {
            return Err(ActrecParseError::as_parse_err(eyre!(
                "Longjmp action malformed, not 1 part: {}",
                line,
            )));
        }
        let invoc_loc = SrcLocEnum::from_str(
            parts
                .get(0)
                .ok_or_else(|| eyre!("Longjmp action missing invocation location"))?,
        )?;
        Ok(Self { invoc_loc })
    }
}

#[derive(Clone, Debug)]
pub struct UnwindAction {
    pub func_name: String,
}

impl UnwindAction {
    const UNWIND_PREFIX: &'static str = "Longjmp Unwind: ";
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if !line.starts_with(Self::UNWIND_PREFIX) {
            return Err(ActrecParseError::to_prefix_err(
                "Unwind action prefix error",
            ));
        }
        let content = line[Self::UNWIND_PREFIX.len()..].trim();
        let parts: Vec<&str> = content.split_whitespace().collect();
        let func_name = parts
            .get(0)
            .ok_or_else(|| eyre!("Unwind action missing function name"))?
            .to_string();
        Ok(Self { func_name })
    }
}

#[derive(Clone, Debug)]
pub enum SJVariant {
    PreLong,
    PostLong,
}

impl SJVariant {
    const PRELONG_PREFIX: &'static str = "Pre-long Setjmp: ";
    const POSTLONG_PREFIX: &'static str = "Post-long Setjmp: ";
    pub fn from_prefix(line: &str) -> Option<(Self, &str)> {
        if line.starts_with(Self::PRELONG_PREFIX) {
            Some((SJVariant::PreLong, &line[Self::PRELONG_PREFIX.len()..]))
        } else if line.starts_with(Self::POSTLONG_PREFIX) {
            Some((SJVariant::PostLong, &line[Self::POSTLONG_PREFIX.len()..]))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct SetjmpAction {
    sj_variants: SJVariant,
    pub func_name: String,
    pub stk_size: usize,
    pub invoc_loc: SrcLocEnum,
}

impl SetjmpAction {
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        let (sj_var, content) = match SJVariant::from_prefix(line) {
            Some((v, c)) => (v, c),
            None => {
                return Err(ActrecParseError::to_prefix_err(
                    "Setjmp action prefix error",
                ))
            }
        };
        let parts: Vec<&str> = content.trim().split_whitespace().collect();
        if parts.len() != 3 {
            return Err(ActrecParseError::as_parse_err(eyre!(
                "Setjmp action malformed, not 3 parts: {}",
                line
            )));
        }

        let func_name = parts
            .get(0)
            .ok_or_else(|| eyre!("Setjmp action missing function name"))?
            .to_string();
        let stk_size = parts
            .get(1)
            .ok_or_else(|| eyre!("Setjmp action missing stack size"))?
            .parse::<usize>()?;
        let invoc_loc = SrcLocEnum::from_str(
            parts
                .get(2)
                .ok_or_else(|| eyre!("Setjmp action missing invocation location"))?,
        )?;
        Ok(Self {
            sj_variants: sj_var,
            func_name,
            stk_size,
            invoc_loc,
        })
    }
}

#[derive(Clone, Debug)]
pub enum RollbackAction {
    Longjmp(LongjmpAction),
    Setjmp(SetjmpAction),
    Unwind(UnwindAction),
}

impl RollbackAction {
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if let Some(lj_act) = ActrecParseError::to_eyre(LongjmpAction::from_line(line))? {
            return Ok(RollbackAction::Longjmp(lj_act));
        }
        if let Some(sj_act) = ActrecParseError::to_eyre(SetjmpAction::from_line(line))? {
            return Ok(RollbackAction::Setjmp(sj_act));
        }
        if let Some(uw_act) = ActrecParseError::to_eyre(UnwindAction::from_line(line))? {
            return Ok(RollbackAction::Unwind(uw_act));
        }
        // prefix error for subsequent handling
        Err(ActrecParseError::to_prefix_err(
            "Rollback action prefix error",
        ))
    }

    pub fn get_loc(&self) -> Option<&SrcLocEnum> {
        match self {
            RollbackAction::Longjmp(lj_act) => Some(&lj_act.invoc_loc),
            RollbackAction::Setjmp(sj_act) => Some(&sj_act.invoc_loc),
            RollbackAction::Unwind(_) => None,
        }
    }

    pub fn plain_stmt_suitable(&self) -> bool {
        match self {
            RollbackAction::Longjmp(_) => true,
            RollbackAction::Setjmp(_) => true,
            RollbackAction::Unwind(_) => false,
        }
    }
}
