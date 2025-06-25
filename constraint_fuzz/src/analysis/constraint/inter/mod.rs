use rayon::prelude::*;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    analysis::constraint::inter::tree::ExecTree,
    deopt::utils::{
        buffer_read_to_bytes, create_dir_if_nonexist, get_basename_str_from_path, get_parent_dir,
    },
    feedback::{
        branches::constraints::Constraint,
        clang_coverage::{BranchCount, CodeCoverage},
    },
};

use color_eyre::eyre::Result;

use super::RevIterSolver;

pub mod tree;

/**
 * This module is used to get function call chain from entry to constraints (inter-procedural analysis)
 */

// Define executions as exec name + cov + func stack. (file path or value)
#[derive(Debug, Clone)]
pub struct ExecRec {
    exec_name: String,
    // execution guard dir
    execg_dir: PathBuf,
    cov_path: PathBuf,
}

impl ExecRec {
    // based on expe pipeline
    pub fn get_exec_msg_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = work_dir.join("exec_recs");
        create_dir_if_nonexist(&msg_dir)?;
        Ok(msg_dir)
    }

    // based on the exec object
    pub fn get_exec_cov_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let sg_cov_dir = msg_dir.join("cov");
        create_dir_if_nonexist(&sg_cov_dir)?;
        Ok(sg_cov_dir)
    }

    // based on the exec object
    pub fn get_func_stack_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let fs_dir = msg_dir.join("func_br_stack");
        create_dir_if_nonexist(&fs_dir)?;
        Ok(fs_dir)
    }

    pub fn setup_exec_dir(work_dir: &Path) -> Result<(PathBuf, PathBuf)> {
        // create coverage directory
        let cov_dir = Self::get_exec_cov_dir(work_dir)?;
        // create func stack directory
        let fs_dir = Self::get_func_stack_dir(work_dir)?;

        Ok((fs_dir, cov_dir))
    }

    pub fn get_exec_list_from_work_dir(work_dir: &Path) -> Result<Vec<Self>> {
        let exec_cov_dir = Self::get_exec_cov_dir(work_dir)?;
        let mut exec_list = vec![];

        // iterate over all files in the coverage directory
        for ent_res in fs::read_dir(&exec_cov_dir)? {
            let entry = ent_res?;
            let fpath = entry.path();
            if fpath.is_file() {
                let exec = Self::from_cov_path(&fpath)?;
                exec_list.push(exec);
            }
        }

        Ok(exec_list)
    }

    pub fn from_cov_path(cov_path: &Path) -> Result<Self> {
        let exec_name = get_basename_str_from_path(cov_path)?;

        let cov_dir = get_parent_dir(cov_path)?;
        let fs_dir = get_parent_dir(cov_dir)?.join("func_stack").join(&exec_name);

        Ok(Self {
            exec_name,
            execg_dir: fs_dir,
            cov_path: cov_path.to_owned(),
        })
    }

    pub fn from_fs_dir(fs_dir: &Path) -> Result<Self> {
        let exec_name = get_basename_str_from_path(fs_dir)?;
        let exec_dir = get_parent_dir(fs_dir)?;
        let cov_dir = exec_dir.join("cov");
        let cov_path = cov_dir.join(&exec_name);
        assert!(
            cov_path.is_file(),
            "Coverage file does not exist: {}",
            cov_path.display()
        );

        Ok(Self {
            exec_name,
            execg_dir: fs_dir.to_owned(),
            cov_path,
        })
    }
}

