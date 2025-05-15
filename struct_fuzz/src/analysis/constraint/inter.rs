use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::{
    deopt::{
        utils::{buffer_read_to_bytes, create_dir_if_nonexist, get_basename_str_from_path},
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

impl Deopt {
    /**
     * exec paths relative to work_dir
     */

    pub fn get_exec_msg_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = work_dir.join("exec_msgs");
        create_dir_if_nonexist(&msg_dir)?;
        Ok(msg_dir)
    }

    pub fn get_exec_cov_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let sg_cov_dir = msg_dir.join("cov");
        create_dir_if_nonexist(&sg_cov_dir)?;
        Ok(sg_cov_dir)
    }

    pub fn get_func_stack_dir(work_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(work_dir)?;
        let fs_dir = msg_dir.join("func_stack");
        create_dir_if_nonexist(&fs_dir)?;
        Ok(fs_dir)
    }
}

// Define executions as exec name + cov + func stack. (file path or value)
struct Exec {
    exec_name: String,
    // func stack path
    fs_path: PathBuf,
}

impl Exec {
    pub fn from_cov_path(cov_path: &Path) -> Result<Self> {
        let exec_name = get_basename_str_from_path(cov_path)?;

        let fs_path = cov_path
            .parent()
            .ok_or_else(|| {
                eyre::eyre!(
                    "Failed to get parent directory for coverage path: {}",
                    cov_path.display()
                )
            })?
            .join("func_stack")
            .join(&exec_name);

        Ok(Self { exec_name, fs_path })
    }
}

impl CodeCoverage {
    pub fn contains_cons(&self, cons: &Constraint) -> Result<bool> {
        let func_name = cons.get_func_name()?;
        for func in self.iter_function_covs() {
            if func.get_name() != func_name {
                continue;
            }
            for br in func.branches.iter() {
                let rng = br.get_range()?;
                if rng == cons.range {
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
    pub fn new(target_func: String, fs_path: &Path) -> Self {
        Self {
            target_func,
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
    // judge related based on cov and then constructs the related ones only
    fn get_related_executions(&self) -> Result<Vec<Exec>> {
        let exec_cov_dir = Deopt::get_exec_cov_dir(&self.work_dir)?;

        let mut exec_list: Vec<Exec> = vec![];
        // iterate over all files in the coverage directory
        for ent_res in walkdir::WalkDir::new(&exec_cov_dir).into_iter() {
            let entry = ent_res?;

            let fpath = entry.path();
            // get cov
            let json_slice = buffer_read_to_bytes(fpath)?;
            let cov: CodeCoverage = serde_json::from_slice(&json_slice)?;

            if cov.contains_cons(&self.cons)? {
                let exec = Exec::from_cov_path(fpath)?;
                exec_list.push(exec);
            }
        }

        Ok(exec_list)
    }

    fn extract_func_chain(&self, exec: &Exec) -> Result<FuncChain> {
        let fs_path = &exec.fs_path;

        if !fs_path.exists() {
            bail!(
                "Function stack file for {} does not exist: {}",
                &exec.exec_name,
                fs_path.display()
            );
        }

        // recover the stack
        let mut func_stack = FuncStack::new(exec.exec_name.clone(), &fs_path);
        func_stack.get_chain()
    }
}

/**
 * unit tests
 */
#[cfg(test)]
mod tests {
    use super::*;
    use crate::deopt::utils::test_utils::setup_test_env;

    #[test]
    fn test_get_related_executions() -> Result<()> {
        let (work_dir, cons) = setup_test_env()?;
        let builder = ConsDFBuilder::new(work_dir.clone(), cons.clone());

        let execs = builder.get_related_executions()?;
        assert!(!execs.is_empty(), "No related executions found");

        Ok(())
    }

    #[test]
    fn test_extract_func_chain() -> Result<()> {
        let (work_dir, cons) = setup_test_env()?;
        let builder = ConsDFBuilder::new(work_dir.clone(), cons.clone());

        let execs = builder.get_related_executions()?;
        for exec in execs {
            let chain = builder.extract_func_chain(&exec)?;
            assert!(
                !chain.is_empty(),
                "Function chain is empty for exec: {}",
                exec.exec_name
            );
        }

        Ok(())
    }
}
