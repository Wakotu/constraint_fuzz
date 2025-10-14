use crate::analysis::constraint::{
    inter::exec_tree::thread_tree::FuncActIter,
    intra::func_src_tree::{
        nodes::{
            cf_nodes::{SwitchArm, SwitchNode},
            SharedStmtNodePtr,
        },
        stmts::QLLoc,
    },
    stmt_collect::{inner_stmt::InnerStmtHandler, ProcessUnit, StmtCollector},
};

use color_eyre::eyre::Result;
use eyre::bail;

impl<'a> StmtCollector<'a> {
    fn switch_expr_handle(
        &self,
        switch_node: &SwitchNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<ProcessUnit> {
        let switch_expr = switch_node.get_expr();
        let mut handler = InnerStmtHandler::new(switch_expr.get_loc(), stmt_ptr.clone(), self)?;

        loop {
            let act_op = act_iter.next();
            let act = match act_op {
                None => break,
                Some(act) => act,
            };

            if !switch_expr.act_inner(act)? {
                break;
            }
            handler.act_handle(act)?;
        }
        let final_pu = handler.get_finalpu_while_update(pu_vec)?;
        Ok(final_pu)
    }

    // Returns loc of dest case
    fn get_dest_arm<'b>(
        &self,
        switch_node: &'b SwitchNode,
        act_iter: &mut FuncActIter,
    ) -> Result<&'b SwitchArm> {
        let act = match act_iter.next() {
            None => bail!("Switch Node Handle: No switch act"),
            Some(act) => act,
        };
        let switch_act = match act.get_switch_act() {
            None => bail!("Switch Node Handle: Not a switch act"),
            Some(switch_act) => switch_act,
        };
        if !switch_node.act_match(switch_act)? {
            bail!("Switch Node Handle: Switch act not match");
        }

        switch_node.get_dest_arm(&switch_act.dest_loc)
    }

    pub fn switch_node_handle(
        &self,
        switch_node: &SwitchNode,
        stmt_ptr: SharedStmtNodePtr,
        act_iter: &mut FuncActIter,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<Option<SharedStmtNodePtr>> {
        let expr_pu = self.switch_expr_handle(switch_node, stmt_ptr.clone(), act_iter, pu_vec)?;
        let arm = self.get_dest_arm(switch_node, act_iter)?;

        let cond_pu = arm.derive_cond_pu(expr_pu)?;
        pu_vec.push(cond_pu);
        Ok(arm.get_first_body_ptr())
    }
}
