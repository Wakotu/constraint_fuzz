use crate::analysis::constraint::{
    inter::exec_tree::{
        action::{ExecAction, LoopActionType},
        thread_tree::FuncActIter,
    },
    intra::func_src_tree::{
        nodes::{
            cf_nodes::{ForNode, LoopPart, WhileNode},
            FuncSrcTreeIter, SharedStmtNodePtr,
        },
        stmts::WhileType,
    },
    stmt_collect::ProcessUnit,
};

use super::StmtCollector;
use color_eyre::eyre::Result;
use eyre::bail;

impl<'a> StmtCollector<'a> {
    fn loopout_act_consume_and_check(act_iter: &mut FuncActIter) -> Result<()> {
        let loop_out_act = match act_iter.next() {
            None => bail!("Normal While Node Handle: loop out action should be present"),
            Some(act) => act,
        };
        if !loop_out_act.is_normal_loopend() {
            bail!("Normal While Node Handle: loop out action should be normal loop")
        }
        Ok(())
    }

    // Return: if enters rollback status
    fn looplock_handle(act_iter: &mut FuncActIter) -> Result<bool> {
        // consume two loop lock actions
        let loop_lock_act = act_iter.next_res()?;
        assert!(
            loop_lock_act.is_loop_lock(),
            "Loop Lock Handle: action should be loop lock"
        );
        let loop_release_act = act_iter.next_res()?;
        assert!(
            loop_release_act.is_loop_release(),
            "Loop Lock Handle: action should be loop"
        );
        // edge detection
        let next_act = act_iter.get_cur().ok_or_else(|| {
            eyre::eyre!("Loop Lock Handle: next action should be present after loop release")
        })?;
        if next_act.is_unwind() {
            // consume the unwind
            act_iter.update();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Return : if enters rollback status
    fn loopexceed_handle(act_iter: &mut FuncActIter) -> Result<bool> {
        let is_rb = Self::looplock_handle(act_iter)?;
        if is_rb {
            Ok(true)
        } else {
            Self::loopout_act_consume_and_check(act_iter)?;
            Ok(false)
        }
    }

    /**
     * Consume loop hit act or (exceed, Out) act, and return whether exceed happens
     */
    fn handle_loopentry(loop_part: &LoopPart, act_iter: &mut FuncActIter) -> Result<(bool, bool)> {
        let first_act = match act_iter.next() {
            None => bail!("Normal While Node handle: entry action should not exceed index"),
            Some(act) => act,
        };
        let loop_act = match first_act {
            ExecAction::Loop(loop_act) => loop_act,
            _ => bail!("Normal While Node Handle: first action should be of type Loop Action"),
        };
        // header loc check
        if !loop_part.src_loc_inner(&loop_act.header_loc) {
            bail!("Normal While Node Handle: header loc mismatch")
        }
        // check loop entry act
        let entry_act = match &loop_act.la_type {
            LoopActionType::LoopEntry(entry_act) => entry_act,
            _ => bail!("Normal While Node Handle: loop entry action should be of type LoopEntry"),
        };
        let is_exceed = entry_act.is_exceed();
        if is_exceed {
            // exceed handle
            let is_rb = Self::loopexceed_handle(act_iter)?;
            if is_rb {
                return Ok((true, true));
            }
        }
        Ok((is_exceed, false))
    }

    fn handle_while_loopentry(
        while_node: &WhileNode,
        act_iter: &mut FuncActIter,
    ) -> Result<(bool, bool)> {
        Self::handle_loopentry(&while_node.derive_loop_part(), act_iter)
    }

    // Return trailing bool: if enters rollback status
    fn loopheader_handle_with_condexpr(
        &self,
        loop_part: LoopPart<'_>,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        // Handle entry act: hit and exceed
        let (is_exceed, is_rb) = Self::handle_loopentry(&loop_part, act_iter)?;
        if is_rb {
            // in rollback status
            return Ok((None, true));
        }
        if is_exceed {
            let next_ptr_op = FuncSrcTreeIter::get_afternext_ptr(stmt_ptr.clone())?;
            // TODO: add loop lock handle
            return Ok((next_ptr_op, false));
        }

        // handle cond expr
        let cond_expr = match loop_part.get_cond_op() {
            Some(expr) => expr,
            None => {
                // go into body
                return Ok((Some(loop_part.get_body_ptr()), false));
            }
        };
        let (outer_act_op, is_rb) =
            self.cond_expr_handle(cond_expr, stmt_ptr.clone(), act_iter, pu_vec)?;
        if is_rb {
            return Ok((None, true));
        }
        let outer_act = outer_act_op.ok_or_else(|| {
            eyre::eyre!("Loop Header Handle: should have outer action after cond expr")
        })?;

        // get next ptr based on outer act
        let body_ptr_op = loop_part.get_dest_body(outer_act)?;
        let next_ptr_op = match body_ptr_op {
            Some(ptr) => Some(ptr),
            None => {
                // normal Loop Out handle
                Self::loopout_act_consume_and_check(act_iter)?;
                FuncSrcTreeIter::get_afternext_ptr(stmt_ptr.clone())?
            }
        };
        Ok((next_ptr_op, false))
    }

    fn normal_while_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        let loop_part = while_node.derive_loop_part();
        self.loopheader_handle_with_condexpr(loop_part, stmt_ptr, act_iter, pu_vec)
    }

    /// Can alse be seen as loop header handle without condexpr
    fn do_while_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        if while_node.is_first_visit() {
            // before arrive
            let (is_exceed, is_rb) = Self::handle_while_loopentry(while_node, act_iter)?;
            if is_exceed {
                bail!("Do stmt handle: do while stmt should not exceed at first visit")
            }
            if is_rb {
                bail!("Do stmt handle: do while stmt should not rollback at first visit")
            }

            let body_ptr = while_node.get_body_ptr();
            Ok((Some(body_ptr), false))
        } else {
            // inner arrive
            // begin with cond expr handle
            let cond_expr = while_node.get_cond_expr();
            let (outer_act_op, is_rb) =
                self.cond_expr_handle(cond_expr, stmt_ptr.clone(), act_iter, pu_vec)?;
            if is_rb {
                return Ok((None, true));
            }
            let outer_act = outer_act_op.ok_or_else(|| {
                eyre::eyre!("Loop Header Handle: should have outer action after cond expr")
            })?;

            // get next ptr based on outer act
            let body_ptr_op = while_node.get_dest_body(outer_act)?;
            let next_ptr_op = match body_ptr_op {
                Some(ptr) => {
                    let (is_exceed, is_rb) = Self::handle_while_loopentry(while_node, act_iter)?;
                    if is_rb {
                        return Ok((None, true));
                    }

                    if is_exceed {
                        let next_ptr_op = FuncSrcTreeIter::get_afternext_ptr(stmt_ptr.clone())?;
                        next_ptr_op
                    } else {
                        Some(ptr)
                    }
                }
                None => {
                    // normal Loop Out handle
                    Self::loopout_act_consume_and_check(act_iter)?;
                    FuncSrcTreeIter::get_afternext_ptr(stmt_ptr.clone())?
                }
            };
            Ok((next_ptr_op, false))
        }
    }

