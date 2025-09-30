use std::{collections::HashSet, hash::Hash};

use crate::{
    analysis::constraint::{
        inter::{
            exec_tree::{
                action::{ExecAction, FuncAction, LoopAction, RecurAction},
                thread_tree::SharedFuncNodePtr,
                ExecForest,
            },
            loc::SrcLocEnum,
        },
        intra::func_src_tree::{
            builder::FuncSrcForest,
            code_query::scope_var_query::SrcVar,
            nodes::{FuncSrcTree, PlainStmtNode, SharedStmtNodePtr, StmtNodeVariants},
        },
    },
    feedback::branches::constraints::UBConstraint,
};

use clap::builder::Str;
use color_eyre::eyre::Result;
use eyre::bail;

pub type StmtStr = String;

pub struct SubCondExpr {
    range: (usize, usize),
    cond_val: bool,
}

pub struct CondExprUnit {
    cond_val: bool,
    sub_expr_vec: Vec<SubCondExpr>,
}

pub enum ProcessUnitVariant {
    Plain {},
    CondExpr(CondExprUnit),
}

pub struct ProcessUnit {
    content: String,
    valid_var_vec: Vec<SrcVar>,
    variants: ProcessUnitVariant,
}

impl ProcessUnit {
    /**
     * Construction Related
     */

    pub fn create_plain_pu(content: &str, valid_var_vec: Vec<SrcVar>) -> Self {
        Self {
            content: content.to_string(),
            valid_var_vec,
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn get_live_var(stmt_ptr: SharedStmtNodePtr) -> HashSet<SrcVar> {
        let mut var_set: HashSet<SrcVar> = HashSet::new();
        let mut cur_ptr = stmt_ptr.clone();
        let mut names_seen: HashSet<String> = HashSet::new();
        loop {
            for var in cur_ptr.borrow().valid_var_vec.iter() {
                if names_seen.contains(&var.name) {
                    continue;
                }
                var_set.insert(var.to_owned());
                names_seen.insert(var.name.to_string());
            }

            let par_ptr = match cur_ptr.borrow().get_parent_ptr() {
                Some(ptr) => ptr,
                None => break,
            };
            cur_ptr = par_ptr;
        }
        var_set
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

    fn construct_func_prior_stmts(
        &self,
        stmt_ptr: SharedStmtNodePtr,
        func_name: &str,
        invoc_loc_op: Option<&SrcLocEnum>,
    ) -> Result<Vec<ProcessUnit>> {
        // get func invoc loc
        let stmt_node = stmt_ptr.borrow();
        let stmt_loc = stmt_node.get_loc();
        let (stmt_str, invoc_idx_op) = stmt_loc.get_content_with_inner(invoc_loc_op)?;
        let invoc_idx = match invoc_idx_op {
            Some(idx) => idx,
            None => stmt_str
                .find(func_name)
                .expect("Failed to find function name in statement"),
        };

        // get actual parameter names
        let mut idx = invoc_idx + func_name.len();
        while idx < stmt_str.len() && stmt_str.as_bytes()[idx].is_ascii_whitespace() {
            idx += 1;
        }
        assert!(stmt_str.as_bytes()[idx] == b'(');
        idx += 1;
        let param_part = &stmt_str[idx..];
        let right_idx = param_part
            .find(')')
            .expect("Failed to find closing parenthesis for function parameters");
        let param_part = &param_part[..right_idx];
        let param_name_vec: Vec<&str> = param_part.split(',').map(|s| s.trim()).collect();

        let live_var_vec = ProcessUnit::get_live_var(stmt_ptr.clone());

        let mut param_var_vec: Vec<SrcVar> = vec![];
        for param_name in param_name_vec.iter() {
            let param_var_op = live_var_vec
                .iter()
                .find(|var| var.name == *param_name)
                .cloned();
            if let Some(param_var) = param_var_op {
                param_var_vec.push(param_var);
            } else {
                bail!(
                    "Failed to find parameter variable: {} in live variables",
                    param_name
                );
            }
        }

        // get formal parameter
        let called_func_tree = self.get_src_func_tree(func_name)?;
        let arg_var_vec = called_func_tree.get_formal_param_vec();

        assert!(param_var_vec.len() == arg_var_vec.len());

        // construct assignment statements for each param-arg pair
        let mut pu_vec: Vec<ProcessUnit> = vec![];
        for (param_var, arg_var) in param_var_vec.iter().zip(arg_var_vec.iter()) {
            assert!(
                param_var.var_type == arg_var.var_type,
                "Parameter and argument variable types do not match: {:?} vs {:?}",
                param_var.var_type,
                arg_var.var_type
            );
        }

        todo!()
    }

    // to be declared
    fn construct_func_after_stmts() {}

    fn func_invoc_handle(
        &self,
        stmt_ptr: SharedStmtNodePtr,
        func_name: &str,
        child_ptr: SharedFuncNodePtr,
        invoc_loc_op: Option<&SrcLocEnum>,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<SrcVar> {
        // prior
        let prior_stmt_vec = self.construct_func_prior_stmts(stmt_ptr, func_name, invoc_loc_op)?;
        pu_vec.extend(prior_stmt_vec);

        // call

        // after

        todo!()
    }

    fn collect_intra(
        &self,
        src_tree: &FuncSrcTree,
        exec_node_ptr: SharedFuncNodePtr,
    ) -> Result<Vec<ProcessUnit>> {
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
            return Ok(vec![]);
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
            let mut act = func_node.get_act_at_res(act_idx)?;

            match &stmt_node.variants {
                StmtNodeVariants::Block(_) => continue,
                StmtNodeVariants::CFStruct(cf_struct) => {
                    // TODO: core logic: need to invoke `iter.select()` here
                }
                StmtNodeVariants::Plain(plain_stmt) => {
                    // handle function imigrate and plain string collection.

                    // use new action match and sync algorithm

                    let mut ret_var_vec: Vec<SrcVar> = vec![];
                    while plain_stmt.match_act_loc(act)? {
                        act_idx += 1;
                        match act {
                            ExecAction::Func(func_act) => match func_act {
                                FuncAction::Call {
                                    func_name,
                                    child_ptr,
                                    invoc_loc,
                                } => {
                                    let ret_var = self.func_invoc_handle(
                                        stmt_ptr.clone(),
                                        func_name,
                                        child_ptr.clone(),
                                        invoc_loc.as_ref(),
                                        &mut pu_vec,
                                    )?;
                                    ret_var_vec.push(ret_var);
                                }
                                FuncAction::Unwind { loc } => {
                                    todo!()
                                }
                                _ => {
                                    bail!("Unexpected Func action: {:?}", func_act);
                                }
                            },
                            // do nothing for the non-func-call action
                            _ => {}
                        }
                        act = func_node.get_act_at_res(act_idx)?;
                    }
                }
            }
        }

        Ok(pu_vec)
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

        self.collect_intra(src_tree, func_node_ptr)
    }

    pub fn collect(&self) -> Result<Vec<ProcessUnit>> {
        let root_ptr = self.exec_forest.get_main_root_ptr();
        self.collect_recur(root_ptr)
    }
}
