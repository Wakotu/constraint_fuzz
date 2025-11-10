use crate::analysis::constraint::{
    inter::exec_tree::thread_tree::FuncActIter,
    intra::func_src_tree::nodes::{FuncSrcTree, FuncSrcTreeIter},
    stmt_collect::path_collect::StmtCollector,
};
use color_eyre::eyre::Result;
use eyre::bail;

impl<'a> StmtCollector<'a> {
    /// Returns if is inner rollback
    pub fn rollback_detect(act: &mut FuncActIter) -> Result<bool> {
        let act = act.get_cur().ok_or_else(|| {
            eyre::eyre!("Rollback Detect: should have subsequent actions at rollback unwind")
        })?;
        if act.is_unwind() {
            Ok(true)
        } else if act.is_postsj() {
            Ok(false)
        } else {
            bail!("Rollback Detect: unexpected action type at rollback unwind")
        }
    }

    pub fn rollback_exit_handle(
        src_tree: &FuncSrcTree,
        act_iter: &mut FuncActIter,
        src_iter: &mut FuncSrcTreeIter<'_>,
    ) -> Result<()> {
        let act = act_iter.next().ok_or_else(|| {
            eyre::eyre!("Rollback Exit: there  should be subsequent action at rollback exit")
        })?;
        let sj_act = act.derive_postsj_act().ok_or_else(|| {
            eyre::eyre!("Rollback Exit: expected post setjmp action at rollback exit")
        })?;
        let next_ptr = src_tree.get_nextptr_by_rbexit(sj_act)?;
        src_iter.update(Some(next_ptr));
        Ok(())
    }
}
