use crate::analysis::constraint::inter::error::ActrecParseError;

#[derive(Clone)]
pub enum RecurLockAct {
    Locked,
    Released,
}

impl RecurLockAct {
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        match line {
            "Recur Lock locked" => Ok(RecurLockAct::Locked),
            "Recur Lock released" => Ok(RecurLockAct::Released),
            _ => Err(ActrecParseError::as_prefix_err(eyre::eyre!(
                "
                Line does not match any known recur action type: {}",
                line
            ))),
        }
    }
}

#[derive(Clone)]
pub enum LoopLockAct {
    Locked,
    Released,
}

impl LoopLockAct {
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        match line {
            "Loop Lock locked" => Ok(LoopLockAct::Locked),
            "Loop Lock released" => Ok(LoopLockAct::Released),
            _ => Err(ActrecParseError::as_prefix_err(eyre::eyre!(
                "
                Line does not match any known loop action type: {}",
                line
            ))),
        }
    }
}

#[derive(Clone)]
pub enum LockAction {
    Recur(RecurLockAct),
    Loop(LoopLockAct),
}

impl LockAction {
    pub fn from_line(line: &str) -> std::result::Result<Self, ActrecParseError> {
        if let Some(recur_act) = ActrecParseError::to_eyre(RecurLockAct::from_line(line))? {
            return Ok(LockAction::Recur(recur_act));
        }
        if let Some(loop_act) = ActrecParseError::to_eyre(LoopLockAct::from_line(line))? {
            return Ok(LockAction::Loop(loop_act));
        }
        // prefix error at the end
        Err(ActrecParseError::as_prefix_err(eyre::eyre!(
            "
            Line does not match any known lock action type: {}",
            line
        )))
    }
}
