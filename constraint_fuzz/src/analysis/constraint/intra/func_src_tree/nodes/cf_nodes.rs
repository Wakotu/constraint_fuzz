use color_eyre::eyre::Result;
use std::{cmp::Ordering, collections::HashMap};

use eyre::bail;
use my_macros::EquivByLoc;

use crate::analysis::constraint::{
    inter::{
        exec_tree::action::{ExecAction, JumpAction, JumpActionType},
        loc::SrcLocEnum,
    },
    intra::func_src_tree::{
        nodes::{SharedStmtNodePtr, SrcExpr},
        stmts::{QLLoc, WhileType},
    },
};

#[derive(EquivByLoc)]
pub struct IfNode {
    pub loc: QLLoc,
    pub cond_expr: SrcExpr,
    pub then_ptr: SharedStmtNodePtr,
    pub else_ptr_op: Option<SharedStmtNodePtr>,
}

impl IfNode {
    fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }
    pub fn get_next_ptr(&self, outer_act: &ExecAction) -> Result<Option<SharedStmtNodePtr>> {
        let jump_act = match outer_act {
            ExecAction::Intra(jump_act) => jump_act,
            act => bail!(
                "Cond Expr Outer action of If Node should not be of action type {:?}",
                act
            ),
        };

        let dest_loc = match &jump_act.jump_variants {
            JumpActionType::BrGuard { val_loc: _ } => &jump_act.dest_loc,
            JumpActionType::MergeBrGuard => &jump_act.dest_loc,
            jump_var => bail!(
                "Cond Expr Outer action of If Node should not be of action type {:?}",
                jump_var
            ),
        };

        let then_contains = {
            let then_node = self.then_ptr.borrow();
            then_node.src_loc_inner(dest_loc)
        };
        if then_contains {
            return Ok(Some(self.then_ptr.clone()));
        }

        match &self.else_ptr_op {
            None => Ok(None),
            Some(else_ptr) => {
                let else_contains = {
                    let else_node = else_ptr.borrow();
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

pub type CasePtrMap = HashMap<QLLoc, Vec<SharedStmtNodePtr>>;

#[derive(EquivByLoc)]
pub struct SwitchNode {
    pub loc: QLLoc,
    pub expr_loc: SrcExpr,
    pub case_ptr_map: CasePtrMap,
}

impl SwitchNode {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }
}

#[derive(EquivByLoc)]
pub struct WhileNode {
    pub loc: QLLoc,
    pub while_type: WhileType,
    pub cond_expr: SrcExpr,
    pub body: SharedStmtNodePtr,
}

impl WhileNode {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }
}

#[derive(EquivByLoc)]
pub struct ForNode {
    pub loc: QLLoc,
    pub init: Option<SrcExpr>,
    pub cond: Option<SrcExpr>,
    pub update: Option<SrcExpr>,
    pub body: SharedStmtNodePtr,
}

impl ForNode {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }
}

pub enum CFStruct {
    If(IfNode),
    Switch(SwitchNode),
    While(WhileNode),
    For(ForNode),
}

impl CFStruct {
    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self {
            Self::If(if_node) => if_node.src_loc_inner(src_loc),
            Self::Switch(switch_node) => switch_node.src_loc_inner(src_loc),
            Self::While(while_node) => while_node.src_loc_inner(src_loc),
            Self::For(for_node) => for_node.src_loc_inner(src_loc),
        }
    }
}
