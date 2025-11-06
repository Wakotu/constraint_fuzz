use color_eyre::eyre::Result;
use std::{cmp::Ordering, collections::HashMap, rc::Rc, sync::Arc};

use eyre::bail;
use my_macros::EquivByLoc;

use crate::analysis::constraint::{
    inter::{
        exec_tree::action::{ExecAction, JumpAction, JumpActionType},
        loc::{SrcLocEnum, ValidSrcLoc},
    },
    intra::func_src_tree::{
        nodes::{SharedStmtNodePtr, SrcExpr},
        stmts::{QLLoc, WhileType},
    },
    stmt_collect::ProcessUnit,
};

#[derive(EquivByLoc)]
pub struct IfNode {
    pub loc: QLLoc,
    pub cond_expr: SrcExpr,
    pub then_body: SharedStmtNodePtr,
    pub else_body_op: Option<SharedStmtNodePtr>,
}

impl IfNode {
    fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn get_dest_body(&self, outer_act: &ExecAction) -> Result<Option<SharedStmtNodePtr>> {
        let dest_loc = outer_act.get_outer_destloc()?;
        let then_contains = {
            let then_node = self.then_body.read().unwrap();
            then_node.src_loc_inner(dest_loc)
        };
        if then_contains {
            return Ok(Some(self.then_body.clone()));
        }

        match &self.else_body_op {
            None => Ok(None),
            Some(else_ptr) => {
                let else_contains = {
                    let else_node = else_ptr.read().unwrap();
                    else_node.src_loc_inner(dest_loc)
                };
                if else_contains {
                    Ok(Some(else_ptr.clone()))
                } else {
                    bail!("If Node outer action match: dest loc not located in both then and else stmt")
                }
            }
        }
    }

    pub fn get_cond_expr(&self) -> &SrcExpr {
        &self.cond_expr
    }
}

// case expr loc -> Vec of case stmts
pub type CasePtrMap = HashMap<QLLoc, Vec<SharedStmtNodePtr>>;

#[derive(EquivByLoc)]
pub struct SwitchCase {
    loc: QLLoc,
}

impl SwitchCase {
    pub fn get_case_literal(&self) -> Result<String> {
        const CASE_PREFIX: &str = "case ";
        let case_str = self.loc.get_content()?;
        // remove prefix and trailing colon
        Ok(case_str[CASE_PREFIX.len()..case_str.len() - 1].to_string())
    }
}

impl PartialOrd for SwitchCase {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.loc.partial_cmp(&other.loc)
    }
}

impl Ord for SwitchCase {
    fn cmp(&self, other: &Self) -> Ordering {
        self.loc.cmp(&other.loc)
    }
}

pub struct SwitchArm {
    case: SwitchCase,
    // not a block nor a stmt
    body: Vec<SharedStmtNodePtr>,
}

impl SwitchArm {
    pub fn derive_cond_pu(&self, expr_pu: ProcessUnit) -> Result<ProcessUnit> {
        let case_lit = self.case.get_case_literal()?;
        Ok(ProcessUnit::concat_cond_pu(expr_pu, case_lit))
    }

    pub fn get_first_body_ptr(&self) -> Option<SharedStmtNodePtr> {
        self.body.first().cloned()
    }

    pub fn case_before(&self, valid_loc: &ValidSrcLoc) -> bool {
        match self
            .case
            .loc
            .compare_line_and_col(valid_loc.line, valid_loc.col)
        {
            Ordering::Less => true,
            _ => false,
        }
    }
}

impl PartialEq for SwitchArm {
    fn eq(&self, other: &Self) -> bool {
        self.case == other.case
    }
}

impl Eq for SwitchArm {}

impl PartialOrd for SwitchArm {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.case.partial_cmp(&other.case)
    }
}

impl Ord for SwitchArm {
    fn cmp(&self, other: &Self) -> Ordering {
        self.case.cmp(&other.case)
    }
}

impl SwitchArm {
    pub fn get_vec_from_caseptr_map(case_ptr_map: CasePtrMap) -> Vec<Self> {
        let mut arms = Vec::new();
        for (case_loc, stmt_ptrs) in case_ptr_map.into_iter() {
            let case = SwitchCase { loc: case_loc };
            let arm = SwitchArm {
                case,
                body: stmt_ptrs,
            };
            arms.push(arm);
        }
        arms.sort();
        arms
    }
}

#[derive(EquivByLoc)]
pub struct SwitchNode {
    pub loc: QLLoc,
    pub expr: SrcExpr,
    pub arm_vec: Vec<SwitchArm>,
}

impl SwitchNode {
    pub fn set_parent_state_for_caseptrs(&self, cur_ptr: SharedStmtNodePtr) {
        for (arm_idx, arm) in self.arm_vec.iter().enumerate() {
            for (case_idx, case_ptr) in arm.body.iter().enumerate() {
                case_ptr.write().unwrap().parent_ptr_op = Some(Arc::downgrade(&cur_ptr));
                case_ptr.write().unwrap().parent_idx_op = Some(case_idx);
                case_ptr.write().unwrap().parent_armidx_op = Some(arm_idx);
            }
        }
    }

