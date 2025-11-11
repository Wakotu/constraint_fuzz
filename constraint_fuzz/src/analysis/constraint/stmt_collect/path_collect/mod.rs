use std::sync::Arc;

use crate::{
    analysis::constraint::{
        inter::exec_tree::{
            action::ExecAction,
            thread_tree::{ExecFuncNode, FuncActIter, SharedFuncNodePtr, Tid},
            ExecForest,
        },
        intra::{
            self,
            func_src_tree::{
                builder::FuncSrcForest,
                code_query::scope_var_query::SrcVar,
                nodes::{
                    cf_nodes::{CFNode, IfNode},
                    FuncSrcTree, FuncSrcTreeIter, PlainStmtNode, SharedStmtNodePtr, SrcExpr,
                    StmtNodeVariants,
                },
                stmts::QLLoc,
            },
        },
        stmt_collect::{
            path_collect::{inner_stmt::InnerStmtHandler, thread::ThreadJoinHandle},
            runtime_path::ThreadRuntimePath,
            ProcessUnit,
        },
        RtpTreeCollector,
    },
    feedback::branches::constraints::UBConstraint,
};
use color_eyre::eyre::Result;
use eyre::bail;

// inner implementation modules
pub mod inner_stmt;
pub mod loop_handle;
pub mod rollback;
pub mod switch_handle;
pub mod thread;

pub struct RuntimePathCollector {
    tid: Tid,
    exec_forest_ptr: Arc<ExecForest>,
    func_src_forest: &'static FuncSrcForest,
    exec_root_ptr: SharedFuncNodePtr,
    ub_cons: UBConstraint,

    // middle state
    subhdl_vec: Vec<ThreadJoinHandle>,
}

impl RuntimePathCollector {
    pub fn main_path_collect(tree_cltr: &RtpTreeCollector) -> Result<Vec<ThreadRuntimePath>> {
        let main_root_ptr = tree_cltr.exec_forest_ptr.get_main_root_ptr();
        let main_tid = tree_cltr.exec_forest_ptr.get_main_tid()?;

        let mut path_cltr = Self {
            tid: main_tid,
            exec_forest_ptr: tree_cltr.exec_forest_ptr.clone(),
            func_src_forest: tree_cltr.func_src_forest,
            exec_root_ptr: main_root_ptr,
            ub_cons: tree_cltr.ub_cons.clone(),
            subhdl_vec: vec![],
        };

        let main_tid = tree_cltr.exec_forest_ptr.get_main_tid()?;
        let pu_vec = path_cltr.path_collect(vec![])?;
        let main_path = ThreadRuntimePath::main_path_construct(main_tid, pu_vec);
        let path_vec = path_cltr.extend_sub_pathvecs(main_path);

        Ok(path_vec)
    }

    pub fn extend_sub_pathvecs(self, cur_path: ThreadRuntimePath) -> Vec<ThreadRuntimePath> {
        let mut path_vec = vec![cur_path];
        for handle in self.subhdl_vec {
            let sub_pathvec = handle
                .join()
                .expect("Sub Thread path collect: Failed to wait for sub thread handle");
            path_vec.extend(sub_pathvec);
        }
        path_vec
    }

    fn is_inside_loop(stmt_ptr: SharedStmtNodePtr) -> bool {
        let mut cur_ptr_op = stmt_ptr.read().unwrap().get_parent_ptr();
        loop {
            let cur_ptr = match cur_ptr_op {
                Some(ptr) => ptr,
                None => return false,
            };
            if cur_ptr.read().unwrap().is_loop_node() {
                return true;
            }

            cur_ptr_op = cur_ptr.read().unwrap().get_parent_ptr();
        }
    }

