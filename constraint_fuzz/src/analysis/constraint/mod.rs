use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    analysis::constraint::{
        exec_rec::ExecRec,
        inter::exec_tree::ExecForest,
        intra::func_src_tree::builder::FuncSrcForest,
        stmt_collect::{
            path_collect::RuntimePathCollector,
            runtime_path::{RuntimePathTree, ThreadRuntimePath},
        },
    },
    deopt::utils::buffer_read_to_bytes,
    execution::expe,
    feedback::branches::constraints::UBConstraint,
};
use color_eyre::eyre::Result;

pub mod exec_rec;
pub mod inter;
pub mod intra;

pub mod stmt_collect;

/**
 * This module is used to get dataflow information of a specified constraint.
 */

// define Dataflow info for constraitns: a set of Statements which results in variables in that Constraint

pub struct RevAnalyzer {
    ub_cons_list: Vec<UBConstraint>,
    // work_dir: PathBuf,
    exec_list: Vec<ExecRec>,
}

impl RevAnalyzer {
    /// constructs RevIterSolver with recoverable error
    // pub fn from_constraint<P: AsRef<Path>>(cons: &Constraint, work_dir: P) -> Result<Self> {
    //     let execs = cons.get_related_executions(work_dir.as_ref())?;
    //     Ok(Self {
    //         cons: cons.clone(),
    //         work_dir: work_dir.as_ref().to_path_buf(),
    //         execs,
    //     })
    // }

    /**
     * construction methods
     */
    fn get_ub_cons_list<P: AsRef<Path>>(expe_dir: P) -> Result<Vec<UBConstraint>> {
        let ub_cons_fpath = expe_dir.as_ref().join("constraints.json");
        let buf = buffer_read_to_bytes(&ub_cons_fpath)?;
        let ub_cons_list: Vec<UBConstraint> = serde_json::from_slice(&buf)?;
        Ok(ub_cons_list)
    }

    pub fn from_expe_dir<P: AsRef<Path>>(expe_dir: P) -> Result<Self> {
        let ub_cons_list = Self::get_ub_cons_list(expe_dir.as_ref())?;
        let exec_list = ExecRec::get_exec_list_from_expe_dir(expe_dir.as_ref())?;
        Ok(Self {
            ub_cons_list,
            // work_dir: expe_dir.as_ref().to_path_buf(),
            exec_list,
        })
    }
}

pub struct RtpTreeCollector {
    ub_cons: UBConstraint,
    func_src_forest: &'static FuncSrcForest,
    exec_forest_ptr: Arc<ExecForest>,
}

impl RtpTreeCollector {
    pub fn new(
        ub_cons: UBConstraint,
        func_src_forest: &'static FuncSrcForest,
        exec_forest_ptr: Arc<ExecForest>,
    ) -> Self {
        Self {
            ub_cons,
            func_src_forest,
            exec_forest_ptr,
        }
    }

    fn main_path_collect(&self) -> Result<Vec<ThreadRuntimePath>> {
        RuntimePathCollector::main_path_collect(self)
    }

    pub fn collect(&self) -> Result<RuntimePathTree> {
        // todo!()
        let path_vec = self.main_path_collect()?;
        let main_tid = self.exec_forest_ptr.get_main_tid()?;
        let rtp_tree = RuntimePathTree::from_path_vec(path_vec, main_tid);
        Ok(rtp_tree)
    }
}