    pub fn while_node_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        match &while_node.while_type {
            WhileType::While => self.normal_while_handle(while_node, stmt_ptr, act_iter, pu_vec),
            WhileType::Do => self.do_while_handle(while_node, stmt_ptr, act_iter, pu_vec),
        }
    }

    // cfseg handle: no need to return next stmt ptr
    fn for_init_handle(
        &self,
        for_node: &ForNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<bool> {
        // check first visit
        if !for_node.is_first_visit() {
            return Ok(false);
        }

        // get init seg
        let init_seg = match for_node.init {
            None => return Ok(false),
            Some(ref seg) => seg,
        };
        self.cfseg_handle(init_seg, stmt_ptr, act_iter, pu_vec)
    }

    fn for_update_handle(
        &self,
        for_node: &ForNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<bool> {
        if for_node.is_first_visit() {
            return Ok(false);
        }

        let updt_seg = match for_node.update {
            None => return Ok(false),
            Some(ref seg) => seg,
        };
        self.cfseg_handle(updt_seg, stmt_ptr, act_iter, pu_vec)
    }

    fn for_cond_handle(
        &self,
        for_node: &ForNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        let loop_part = for_node.derive_loop_part();
        self.loopheader_handle_with_condexpr(loop_part, stmt_ptr, act_iter, pu_vec)
    }

    pub fn for_node_handle(
        &self,
        for_node: &ForNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<(Option<SharedStmtNodePtr>, bool)> {
        let is_rb = self.for_init_handle(for_node, stmt_ptr.clone(), act_iter, pu_vec)?;
        if is_rb {
            return Ok((None, true));
        }
        let is_rb = self.for_update_handle(for_node, stmt_ptr.clone(), act_iter, pu_vec)?;
        if is_rb {
            return Ok((None, true));
        }
        self.for_cond_handle(for_node, stmt_ptr, act_iter, pu_vec)
    }
}