    fn allow_after(stmt_ptr: SharedStmtNodePtr, act: &ExecAction) -> bool {
        act.is_loop_entry() && Self::is_inside_loop(stmt_ptr)
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

    /// Return: whether to enter rollback status after current handle
    fn plain_stmt_handle(
        &self,
        plain_stmt: &PlainStmtNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<ProcessUnit>, bool, Vec<ThreadJoinHandle>)> {
        let mut handler = InnerStmtHandler::from_stmt_ptr(stmt_ptr.clone(), self)?;
        let mut uw_detect = false;
        let mut hdl_vec = vec![];

        loop {
            let act_op = act_iter.get_cur();
            let act = match act_op {
                None => break,
                Some(act) => act,
            };

            if !plain_stmt.act_inner(act)? {
                break;
            }
            // actions that match
            if act.is_longjmp() {
                uw_detect = true;
            }
            let (is_rb, inner_hdlvec) = handler.act_handle(act)?;
            hdl_vec.extend(inner_hdlvec);
            if is_rb {
                uw_detect = true;
                break; // break earlier
            }
            act_iter.update();
        }
        if plain_stmt.is_return_stmt() {
            let ret_expr = handler.get_finalpu_while_update(pu_vec)?;
            Ok((Some(ret_expr), uw_detect, hdl_vec))
        } else {
            handler.update_pu(pu_vec)?;
            Ok((None, uw_detect, hdl_vec))
        }
    }

    /**
     * Handle segments in header of control flow structures like for-init, for-update
     */
    fn cfseg_handle(
        &self,
        cfseg: &SrcExpr,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<bool> {
        let mut handler = InnerStmtHandler::new(cfseg.get_loc(), stmt_ptr.clone(), self)?;
        let mut uw_detect = false;

        loop {
            let act_op = act_iter.get_cur();
            let act = match act_op {
                None => break,
                Some(act) => act,
            };

            if !cfseg.act_inner(act)? {
                break;
            }
            let (is_rb, inner_hdlvec) = handler.act_handle(act)?;
            assert!(
                inner_hdlvec.is_empty(),
                "Thread Action should not appear at control flow segment handle"
            );
            if is_rb {
                uw_detect = true;
                break; // break earlier
            }
            act_iter.update();
        }
        handler.update_pu(pu_vec)?;
        Ok(uw_detect)
    }

    // Return trailing bool: if enters rollback status
    fn cond_expr_handle<'a>(
        &self,
        cond_expr: &SrcExpr,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &'a mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<&'a ExecAction>, bool)> {
        let mut handler = InnerStmtHandler::new(cond_expr.get_loc(), stmt_ptr.clone(), self)?;
        let (outact_op, is_rb) = loop {
            let act_op = act_iter.get_cur();
            // all acts here should be either inner act or outer act.
            let act = match act_op {
                None => bail!(
                    "Cond Expr Handle: inner and outer struct should not exceed function action idx"
                ),
                Some(act) => act,
            };
            let (is_inner, is_outer) = cond_expr.cond_expr_act_match(act)?;
            if !is_inner {
                break (Some(act), false);
            }
            let (is_rb, inner_hdlvec) = handler.act_handle(act)?;
            assert!(
                inner_hdlvec.is_empty(),
                "Thread Action should not appear at cond expr handle"
            );
            if is_rb {
                break (None, true);
            }
            if is_outer {
                break (Some(act), false);
            }

            act_iter.update();
        };
        // NOTE: here act_idx should lies in the one after outer_act.
        handler.update_pu(pu_vec)?;

        Ok((outact_op, is_rb))
    }

    fn if_struct_handle(
        &self,
        if_node: &IfNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        let cond_expr = if_node.get_cond_expr();

        // inner expression handle
        let (outer_act_op, is_rb) =
            self.cond_expr_handle(cond_expr, stmt_ptr.clone(), act_iter, pu_vec)?;
        if is_rb {
            return Ok((None, true));
        }
        let outer_act = outer_act_op.ok_or_else(|| {
            eyre::eyre!("If Struct Handle: should have outer action after cond expr")
        })?;

        // current action would be always an outer action
        let body_ptr_op = if_node.get_dest_body(outer_act)?;
        // NOTE: None determines end of the Func Src Tree
        let next_ptr_op = match body_ptr_op {
            Some(ptr) => Some(ptr),
            None => FuncSrcTreeIter::get_afternext_ptr(stmt_ptr.clone())?,
        };
        Ok((next_ptr_op, false))
    }

    fn check_recur_lock(func_node: &ExecFuncNode) -> Result<(bool, bool)> {
        let mut act_iter = func_node.iter();

        // let mut act_idx: usize = 0;

        // check for recur locked at the beginning
        let first_act = act_iter
            .next()
            .ok_or_else(|| eyre::eyre!("Function node should have at least one action"))?;
        if first_act.is_recur_lock() {
            let second_act = act_iter.next().ok_or_else(|| {
                eyre::eyre!(
                    "Function node should have at least two actions when first is Recur Locked"
                )
            })?;
            assert!(
                second_act.is_recur_release(),
                "Second Action should be Recur Released action"
            );

            // check third action
            let third_act = act_iter.next().ok_or_else(|| {
                eyre::eyre!("Recur Locked function node should has at least 3 actions")
            })?;

            Ok((true, third_act.is_unwind()))
        } else {
            Ok((false, false))
        }
    }

    /// Return : trailing bool means whether to enter rollback status after current function handle
    fn collect_intra(
        &self,
        src_tree: &FuncSrcTree,
        exec_node_ptr: SharedFuncNodePtr,
    ) -> Result<(
        Vec<ProcessUnit>,
        Option<SrcVar>,
        bool,
        Vec<ThreadJoinHandle>,
    )> {
        let func_node = exec_node_ptr.read().unwrap();
        let (is_recur_lock, enter_rb) = Self::check_recur_lock(&func_node)?;
        if is_recur_lock {
            return Ok((vec![], None, enter_rb, vec![]));
        }

        let mut act_iter = func_node.iter();

        let mut pu_vec: Vec<ProcessUnit> = vec![];

        let mut src_iter = src_tree.iter();
        let mut stmt_ptr_op;
        let mut func_hdlvec = vec![];
        loop {
            // iteration logic
            stmt_ptr_op = src_iter.next();
            let stmt_ptr = match stmt_ptr_op {
                Some(ptr) => ptr,
                None => break,
            };
            let stmt_node = stmt_ptr.read().unwrap();

            match &stmt_node.variants {
                StmtNodeVariants::Block(_) => continue,
                StmtNodeVariants::CF(cf_struct) => {
                    let (next_ptr_op, is_rb) = match cf_struct {
                        // TODO: modify return value of if interface and switch interface
                        CFNode::If(if_node) => self.if_struct_handle(
                            if_node,
                            stmt_ptr.clone(),
                            &mut act_iter,
                            &mut pu_vec,
                        )?,
                        CFNode::While(while_node) => self.while_node_handle(
                            while_node,
                            stmt_ptr.clone(),
                            &mut act_iter,
                            &mut pu_vec,
                        )?,
                        CFNode::Switch(switch_node) => self.switch_node_handle(
                            switch_node,
                            stmt_ptr.clone(),
                            &mut act_iter,
                            &mut pu_vec,
                        )?,
                        CFNode::For(for_node) => self.for_node_handle(
                            for_node,
                            stmt_ptr.clone(),
                            &mut act_iter,
                            &mut pu_vec,
                        )?,
                    };
                    if is_rb {
                        // rollback inner
                        let is_inner = Self::rollback_detect(&mut act_iter)?;
                        if is_inner {
                            return Ok((pu_vec, None, true, func_hdlvec));
                        } else {
                            Self::rollback_exit_handle(src_tree, &mut act_iter, &mut src_iter)?;
                        }
                    } else {
                        src_iter.update(next_ptr_op);
                    }
                }
                StmtNodeVariants::Plain(plain_stmt) => {
                    // handle function imigrate and plain string collection.

                    // handle return stmt

                    let (retexpr_op, is_rb, hdl_vec) = self.plain_stmt_handle(
                        plain_stmt,
                        stmt_ptr.clone(),
                        &mut act_iter,
                        &mut pu_vec,
                    )?;
                    func_hdlvec.extend(hdl_vec);

                    if is_rb {
                        let is_inner = Self::rollback_detect(&mut act_iter)?;
                        if is_inner {
                            return Ok((pu_vec, None, true, func_hdlvec));
                        } else {
                            // rollback exit handle
                            Self::rollback_exit_handle(src_tree, &mut act_iter, &mut src_iter)?;
                        }
                    } else {
                        if let Some(ret_expr) = retexpr_op {
                            let ret_expr = ret_expr
                                .get_exprpu_ref()
                                .ok_or_else(|| eyre::eyre!("Return Expr should be expr pu"))?;
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
                            return Ok((pu_vec, ret_var_op, false, func_hdlvec));
                        }
                    }
                }
            }
        }

        Ok((pu_vec, None, false, func_hdlvec))
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

    fn collect_recur(&mut self, func_node_ptr: SharedFuncNodePtr) -> Result<Vec<ProcessUnit>> {
        let func_node = func_node_ptr.read().unwrap();
        // handle init node
        if func_node.is_init() {
            let child_ptr = func_node.get_entryfunc_ptr()?;
            return self.collect_recur(child_ptr);
        }

        let func_name = func_node.get_func_name().ok_or_else(|| {
            eyre::eyre!("Function node should have a function name, but got None")
        })?;

        let src_tree = self.get_src_func_tree(func_name)?;
        drop(func_node);

        let (pu_vec, _, is_rb, hdl_vec) = self.collect_intra(src_tree, func_node_ptr)?;
        self.subhdl_vec.extend(hdl_vec);
        assert!(
            !is_rb,
            "Top Level Function should not enter rollback status after intra collection"
        );
        Ok(pu_vec)
    }

    pub fn path_collect(&mut self, prepath_pu: Vec<ProcessUnit>) -> Result<Vec<ProcessUnit>> {
        let mut pu_vec = prepath_pu;
        let intra_pu_vec = self.collect_recur(self.exec_root_ptr.clone())?;
        pu_vec.extend(intra_pu_vec);
        Ok(pu_vec)
    }
}