impl CodeCoverage {
    pub fn contains_cons(&self, cons: &Constraint) -> Result<bool> {
        let func_name = cons.get_func_name()?;
        let fpath = &cons.fpath;
        for func in self.iter_function_covs() {
            if func.get_name() != func_name {
                continue;
            }
            for br in func.branches.iter() {
                let cov_fpath = func.get_source_file_path_by_cov_branch(br)?;
                let rng = br.get_range()?;
                // checks source file path and range equivalence
                if &cov_fpath == fpath && rng == cons.range {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl RevIterSolver {
    fn exec_contains_cons(
        idx: usize,
        len: usize,
        exec: &ExecRec,
        cons: &Constraint,
    ) -> Result<bool> {
        log::debug!(
            "Processing execution {}/{}: {}",
            idx + 1,
            len,
            exec.exec_name
        );
        let json_slice = buffer_read_to_bytes(&exec.cov_path)?;
        let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;
        let flag = cov.contains_cons(cons)?;
        if flag {
            log::debug!("Execution {} contains constraint", exec.exec_name);
        } else {
            log::debug!("Execution {} does not contain constraint", exec.exec_name);
        }

        Ok(flag)
    }

    pub fn get_related_executions(&self) -> Result<Vec<ExecRec>> {
        let exec_list = ExecRec::get_exec_list_from_work_dir(&self.work_dir)?;
        let res_list = exec_list
            .par_iter()
            .enumerate()
            .filter_map(|(idx, exec)| {
                match Self::exec_contains_cons(idx, exec_list.len(), exec, &self.cons) {
                    Ok(true) => Some(exec.to_owned()),
                    Ok(false) => None,
                    Err(e) => {
                        // log::warn!("Error processing execution {}: {}", exec.exec_name, e);
                        // None
                        panic!("Error processing execution {}: {}", exec.exec_name, e);
                    }
                }
            })
            .collect();
        Ok(res_list)
    }

    // fn get_related_executions_with_num(&self, num: Option<usize>) -> Result<Vec<ExecRec>> {
    //     let exec_list = ExecRec::get_exec_list_from_work_dir(&self.work_dir)?;
    //     let mut res_list = vec![];

    //     for (idx, exec) in exec_list.iter().enumerate() {
    //         log::debug!(
    //             "Processing execution {}/{}: {}",
    //             idx + 1,
    //             exec_list.len(),
    //             exec.exec_name
    //         );

    //         let json_slice = buffer_read_to_bytes(&exec.cov_path)?;
    //         let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;

    //         if cov.contains_cons(&self.cons)? {
    //             log::debug!("exec: {} related and collected", exec.exec_name);
    //             res_list.push(exec.to_owned());
    //         } else {
    //             log::debug!("exec: {} not related", exec.exec_name);
    //         }

    //         // check len
    //         if let Some(num) = num {
    //             if res_list.len() >= num {
    //                 break;
    //             }
    //         }
    //     }
    //     Ok(res_list)
    // }

    // iterate over all files in the coverage directory
    // judge related based on cov and then constructs the related ones only
    // pub fn get_related_executions(&self) -> Result<Vec<ExecRec>> {
    //     self.get_related_executions_with_num(None)
    // }

    /// exec -> threads -> func stack list -> func chain list
    pub fn extract_exec_trees_from_rec(&self, exec: &ExecRec) -> Result<Vec<ExecTree>> {
        let mut tree_list: Vec<ExecTree> = vec![];
        for ent_res in fs::read_dir(&exec.execg_dir)? {
            let entry = ent_res?;
            let fs_path = entry.path();
            assert!(fs_path.is_file(), "Expected a file: {}", fs_path.display());
            let tree = ExecTree::from_guard_file(&fs_path, &self.cons)?;
            // may need some non-empty check
            tree_list.push(tree);
        }
        Ok(tree_list)
    }

    pub fn get_all_exec_trees(&self) -> Result<Vec<ExecTree>> {
        let execs = self.get_related_executions()?;
        let mut res_tree_list = vec![];
        for exec in execs {
            let chain_list = self.extract_exec_trees_from_rec(&exec)?;
            assert!(
                !chain_list.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            res_tree_list.extend(chain_list);
        }
        // deduplicate_unordered(&mut res_chain_list);
        Ok(res_tree_list)
    }
}

/**
 * unit tests
 */
#[cfg(test)]
mod tests {
    use crate::init_report_utils_for_tests;

    use super::*;

    fn setup_test_consdf_builder() -> Result<RevIterSolver> {
        let work_dir =
            "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-06-24 10:30:27";
        let cons_path = Path::new(work_dir).join("constraints.json");
        let json_slice = buffer_read_to_bytes(&cons_path);
        let cons_list: Vec<Constraint> = serde_json::from_slice(&json_slice?)?;
        let builder = RevIterSolver::new(&cons_list[0], work_dir);
        Ok(builder)
    }

    #[test]
    fn test_get_related_executions() -> Result<()> {
        init_report_utils_for_tests()?;
        let builder = setup_test_consdf_builder()?;
        let exec_list = builder.get_related_executions()?;
        log::debug!("exec_list: {:?}", exec_list);

        Ok(())
    }

    #[test]
    fn test_exec_tree_construction() -> Result<()> {
        init_report_utils_for_tests()?;
        let builder = setup_test_consdf_builder()?;

        let execs = builder.get_related_executions()?;
        let mut res_chain_list = vec![];
        for exec in execs {
            let tree_list = builder.extract_exec_trees_from_rec(&exec)?;
            assert!(
                !tree_list.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            // log::debug!("Function chain for {}: {:?}", exec.exec_name, chain_list);
            res_chain_list.extend(tree_list);
        }
        todo!()
        // deduplicate_unordered(&mut res_chain_list);

        // log::debug!("Total function chains extracted: {}", res_chain_list.len());
        // log::debug!("Function chains: {:?}", res_chain_list);
        // Ok(())
    }
}
