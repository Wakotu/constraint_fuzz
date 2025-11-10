use crate::{
    analysis::constraint::{
        inter::exec_tree::{action::ExecAction, ExecForest},
        intra::func_src_tree::{
            builder::FuncSrcForest,
            nodes::{FuncSrcTree, SharedStmtNodePtr},
        },
    },
    feedback::branches::constraints::UBConstraint,
};

// inner implementation modules
pub mod inner_stmt;
pub mod loop_handle;
pub mod rollback;
pub mod switch_handle;
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
    ) -> Result<(Option<ProcessUnit>, bool)> {
        let mut handler = InnerStmtHandler::from_stmt_ptr(stmt_ptr.clone(), self)?;
        let mut uw_detect = false;
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
            let is_rb = handler.act_handle(act)?;
            if is_rb {
                uw_detect = true;
                break; // break earlier
            }
            act_iter.update();
        }
        if plain_stmt.is_return_stmt() {
            let ret_expr = handler.get_finalpu_while_update(pu_vec)?;
            Ok((Some(ret_expr), uw_detect))
        } else {
            handler.update_pu(pu_vec)?;
            Ok((None, uw_detect))
        }
    }

    /**
     * Handle segments in header of control flow structures like for-init, for-update
     */
    fn cfseg_handle(
        &self,
        cfseg: &SrcExpr,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &'a mut FuncActIter,
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
            let is_rb = handler.act_handle(act)?;
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
    fn cond_expr_handle(
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
            let is_rb = handler.act_handle(act)?;
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
    ) -> Result<(Vec<ProcessUnit>, Option<SrcVar>, bool)> {
        let func_node = exec_node_ptr.read().unwrap();
        let (is_recur_lock, enter_rb) = Self::check_recur_lock(&func_node)?;
        if is_recur_lock {
            return Ok((vec![], None, enter_rb));
        }

        let mut act_iter = func_node.iter();

        let mut pu_vec: Vec<ProcessUnit> = vec![];

        let mut src_iter = src_tree.iter();
        let mut stmt_ptr_op;
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
                            return Ok((pu_vec, None, true));
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

                    let (retexpr_op, is_rb) = self.plain_stmt_handle(
                        plain_stmt,
                        stmt_ptr.clone(),
                        &mut act_iter,
                        &mut pu_vec,
                    )?;
                    if is_rb {
                        let is_inner = Self::rollback_detect(&mut act_iter)?;
                        if is_inner {
                            return Ok((pu_vec, None, true));
                        } else {
                            // rollback exit handle
                            Self::rollback_exit_handle(src_tree, &mut act_iter, &mut src_iter)?;
                        }
                    } else {
                        if let Some(ret_expr) = retexpr_op {
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
                            return Ok((pu_vec, ret_var_op, false));
                        }
                    }
                }
            }
        }

        Ok((pu_vec, None, false))
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
        let func_node = func_node_ptr.read().unwrap();
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

        let (pu_vec, _, is_rb) = self.collect_intra(src_tree, func_node_ptr)?;
        assert!(
            !is_rb,
            "Top Level Function should not enter rollback status after intra collection"
        );
        Ok(pu_vec)
    }

    pub fn collect(&self) -> Result<Vec<ProcessUnit>> {
        let root_ptr = self.exec_forest.get_main_root_ptr();
        self.collect_recur(root_ptr)
    }
}
