// use rayon::prelude::*;

use std::fmt;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::analysis::constraint::exec_rec::ExecRec;
use crate::{
    analysis::constraint::inter::{exec_tree::thread_tree::ThreadTree, loc::SrcRegion},
    config::is_debug_mode,
    deopt::utils::{
        buffer_read_to_bytes, create_dir_if_nonexist, get_basename_str_from_path, get_parent_dir,
    },
    feedback::{
        branches::constraints::UBConstraint,
        clang_coverage::{BranchCount, CodeCoverage},
    },
};

use color_eyre::eyre::Result;

use super::RevAnalyzer;

pub mod error;
pub mod exec_tree;
pub mod loc;

/**
 * This module is used to get function call chain from entry to constraints (inter-procedural analysis)
 */

// Define executions as exec name + cov + func stack. (file path or value)

impl CodeCoverage {
    pub fn contains_cons(&self, cons: &UBConstraint) -> Result<bool> {
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
    pub fn get_related_br_regions(&self, cons: &UBConstraint) -> Result<Vec<SrcRegion>> {
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

impl RevAnalyzer {
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

    pub fn iter_execs(&self) -> impl Iterator<Item = &ExecRec> {
        self.exec_list.iter()
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

    use eyre::bail;
    use walkdir::WalkDir;

    use crate::{
        analysis::constraint::inter::exec_tree::{action::FuncActionType, ExecForest},
        deopt::utils::{get_file_lineno, timer_it},
        setup_test_run_entry,
    };

    use super::*;

    // fn setup_test_solver() -> Result<RevAnalyzer> {
    //     let work_dir =
    //         "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-06-26 12:10:38";
    //     let cons_path = Path::new(work_dir).join("constraints.json");
    //     let json_slice = buffer_read_to_bytes(&cons_path);
    //     let cons_list: Vec<Constraint> = serde_json::from_slice(&json_slice?)?;
    //     log::info!(
    //         "Loaded {} constraints from {}",
    //         cons_list.len(),
    //         cons_path.display()
    //     );

    //     for (idx, cons) in cons_list.iter().enumerate() {
    //         log::info!(
    //             "Processing constraint {}/{}: {}",
    //             idx + 1,
    //             cons_list.len(),
    //             cons
    //         );
    //         let builder_res = RevAnalyzer::from_constraint(cons, work_dir);
    //         match builder_res {
    //             Ok(builder) => {
    //                 if is_debug_mode() {
    //                     log::info!(
    //                         "RevIterSolver initialized successfully at constraint {}: {}.",
    //                         idx + 1,
    //                         cons
    //                     );
    //                 }
    //                 return Ok(builder);
    //             }
    //             Err(e) => {
    //                 log::warn!("Failed to initialize RevIterSolver: {}. Retrying...", e);
    //                 continue;
    //             }
    //         }
    //     }
    //     bail!("Failed to initialize RevIterSolver after processing all constraints.");
    // }

    // #[test]
    // fn test_get_related_executions() -> Result<()> {
    //     setup_test_run_entry("libaom", true)?;
    //     let _solver = setup_test_solver()?;
    //     // // let exec_list = solver.get_related_executions()?;
    //     // log::debug!("exec_list: {:?}", exec_list);

    //     Ok(())
    // }

    #[test]
    fn test_largest_guard_file() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let guard_dir = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-07-22 22:55:29/exec_recs/guards";

        let mut largest_file = None;
        let mut largest_size = 0;

        for ent_res in WalkDir::new(guard_dir) {
            let entry = ent_res?;
            let fpath = entry.path();
            if fpath.is_file() {
                let lines = get_file_lineno(fpath)?;
                if lines > largest_size {
                    largest_size = lines;
                    largest_file = Some(fpath.to_owned());
                }
            }
        }
        let largest_file = largest_file.ok_or_else(|| eyre::eyre!("No guard files found"))?;
        log::info!("Largest guard file: {:?}", largest_file);
        log::info!("Number of lines: {}", largest_size);

        Ok(())
    }

    #[test]
    // Forest analyze
    fn test_exec_forest_analyze() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        // 479w+ lines
        // let guard_fpath = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-07-06 17:10:42/exec_recs/guards/00cadb83512031af7f5c2d1a9ec4e552/139779446296768_main";
        // 1w+ lines
        // let guard_fpath = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-07-06 17:10:42/exec_recs/guards/3effe625c2b16d1fae470b3a34a27a33/140367869948096_main";
        // 16w+ lines
        // let guard_fpath = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-07-22 10:55:25/exec_recs/guards/552e96e21047f09afb5bcd5ec3d474eb/140139541866176";

        // let guard_fpath = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-08-14 11:39:06/exec_recs/guards/260e31301a74deb28aa8b46a4d0855b8/140251941331136_main";

        let guard_dir = "/struct_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-08-14 11:39:06/exec_recs/guards/0ae2706207b8f8ea054bf44e755a6f4e";

        let exec_forest = timer_it(
            || ExecForest::from_guard_dir(guard_dir),
            "Guard Directory Parsing",
        )?;

        // show metadata of the tree
        log::info!("Forest Size: {}", exec_forest.len());
        for tree in exec_forest.iter_trees() {
            log::info!("Tid {}, Tree depth: {}", tree.get_tid(), tree.get_depth());
            tree.show_long_func_nodes()?;
            tree.show_recur_entries()?;
            tree.show_most_called_funcs()?;
            tree.show_most_hit_loop_headers()?;
            tree.show_func_with_most_childs()?;
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

            let func_name = match FuncActionType::get_func_name_from_return_guard(&line) {
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
