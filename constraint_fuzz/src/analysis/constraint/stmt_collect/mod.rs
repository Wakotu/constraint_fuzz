use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

pub mod inner_stmt;

use crate::{
    analysis::constraint::{
        inter::{
            exec_tree::{
                action::{ExecAction, FuncAction, LoopAction, RecurAction},
                thread_tree::{ExecFuncNode, SharedFuncNodePtr},
                ExecForest,
            },
            loc::SrcLocEnum,
        },
        intra::func_src_tree::{
            builder::FuncSrcForest,
            code_query::{custom_class_query::VarType, scope_var_query::SrcVar},
            nodes::{
                cf_nodes::{CFStruct, IfNode},
                FuncSrcTree, FuncSrcTreeIter, PlainStmtNode, SharedStmtNodePtr, StmtNodeVariants,
            },
            stmts::QLLoc,
        },
        stmt_collect::inner_stmt::{ArgExpr, InnerStmtHandler, InvocSubstOpr},
    },
    feedback::branches::constraints::UBConstraint,
};

use chrono::format::format;
use clap::builder::Str;
use color_eyre::eyre::Result;
use eyre::bail;
use reqwest::header::IF_NONE_MATCH;

pub type StmtStr = String;

pub enum ProcessUnitVariant {
    Plain {},
    CondExpr { val: bool },
}

#[derive(Clone, PartialEq, Eq)]
pub struct InnerCondRec {
    inner_idx: usize,
    cond_val: bool,
}

impl InnerCondRec {
    pub fn before(&self, loc: usize) -> bool {
        self.inner_idx < loc
    }

    pub fn before_or_eq(&self, loc: usize) -> bool {
        self.inner_idx <= loc
    }

    pub fn derive_minus(&self, loc: usize) -> Self {
        Self {
            inner_idx: self.inner_idx - loc,
            cond_val: self.cond_val,
        }
    }

    pub fn derive_plus(&self, loc: usize) -> Self {
        Self {
            inner_idx: self.inner_idx + loc,
            cond_val: self.cond_val,
        }
    }
}

impl PartialOrd for InnerCondRec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.inner_idx.partial_cmp(&other.inner_idx)
    }
}

impl Ord for InnerCondRec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner_idx.cmp(&other.inner_idx)
    }
}

pub struct ProcessUnit {
    pub content: String,
    pub valid_var_vec: Vec<SrcVar>,
    pub cond_rec_vec: Vec<InnerCondRec>,
    pub variants: ProcessUnitVariant,
}

impl ProcessUnit {
    /**
     * Construction Related
     */

