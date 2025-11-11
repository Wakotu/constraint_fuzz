use std::thread::{self, JoinHandle};

use crate::analysis::constraint::{
    inter::exec_tree::action::ThreadAction,
    stmt_collect::{
        path_collect::{inner_stmt::InnerStmtHandler, RuntimePathCollector},
        runtime_path::{PathRoot, ThreadRuntimePath},
        ProcessUnit,
    },
};
use color_eyre::eyre::Result;

pub type ThreadJoinHandle = JoinHandle<Vec<ThreadRuntimePath>>;

impl<'a> InnerStmtHandler<'a> {
    fn prepath_pu_construct(&mut self, thread_act: &ThreadAction) -> Result<Vec<ProcessUnit>> {
        let (_, left_idx) = self.stmt_info.get_startidxs_threadact(thread_act)?;

        let (arg_expr_vec, _) = self.arg_expr_collect(left_idx)?;
        let arg_expr = arg_expr_vec.get(3).ok_or_else(|| {
            eyre::eyre!("Number of Arg expressions inside a thread create invocation should be 4")
        })?;

        let tid = thread_act.get_thread_id();
        let exec_tree = self.rtp_cltr.exec_forest_ptr.get_thread_tree(tid)?;
        let entry_funcname = exec_tree.get_entry_funcname()?;

        let param_var_vec = self.func_paramvar_vec(&entry_funcname)?;
        let param_var = param_var_vec.get(0).ok_or_else(|| {
            eyre::eyre!("Target function of thread action should only have one parameter")
        })?;
        let pu = ProcessUnit::create_pre_func_assign_pu(arg_expr, param_var);
        Ok(vec![pu])
    }

    fn create_thread_rtpcltr(&self, thread_act: &ThreadAction) -> Result<RuntimePathCollector> {
        let tid = thread_act.get_thread_id();
        let exec_tree = self.rtp_cltr.exec_forest_ptr.get_thread_tree(tid)?;

        let rtp_cltr = RuntimePathCollector {
            tid,
            exec_forest_ptr: self.rtp_cltr.exec_forest_ptr.clone(),
            func_src_forest: self.rtp_cltr.func_src_forest,
            exec_root_ptr: exec_tree.get_root_ptr(),
            ub_cons: self.rtp_cltr.ub_cons.clone(),
            subhdl_vec: vec![],
        };
        Ok(rtp_cltr)
    }

    fn thread_path_root(&self) -> PathRoot {
        let len = self.pu_vec.len();
        let tid = self.rtp_cltr.tid;
        PathRoot { tid, len }
    }

    pub fn thread_invoc_handle(&mut self, thread_act: &ThreadAction) -> Result<ThreadJoinHandle> {
        self.pu_vec
            .push(ProcessUnit::new_thread_pu(thread_act.get_thread_id()));

        let prepath_puvec = self.prepath_pu_construct(thread_act)?;

        // create a new PathCollector instance
        let mut rtp_cltr = self.create_thread_rtpcltr(thread_act)?;
        let tid = thread_act.get_thread_id();
        let path_root = self.thread_path_root();

        // Spawn new path collector
        let join_handle = thread::spawn(move || {
            let pu_vec = rtp_cltr
                .path_collect(prepath_puvec)
                .expect("Failed to collect runtime path for sub thread");

            let cur_path = ThreadRuntimePath::thread_path_construct(tid, path_root, pu_vec);
            rtp_cltr.extend_sub_pathvecs(cur_path)
        });
        Ok(join_handle)
    }
}
