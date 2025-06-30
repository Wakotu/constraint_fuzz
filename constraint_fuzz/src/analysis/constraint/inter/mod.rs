use rayon::prelude::*;

use std::fmt;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    analysis::constraint::inter::{loc::SrcRegion, tree::ExecTree},
    config::is_debug_mode,
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

pub mod loc;

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

impl fmt::Display for ExecRec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExecRec({})", self.exec_name)
    }
}

impl ExecRec {
    /**
     * exec_* means global path, sg_* means path for single ExecRec instance
     */

    // based on expe pipeline
    pub fn get_exec_msg_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = work_dir.join("exec_recs");
        create_dir_if_nonexist(&msg_dir)?;
        Ok(msg_dir)
    }

    // based on the exec object
    pub fn get_exec_cov_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let cov_dir = msg_dir.join("cov");
        create_dir_if_nonexist(&cov_dir)?;
        Ok(cov_dir)
    }

    pub fn get_sg_guard_dir(work_dir: &Path, exec_name: &str) -> Result<PathBuf> {
        let guard_dir = Self::get_exec_guard_dir(work_dir)?;
        let sg_guard_dir = guard_dir.join(exec_name);
        create_dir_if_nonexist(&sg_guard_dir)?;
        Ok(sg_guard_dir)
    }

    // based on the exec object
    pub fn get_exec_guard_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let fs_dir = msg_dir.join("guards");
        create_dir_if_nonexist(&fs_dir)?;
        Ok(fs_dir)
    }

    pub fn setup_exec_dir(work_dir: &Path) -> Result<(PathBuf, PathBuf)> {
        // create coverage directory
        let cov_dir = Self::get_exec_cov_dir(work_dir)?;
        // create func stack directory
        let fs_dir = Self::get_exec_guard_dir(work_dir)?;

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

    fn get_work_dir_from_cov_path(cov_path: &Path) -> Result<PathBuf> {
        let cov_dir = get_parent_dir(cov_path)?;
        let msg_dir = get_parent_dir(&cov_dir)?;
        let work_dir = get_parent_dir(&msg_dir)?;
        Ok(work_dir)
    }

    pub fn from_cov_path(cov_path: &Path) -> Result<Self> {
        let exec_name = get_basename_str_from_path(cov_path)?;

        let work_dir = Self::get_work_dir_from_cov_path(cov_path)?;
        let sg_guard_dir = Self::get_sg_guard_dir(&work_dir, &exec_name)?;

        Ok(Self {
            exec_name,
            execg_dir: sg_guard_dir.to_owned(),
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

    pub fn get_coverage(&self) -> Result<CodeCoverage> {
        let json_slice = buffer_read_to_bytes(&self.cov_path)?;
        let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;
        Ok(cov)
    }

    // pub fn contains_constraint(&self, cons: &Constraint) -> Result<bool> {
    //     let json_slice = buffer_read_to_bytes(&self.cov_path)?;
    //     let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;
    //     cov.contains_cons(cons)
    // }
}

impl CodeCoverage {
    pub fn contains_cons(&self, cons: &Constraint) -> Result<bool> {
        let func_name = cons.get_func_name()?;
        let fpath = &cons.fpath;
        for func in self.iter_function_covs() {
            if func.get_name() != func_name {
                continue;
            }
            for cov_br in func.iter_cov_branches() {
                let cov_fpath = func.get_source_file_path_by_cov_branch(cov_br)?;
                let rng = cov_br.get_range()?;
                // checks source file path and range equivalence
                if &cov_fpath == fpath && rng == cons.range {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// get all br regions inside the same function with specified constraint
    pub fn get_related_br_regions(&self, cons: &Constraint) -> Result<Vec<SrcRegion>> {
        let mut br_rgn_list = vec![];
        for cov_func in self.iter_function_covs() {
            for br_rgn in cov_func.get_br_regions()? {
                if br_rgn.is_related_to_cons(cons)? {
                    br_rgn_list.push(br_rgn);
                }
            }
        }
        Ok(br_rgn_list)
    }
}

impl RevIterSolver {
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
        log::warn!("Guard Directory: {}", exec.execg_dir.display());

        for ent_res in fs::read_dir(&exec.execg_dir)? {
            let entry = ent_res?;
            let guard_fpath = entry.path();
            assert!(
                guard_fpath.is_file(),
                "Expected a file: {}",
                guard_fpath.display()
            );
            let tree = ExecTree::from_guard_file(&guard_fpath, &self.cons)?;
            // may need some non-empty check

            if is_debug_mode() {
                log::debug!("Guard file: {}", guard_fpath.display());
                log::debug!("{:?}", tree);
            }

            tree_list.push(tree);
        }
        Ok(tree_list)
    }

    pub fn iter_execs(&self) -> impl Iterator<Item = &ExecRec> {
        self.execs.iter()
    }

    pub fn get_exec_trees_for_expe(&self) -> Result<Vec<ExecTree>> {
        // let execs = self.get_related_executions()?;
        let mut res_tree_list = vec![];
        for exec in self.execs.iter() {
            let chain_list = self.extract_exec_trees_from_rec(exec)?;
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
    use std::{
        collections::HashMap,
        fs::File,
        io::{BufRead, BufReader},
    };

    use base64::read;
    use eyre::bail;

    use crate::{analysis::constraint::inter::tree::FuncActionType, setup_test_run_entry};

    use super::*;

    fn setup_test_solver() -> Result<RevIterSolver> {
        let work_dir =
            "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-06-26 12:10:38";
        let cons_path = Path::new(work_dir).join("constraints.json");
        let json_slice = buffer_read_to_bytes(&cons_path);
        let cons_list: Vec<Constraint> = serde_json::from_slice(&json_slice?)?;
        log::info!(
            "Loaded {} constraints from {}",
            cons_list.len(),
            cons_path.display()
        );

        for (idx, cons) in cons_list.iter().enumerate() {
            log::info!(
                "Processing constraint {}/{}: {}",
                idx + 1,
                cons_list.len(),
                cons
            );
            let builder_res = RevIterSolver::from_constraint(cons, work_dir);
            match builder_res {
                Ok(builder) => {
                    if is_debug_mode() {
                        log::info!(
                            "RevIterSolver initialized successfully at constraint {}: {}.",
                            idx + 1,
                            cons
                        );
                    }
                    return Ok(builder);
                }
                Err(e) => {
                    log::warn!("Failed to initialize RevIterSolver: {}. Retrying...", e);
                    continue;
                }
            }
        }
        bail!("Failed to initialize RevIterSolver after processing all constraints.");
    }

    #[test]
    fn test_get_related_executions() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let _solver = setup_test_solver()?;
        // // let exec_list = solver.get_related_executions()?;
        // log::debug!("exec_list: {:?}", exec_list);

        Ok(())
    }

    #[test]
    fn test_exec_tree_construction_one() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let solver = setup_test_solver()?;

        for exec in solver.iter_execs() {
            let tree_list = solver.extract_exec_trees_from_rec(&exec)?;
            assert!(
                !tree_list.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            // log::debug!("Function chain for {}: {:?}", exec.exec_name, chain_list);
            break;
        }
        Ok(())
    }

    #[test]
    fn test_exec_tree_construction() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let solver = setup_test_solver()?;

        let mut res_chain_list = vec![];
        for exec in solver.iter_execs() {
            let tree_list = solver.extract_exec_trees_from_rec(&exec)?;
            assert!(
                !tree_list.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            // log::debug!("Function chain for {}: {:?}", exec.exec_name, chain_list);
            res_chain_list.extend(tree_list);
        }
        Ok(())
    }

    #[test]
    fn test_guard_file_function_count() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let gaurd_fpath = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-06-26 12:10:38/exec_recs/guards/9dae87ac037f2dffb8ab66e59c827dc8/139690227110080_main";

        let file = File::open(gaurd_fpath)?;
        let reader = BufReader::new(file);

        let mut func_count_dict = HashMap::new();
        for (idx, line_res) in reader.lines().enumerate() {
            let line = line_res?;

            let func_name = match FuncActionType::get_func_name(&line) {
                Ok(name) => name,
                Err(e) => {
                    log::error!(
                        "Failed to get function name from line {}: {}",
                        idx + 1,
                        line
                    );
                    log::error!("Error: {}", e);
                    continue;
                }
            };
            log::debug!("Line {}: {}", idx + 1, func_name);
            *func_count_dict.entry(func_name.to_owned()).or_insert(0) += 1;
        }

        // sort the function counts in descending order
        let mut func_count_vec: Vec<_> = func_count_dict.into_iter().collect();
        func_count_vec.sort_by(|a, b| b.1.cmp(&a.1));

        // output first 10 pairs
        log::info!("Function counts (top 10):");
        for (func_name, count) in func_count_vec.iter().take(10) {
            log::info!("{}: {}", func_name, count);
        }

        Ok(())
    }
}
