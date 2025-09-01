use color_eyre::eyre::{eyre, Result};
use std::fs;
use std::path::Path;
use std::{fmt, path::PathBuf};

use crate::analysis::constraint::exec_rec::case_map::get_case_path_from_exec_name;
use crate::analysis::constraint::inter::exec_tree::ExecForest;
use crate::deopt::utils::{
    buffer_read_to_bytes, create_dir_if_nonexist, get_basename_str_from_path, get_parent_dir,
};
use crate::feedback::clang_coverage::CodeCoverage;

pub mod case_map;

pub struct ExecRec {
    exec_name: String,
    // execution guard dir
    execg_dir: PathBuf,
    cov_path: PathBuf,
    cov: CodeCoverage,
    exec_forest: ExecForest,
}

impl fmt::Display for ExecRec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExecRec({})", self.exec_name)
    }
}

impl ExecRec {
    /**
     * exec_* means global path, sg_* means path for single ExecRec instance
     * Path construction methods
     */

    // based on expe pipeline
    pub fn get_exec_msg_dir(expe_dir: &Path) -> Result<PathBuf> {
        let msg_dir = expe_dir.join("exec_recs");
        create_dir_if_nonexist(&msg_dir)?;
        Ok(msg_dir)
    }

    // based on the exec object
    pub fn get_exec_cov_dir(expe_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(expe_dir)?;
        let cov_dir = msg_dir.join("cov");
        create_dir_if_nonexist(&cov_dir)?;
        Ok(cov_dir)
    }

    pub fn get_sg_guard_dir(expe_dir: &Path, exec_name: &str) -> Result<PathBuf> {
        let guard_dir = Self::get_exec_guard_dir(expe_dir)?;
        let sg_guard_dir = guard_dir.join(exec_name);
        create_dir_if_nonexist(&sg_guard_dir)?;
        Ok(sg_guard_dir)
    }

    // based on the exec object
    pub fn get_exec_guard_dir(expe_dir: &Path) -> Result<PathBuf> {
        let msg_dir = Self::get_exec_msg_dir(expe_dir)?;
        let fs_dir = msg_dir.join("guards");
        create_dir_if_nonexist(&fs_dir)?;
        Ok(fs_dir)
    }

    /// Main interface of exec message directory construction
    pub fn setup_exec_dir(expe_dir: &Path) -> Result<(PathBuf, PathBuf)> {
        // create coverage directory
        let cov_dir = Self::get_exec_cov_dir(expe_dir)?;
        // create func stack directory
        let guard_dir = Self::get_exec_guard_dir(expe_dir)?;

        Ok((guard_dir, cov_dir))
    }

    fn get_expe_dir_from_cov_path(cov_path: &Path) -> Result<PathBuf> {
        let cov_dir = get_parent_dir(cov_path)?;
        let msg_dir = get_parent_dir(&cov_dir)?;
        let expe_dir = get_parent_dir(&msg_dir)?;
        Ok(expe_dir)
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

impl ExecRec {
    /**
     * Self-Generation methods
     */

    pub fn get_exec_list_from_expe_dir(expe_dir: &Path) -> Result<Vec<Self>> {
        let exec_cov_dir = Self::get_exec_cov_dir(expe_dir)?;
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

        let expe_dir = Self::get_expe_dir_from_cov_path(cov_path)?;
        let sg_guard_dir = Self::get_sg_guard_dir(&expe_dir, &exec_name)?;
        let exec_forest = ExecForest::from_guard_dir(&sg_guard_dir)?;

        let buf = buffer_read_to_bytes(cov_path)?;
        let cov: CodeCoverage = serde_json::from_slice(&buf)?;

        Ok(Self {
            exec_name,
            execg_dir: sg_guard_dir.to_owned(),
            cov_path: cov_path.to_owned(),
            exec_forest,
            cov,
        })
    }

    pub fn from_sg_guard_dir(sg_guard_dir: &Path) -> Result<Self> {
        let exec_name = get_basename_str_from_path(sg_guard_dir)?;
        let exec_dir = get_parent_dir(sg_guard_dir)?;
        let cov_dir = exec_dir.join("cov");
        let cov_path = cov_dir.join(&exec_name);

        let exec_forest = ExecForest::from_guard_dir(sg_guard_dir)?;
        let cov: CodeCoverage = {
            let buf = buffer_read_to_bytes(&cov_path)?;
            serde_json::from_slice(&buf)?
        };

        assert!(
            cov_path.is_file(),
            "Coverage file does not exist: {}",
            cov_path.display()
        );

        Ok(Self {
            exec_name,
            execg_dir: sg_guard_dir.to_owned(),
            cov_path,
            exec_forest,
            cov,
        })
    }
}

impl ExecRec {
    /**
     * Property gettting methods
     */

    pub fn get_case_path(&self) -> Result<PathBuf> {
        let case_path = get_case_path_from_exec_name(&self.exec_name)?;
        assert!(
            case_path.is_file(),
            "Case path does not exist: {}",
            case_path.display()
        );

        Ok(case_path)
    }
}