    /**
     * Plain means no extra information
     */
    pub fn create_plain_pu(content: String, valid_var_vec: Vec<SrcVar>) -> Self {
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: vec![],
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_pre_func_assign_pu(arg_expr: &ArgExpr, param_var: &SrcVar) -> Self {
        let param_var_str = param_var.var_name_str();
        let pre_off = param_var_str.len() + 3;
        let assign_str = format!("{} = {};", param_var.var_name_str(), arg_expr.expr_str);

        let mut var_vec = arg_expr.var_vec.clone();
        var_vec.push(param_var.clone());

        let cond_vec = arg_expr.derive_cond_vec(pre_off);

        Self {
            content: assign_str,
            valid_var_vec: var_vec,
            cond_rec_vec: cond_vec,
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_plain_pu_with_cond_recs(
        content: String,
        valid_var_vec: Vec<SrcVar>,
        cond_recs: &Vec<InnerCondRec>,
    ) -> Self {
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: cond_recs.to_vec(),
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_ret_assign_pu(
        ret_expr: &str,
        ret_var: &SrcVar,
        ret_stmt_ptr: SharedStmtNodePtr,
    ) -> Self {
        let content = format!("{} = {};", ret_var.name, ret_expr);
        let mut valid_var_vec = SrcVar::get_live_var(ret_stmt_ptr);
        valid_var_vec.push(ret_var.clone());
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: vec![],
            variants: ProcessUnitVariant::Plain {},
        }
    }
}

pub struct StmtCollector<'a> {
    exec_forest: &'a ExecForest,
    func_src_forest: &'a FuncSrcForest,
    ub_cons: &'a UBConstraint,
}

impl<'a> StmtCollector<'a> {
    pub fn new(
        exec_forest: &'a ExecForest,
        func_src_forest: &'a FuncSrcForest,
        ub_cons: &'a UBConstraint,
    ) -> Self {
        Self {
            exec_forest,
            func_src_forest,
            ub_cons,
        }
    }

    fn is_inside_loop(stmt_ptr: SharedStmtNodePtr) -> bool {
        let mut cur_ptr_op = stmt_ptr.borrow().get_parent_ptr();
        loop {
            let cur_ptr = match cur_ptr_op {
                Some(ptr) => ptr,
                None => return false,
            };
            if cur_ptr.borrow().is_loop_node() {
                return true;
            }

            cur_ptr_op = cur_ptr.borrow().get_parent_ptr();
        }
    }

    fn allow_after(stmt_ptr: SharedStmtNodePtr, act: &ExecAction) -> bool {
        let loop_act = match act {
            ExecAction::Loop(loop_act) => loop_act,
            _ => {
                return false;
            }
        };
        loop_act.is_loop_entry() && Self::is_inside_loop(stmt_ptr)
    }

    fn create_ret_var(src_tree: &FuncSrcTree, ret_loc: QLLoc) -> Option<SrcVar> {
        if src_tree.ret_type.is_void() {
            return None;
        }
        Some(SrcVar {
            loc: ret_loc,
            name: format!("{}_ret", src_tree.func_name),
            var_type: src_tree.ret_type.clone(),
        })
    }

    fn plain_stmt_handle(
        &self,
        plain_stmt: &PlainStmtNode,
        stmt_ptr: SharedStmtNodePtr,
        func_node: &ExecFuncNode,
        pu_vec: &mut Vec<ProcessUnit>,
        act_idx: &mut usize,
    ) -> Result<()> {
        let mut act = func_node.get_act_at_res(*act_idx)?;

        let mut handler = InnerStmtHandler::from_stmt_ptr(stmt_ptr.clone(), self)?;
        while plain_stmt.action_inner(act)? {
            handler.act_handle(act)?;
            // act move forward
            *act_idx += 1;
            act = func_node.get_act_at_res(*act_idx)?;
        }
        handler.update_pu(pu_vec)?;
        Ok(())
    }

    fn if_struct_handle(
        &self,
        if_node: &IfNode,
        stmt_ptr: SharedStmtNodePtr,
        func_node: &ExecFuncNode,
        pu_vec: &mut Vec<ProcessUnit>,
        act_idx: &mut usize,
    ) -> Result<SharedStmtNodePtr> {
        let cond_expr = if_node.get_cond_expr();

        // inner expression handle
        let mut handler = InnerStmtHandler::new(cond_expr.get_loc(), stmt_ptr.clone(), self)?;
        let outer_act = loop {
            let act = func_node.get_act_at_res(*act_idx)?;
            let (is_inner, is_outer) = cond_expr.cond_expr_act_match(act)?;
            if !is_inner {
                break act;
            }
            handler.act_handle(act)?;
            if is_outer {
                break act;
            }
            // update act
            *act_idx += 1;
        };
        handler.update_pu(pu_vec)?;

        // current action would be always an outer action
        let next_ptr_op = if_node.get_next_ptr(outer_act)?;
        let next_ptr = match next_ptr_op {
            Some(ptr) => ptr,
            None => {
                let next_ptr_op = FuncSrcTreeIter::get_next_sibling_ptr(stmt_ptr.clone())?;
                match next_ptr_op {
                    None => bail!("If Struct Handle: Failed to find next sibling ptr"),
                    Some(p) => p,
                }
            }
        };
        *act_idx += 1;
        Ok(next_ptr)
    }

    fn collect_intra(
        &self,
        src_tree: &FuncSrcTree,
        exec_node_ptr: SharedFuncNodePtr,
    ) -> Result<(Vec<ProcessUnit>, Option<SrcVar>)> {
        let exec_func = exec_node_ptr.borrow();
        let mut act_idx: usize = 0;

        // check for recur locked at the beginning
        let first_act = exec_func
            .get_act_at(act_idx)
            .ok_or_else(|| eyre::eyre!("Function node should have at least one action"))?;
        if let ExecAction::Recur(RecurAction::Locked) = first_act {
            act_idx += 1;
            let second_act = exec_func.get_act_at(act_idx).ok_or_else(|| {
                eyre::eyre!(
                    "Function node should have at least two actions when first is Recur Locked"
                )
            })?;
            assert!(
                matches!(second_act, ExecAction::Recur(RecurAction::Released)),
                "Second Action should be Recur Released action"
            );
            return Ok((vec![], None));
        }

        let mut pu_vec: Vec<ProcessUnit> = vec![];
        let func_node = exec_node_ptr.borrow();

        let mut iter = src_tree.iter();
        let mut stmt_ptr_op;
        loop {
            // iteration logic
            stmt_ptr_op = iter.next();
            let stmt_ptr = match stmt_ptr_op {
                Some(ptr) => ptr,
                None => break,
            };
            let stmt_node = stmt_ptr.borrow();

            match &stmt_node.variants {
                StmtNodeVariants::Block(_) => continue,
                StmtNodeVariants::CFStruct(cf_struct) => {
                    let next_stmt_ptr = match cf_struct {
                        CFStruct::If(if_node) => self.if_struct_handle(
                            if_node,
                            stmt_ptr.clone(),
                            &func_node,
                            &mut pu_vec,
                            &mut act_idx,
                        )?,
                        CFStruct::While(while_node) => {
                            todo!()
                        }
                        CFStruct::Switch(switch_node) => {
                            todo!()
                        }
                        CFStruct::For(for_node) => {
                            todo!()
                        }
                    };

                    iter.update_in_cf(next_stmt_ptr);
                }
                StmtNodeVariants::Plain(plain_stmt) => {
                    // handle function imigrate and plain string collection.

                    // handle return stmt
                    let ret_expr_op = plain_stmt.get_return_expr()?;
                    if let Some(ret_expr) = ret_expr_op {
                        let ret_var_op = Self::create_ret_var(src_tree, plain_stmt.loc.clone());
                        match ret_var_op {
                            None => {}
                            Some(ref ret_var) => {
                                let ret_assign_pu = ProcessUnit::create_ret_assign_pu(
                                    &ret_expr,
                                    &ret_var,
                                    stmt_ptr.clone(),
                                );
                                pu_vec.push(ret_assign_pu);
                            }
                        }
                        return Ok((pu_vec, ret_var_op));
                    }

                    self.plain_stmt_handle(
                        plain_stmt,
                        stmt_ptr.clone(),
                        &func_node,
                        &mut pu_vec,
                        &mut act_idx,
                    )?;
                }
            }
        }

        Ok((pu_vec, None))
    }

    fn get_src_func_tree(&self, func_name: &str) -> Result<&FuncSrcTree> {
        self.func_src_forest.get_value(func_name).ok_or_else(|| {
            eyre::eyre!(
                "Function source tree not found for function: {}. Available functions: {:?}",
                func_name,
                self.func_src_forest.get_all_func_names()
            )
        })
    }

    fn collect_recur(&self, func_node_ptr: SharedFuncNodePtr) -> Result<Vec<ProcessUnit>> {
        let func_node = func_node_ptr.borrow();
        if func_node.is_init() {
            assert!(
                func_node.data.len() == 1,
                "Init node should have only one action"
            );
            let exec_act = &func_node.data[0];
            match exec_act {
                ExecAction::Func(func_act) => {
                    let child_ptr = func_act
                        .get_child_ptr()
                        .ok_or_else(|| eyre::eyre!("Init Func action should have a child node"))?;
                    return self.collect_recur(child_ptr);
                }
                _ => {
                    bail!("Init node should have only one Func action");
                }
            }
        }

        let func_name = func_node.get_func_name().ok_or_else(|| {
            eyre::eyre!("Function node should have a function name, but got None")
        })?;

        let src_tree = self.get_src_func_tree(func_name)?;
        drop(func_node);

        let (pu_vec, _) = self.collect_intra(src_tree, func_node_ptr)?;
        Ok(pu_vec)
    }

    pub fn collect(&self) -> Result<Vec<ProcessUnit>> {
        let root_ptr = self.exec_forest.get_main_root_ptr();
        self.collect_recur(root_ptr)
    }
}