    pub fn get_expr(&self) -> &SrcExpr {
        &self.expr
    }

    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn get_arm_body(&self, arm_idx: usize) -> Option<&Vec<SharedStmtNodePtr>> {
        let arm = match self.arm_vec.get(arm_idx) {
            None => return None,
            Some(arm) => arm,
        };
        Some(&arm.body)
    }

    pub fn act_match(&self, jump_act: &JumpAction) -> Result<bool> {
        let from_loc = match &jump_act.from_loc {
            SrcLocEnum::NullLoc => bail!("Switch Node action match: from loc is null"),
            SrcLocEnum::Valid(loc) => loc,
        };
        Ok(self.loc.start_match(from_loc) && self.src_loc_inner(&jump_act.dest_loc))
    }

    pub fn get_dest_arm<'a>(&'a self, dest_loc: &SrcLocEnum) -> Result<&'a SwitchArm> {
        let valid_loc = match dest_loc {
            SrcLocEnum::NullLoc => bail!("Switch Node get dest case: dest loc is null"),
            SrcLocEnum::Valid(loc) => loc,
        };

        for arm in self.arm_vec.iter().rev() {
            if arm.case_before(valid_loc) {
                return Ok(arm);
            }
        }
        bail!("Switch Node get dest arm: no matching arm found")
    }
}

pub struct LoopPart<'a> {
    pub loc: &'a QLLoc,
    pub cond_expr: Option<&'a SrcExpr>,
    pub body: &'a SharedStmtNodePtr,
    pub count: usize,
}

impl<'a> LoopPart<'a> {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn get_dest_body(&self, outer_act: &ExecAction) -> Result<Option<SharedStmtNodePtr>> {
        let dest_loc = outer_act.get_outer_destloc()?;

        let body_contains = {
            let body_node = self.body.read().unwrap();
            body_node.src_loc_inner(dest_loc)
        };
        if body_contains {
            Ok(Some(self.body.clone()))
        } else {
            Ok(None)
        }
    }

    pub fn get_body_ptr(&self) -> SharedStmtNodePtr {
        self.body.clone()
    }

    pub fn get_cond_op(&self) -> Option<&'a SrcExpr> {
        self.cond_expr
    }
}

#[derive(EquivByLoc)]
pub struct WhileNode {
    pub loc: QLLoc,
    pub while_type: WhileType,
    pub cond_expr: SrcExpr,
    pub body: SharedStmtNodePtr,
    pub count: usize,
}

impl WhileNode {
    pub fn derive_loop_part<'a>(&'a self) -> LoopPart<'a> {
        LoopPart {
            loc: &self.loc,
            cond_expr: Some(&self.cond_expr),
            body: &self.body,
            count: self.count,
        }
    }

    pub fn new(
        loc: &QLLoc,
        while_type: &WhileType,
        cond_expr: &SrcExpr,
        body: SharedStmtNodePtr,
    ) -> Self {
        Self {
            loc: loc.clone(),
            while_type: while_type.clone(),
            cond_expr: cond_expr.clone(),
            body: body,
            count: 0,
        }
    }

    pub fn is_first_visit(&self) -> bool {
        self.count == 1
    }

    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn get_cond_expr(&self) -> &SrcExpr {
        &self.cond_expr
    }

    pub fn get_body_ptr(&self) -> SharedStmtNodePtr {
        self.body.clone()
    }

    pub fn get_dest_body(&self, outer_act: &ExecAction) -> Result<Option<SharedStmtNodePtr>> {
        let dest_loc = outer_act.get_outer_destloc()?;

        let body_contains = {
            let body_node = self.body.read().unwrap();
            body_node.src_loc_inner(dest_loc)
        };
        if body_contains {
            Ok(Some(self.body.clone()))
        } else {
            Ok(None)
        }
    }
}

#[derive(EquivByLoc)]
pub struct ForNode {
    pub loc: QLLoc,
    pub init: Option<SrcExpr>,
    pub cond_expr: Option<SrcExpr>,
    pub update: Option<SrcExpr>,
    pub body: SharedStmtNodePtr,
    pub count: usize,
}

impl ForNode {
    pub fn derive_loop_part<'a>(&'a self) -> LoopPart<'a> {
        LoopPart {
            loc: &self.loc,
            cond_expr: self.cond_expr.as_ref(),
            body: &self.body,
            count: self.count,
        }
    }

    pub fn new(
        loc: &QLLoc,
        init: Option<SrcExpr>,
        cond: Option<SrcExpr>,
        update: Option<SrcExpr>,
        body: SharedStmtNodePtr,
    ) -> Self {
        Self {
            loc: loc.clone(),
            init,
            cond_expr: cond,
            update,
            body,
            count: 0,
        }
    }

    pub fn is_first_visit(&self) -> bool {
        self.count == 1
    }

    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }
}

pub enum CFNode {
    If(IfNode),
    Switch(SwitchNode),
    While(WhileNode),
    For(ForNode),
}

impl CFNode {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self {
            Self::If(if_node) => if_node.src_loc_inner(src_loc),
            Self::Switch(switch_node) => switch_node.src_loc_inner(src_loc),
            Self::While(while_node) => while_node.src_loc_inner(src_loc),
            Self::For(for_node) => for_node.src_loc_inner(src_loc),
        }
    }
}
