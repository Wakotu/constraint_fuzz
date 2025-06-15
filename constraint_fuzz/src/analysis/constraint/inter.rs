use rayon::prelude::*;
use std::{
    fs,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::{
    deopt::{
        utils::{
            buffer_read_to_bytes, create_dir_if_nonexist, get_basename_str_from_path,
            get_parent_dir,
        },
        Deopt,
    },
    feedback::{
        branches::constraints::Constraint,
        clang_coverage::{BranchCount, CodeCoverage},
    },
};

use color_eyre::eyre::Result;
use eyre::bail;

use super::ConsDFBuilder;

/**
 * This module is used to get function call chain from entry to constraints (inter-procedural analysis)
 */

pub type FuncChain = Vec<String>;

// Define executions as exec name + cov + func stack. (file path or value)
#[derive(Debug, Clone)]
pub struct ExecRec {
    exec_name: String,
    // func stack path
    fs_dir: PathBuf,
    cov_path: PathBuf,
}

impl ExecRec {
    // based on expe pipeline
    pub fn get_exec_msg_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = work_dir.join("exec_msg");
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
        let fs_dir = msg_dir.join("func_stack");
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
            fs_dir,
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
            fs_dir: fs_dir.to_owned(),
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

struct FuncStack {
    target_func: String,
    sta: Vec<String>,
    fs_path: PathBuf,
}

impl FuncStack {
    pub fn new(target_func: &str, fs_path: &Path) -> Self {
        Self {
            target_func: target_func.to_owned(),
            sta: Vec::new(),
            fs_path: fs_path.to_owned(),
        }
    }

    fn enter(&mut self, func_name: &str) -> bool {
        self.sta.push(func_name.to_owned());

        if func_name == self.target_func {
            return true;
        }

        false
    }

    fn return_from(&mut self, func_name: &str) -> Result<()> {
        if self.is_empty() {
            bail!("Attempted to return from an empty function stack");
        }

        let top_name = self.sta.last().unwrap().as_str();
        if top_name != func_name {
            bail!(
                "Attempted to return from function {} but the top of the stack is {}",
                func_name,
                top_name
            );
        }
        self.sta.pop();
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.sta.is_empty()
    }

    fn get_func_namne_from_line<'a>(line: &'a str, prefix: &'a str) -> Result<&'a str> {
        if !line.starts_with(prefix) {
            bail!("Line does not start with expected prefix: {}", line);
        }

        // extract func_name: get rid of prefix and read until char '('
        let start = prefix.len();
        let end = line.find('(').unwrap_or_else(|| line.len());
        let func_name = &line[start..end];
        Ok(func_name)
    }

    pub fn get_chain(&mut self) -> Result<FuncChain> {
        let file = File::open(&self.fs_path)?;
        let reader = BufReader::new(file);
        for line_res in reader.lines() {
            let line = line_res?;
            if line.starts_with("enter ") {
                let func_name = Self::get_func_namne_from_line(&line, "enter ")?;
                let flag = self.enter(func_name);
                if flag {
                    return Ok(self.sta.clone());
                }
            } else if line.starts_with("return from ") {
                let func_name = Self::get_func_namne_from_line(&line, "return from ")?;
                self.return_from(func_name)?;
            } else {
                bail!("Unexpected line in function stack file: {}", line);
            }
        }

        bail!(
            "Function {} not found in stack file: {}",
            self.target_func,
            self.fs_path.display()
        )
    }
}

impl ConsDFBuilder {
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

    fn get_related_executions_with_num(&self, num: Option<usize>) -> Result<Vec<ExecRec>> {
        let exec_list = ExecRec::get_exec_list_from_work_dir(&self.work_dir)?;
        let mut res_list = vec![];

        for (idx, exec) in exec_list.iter().enumerate() {
            log::debug!(
                "Processing execution {}/{}: {}",
                idx + 1,
                exec_list.len(),
                exec.exec_name
            );

            let json_slice = buffer_read_to_bytes(&exec.cov_path)?;
            let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;

            if cov.contains_cons(&self.cons)? {
                log::debug!("exec: {} related and collected", exec.exec_name);
                res_list.push(exec.to_owned());
            } else {
                log::debug!("exec: {} not related", exec.exec_name);
            }

            // check len
            if let Some(num) = num {
                if res_list.len() >= num {
                    break;
                }
            }
        }
        Ok(res_list)
    }

    // iterate over all files in the coverage directory
    // judge related based on cov and then constructs the related ones only
    // pub fn get_related_executions(&self) -> Result<Vec<ExecRec>> {
    //     self.get_related_executions_with_num(None)
    // }

    /// exec -> threads -> func stack list -> func chain list
    pub fn extract_func_chain(&self, exec: &ExecRec) -> Result<Vec<FuncChain>> {
        let mut chain_list = vec![];
        for ent_res in fs::read_dir(&exec.fs_dir)? {
            let entry = ent_res?;
            let fs_path = entry.path();
            assert!(fs_path.is_file(), "Expected a file: {}", fs_path.display());
            // if !fs_path.exists() {
            //     bail!(
            //         "Function stack file for {} does not exist: {}",
            //         &exec.exec_name,
            //         fs_path.display()
            //     );
            // }

            // recover the stack
            let mut func_stack = FuncStack::new(&self.cons.get_func_name()?, &fs_path);
            let chain = match func_stack.get_chain() {
                Ok(chain) => chain,
                Err(e) => {
                    log::warn!(
                        "Failed to get function chain for {}: {}",
                        fs_path.display(),
                        e
                    );
                    continue;
                }
            };
            assert!(
                !chain.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            chain_list.push(chain);
        }
        assert!(
            !chain_list.is_empty(),
            "No function chains found for exec: {}",
            exec.exec_name
        );
        Ok(chain_list)
    }
}

/**
 * unit tests
 */
#[cfg(test)]
mod tests {
    use crate::{deopt::utils::deduplicate_unordered, init_report_utils_for_tests};

    use super::*;

    fn setup_test_consdf_builder() -> Result<ConsDFBuilder> {
        let work_dir =
            "/constraint_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-05-14 00:00:49";
        let json_slice = buffer_read_to_bytes("/constraint_fuzz/constraint_fuzz/output/build/libaom/expe/example_fuzzer-2025-05-14 00:00:49/constraints.json");
        let cons_list: Vec<Constraint> = serde_json::from_slice(&json_slice?)?;
        let builder = ConsDFBuilder::new(&cons_list[0], work_dir);
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
    fn test_extract_func_chain() -> Result<()> {
        init_report_utils_for_tests()?;
        let builder = setup_test_consdf_builder()?;

        let execs = builder.get_related_executions()?;
        let mut res_chain_list = vec![];
        for exec in execs {
            let chain_list = builder.extract_func_chain(&exec)?;
            assert!(
                !chain_list.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
            log::debug!("Function chain for {}: {:?}", exec.exec_name, chain_list);
            res_chain_list.extend(chain_list);
        }
        deduplicate_unordered(&mut res_chain_list);

        log::debug!("Total function chains extracted: {}", res_chain_list.len());
        log::debug!("Function chains: {:?}", res_chain_list);
        Ok(())
    }
}
