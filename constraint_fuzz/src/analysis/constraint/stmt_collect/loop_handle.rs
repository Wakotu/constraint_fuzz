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

    /**
     * Consume loop hit act or (exceed, Out) act, and return whether exceed happens
     */
    fn handle_loopentry(loop_part: &LoopPart, act_iter: &mut FuncActIter) -> Result<bool> {
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
            Self::loopout_act_consume_and_check(act_iter)?;
        }
        Ok(is_exceed)
    }

    fn handle_while_loopentry(while_node: &WhileNode, act_iter: &mut FuncActIter) -> Result<bool> {
        Self::handle_loopentry(&while_node.derive_loop_part(), act_iter)
    }

    fn normal_loop_hanlde(
        &self,
        loop_part: LoopPart<'_>,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
        // Handle entry act: hit and exceed
        let is_exceed = Self::handle_loopentry(&loop_part, act_iter)?;
        if is_exceed {
            let next_ptr_op = FuncSrcTreeIter::get_after_next_ptr(stmt_ptr.clone())?;
            return Ok(next_ptr_op);
        }

        // handle cond expr
        let cond_expr = match loop_part.get_cond_op() {
            Some(expr) => expr,
            None => {
                // go into body
                return Ok(Some(loop_part.get_body_ptr()));
            }
        };
        let outer_act = self.cond_expr_handle(cond_expr, stmt_ptr.clone(), act_iter, pu_vec)?;

        // get next ptr based on outer act
        let body_ptr_op = loop_part.get_dest_body(outer_act)?;
        let next_ptr_op = match body_ptr_op {
            Some(ptr) => Some(ptr),
            None => {
                // normal Loop Out handle
                Self::loopout_act_consume_and_check(act_iter)?;
                FuncSrcTreeIter::get_after_next_ptr(stmt_ptr.clone())?
            }
        };
        Ok(next_ptr_op)
    }

    fn normal_while_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
        let loop_part = while_node.derive_loop_part();
        self.normal_loop_hanlde(loop_part, stmt_ptr, act_iter, pu_vec)
    }

    fn do_while_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
        if while_node.is_first_visit() {
            // before arrive
            let is_exceed = Self::handle_while_loopentry(while_node, act_iter)?;
            if is_exceed {
                bail!("Do stmt handle: do while stmt should not exceed at first visit")
            }
            let body_ptr = while_node.get_body_ptr();
            Ok(Some(body_ptr))
        } else {
            // inner arrive
            // begin with cond expr handle
            let cond_expr = while_node.get_cond_expr();
            let outer_act = self.cond_expr_handle(cond_expr, stmt_ptr.clone(), act_iter, pu_vec)?;

            // get next ptr based on outer act
            let body_ptr_op = while_node.get_dest_body(outer_act)?;
            let next_ptr_op = match body_ptr_op {
                Some(ptr) => {
                    let is_exceed = Self::handle_while_loopentry(while_node, act_iter)?;

                    if is_exceed {
                        let next_ptr_op = FuncSrcTreeIter::get_after_next_ptr(stmt_ptr.clone())?;
                        next_ptr_op
                    } else {
                        Some(ptr)
                    }
                }
                None => {
                    // normal Loop Out handle
                    Self::loopout_act_consume_and_check(act_iter)?;
                    FuncSrcTreeIter::get_after_next_ptr(stmt_ptr.clone())?
                }
            };
            Ok(next_ptr_op)
        }
    }

    pub fn while_node_handle(
        &self,
        while_node: &WhileNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
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
    ) -> Result<()> {
        // check first visit
        if !for_node.is_first_visit() {
            return Ok(());
        }

        // get init seg
        let init_seg = match for_node.init {
            None => return Ok(()),
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
    ) -> Result<()> {
        if for_node.is_first_visit() {
            return Ok(());
        }

        let updt_seg = match for_node.update {
            None => return Ok(()),
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
    ) -> Result<Option<SharedStmtNodePtr>> {
        let loop_part = for_node.derive_loop_part();
        self.normal_loop_hanlde(loop_part, stmt_ptr, act_iter, pu_vec)
    }

    pub fn for_node_handle(
        &self,
        for_node: &ForNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
        self.for_init_handle(for_node, stmt_ptr.clone(), act_iter, pu_vec)?;
        self.for_update_handle(for_node, stmt_ptr.clone(), act_iter, pu_vec)?;
        self.for_cond_handle(for_node, stmt_ptr, act_iter, pu_vec)
    }
}
