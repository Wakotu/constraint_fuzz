use crate::deopt::utils::buffer_read_to_bytes;
use crate::deopt::utils::get_file_parent_dir;
use crate::execution::get_file_dirname;
use crate::feedback::branches::constraints::Constraint;
use crate::feedback::clang_coverage::CodeCoverage;
use color_eyre::eyre::Result;
use std::fs;
use std::path::PathBuf;

use std::path::Path;

use crate::execution::Executor;

impl Executor {
    // return list of Constraints
    pub fn extract_cons_from_cov(
        &self,
        cov: CodeCoverage,
        work_dir: &Path,
    ) -> Result<Vec<Constraint>> {
        // analysis for unselected branches
        let cons_list = cov.collect_rev_constraints_from_cov_by_pool()?;
        self.save_cons_list(&cons_list, work_dir)?;
        // self.show_each_cons(&cons_list, work_dir)?;
        log::debug!("Constraint Extraction done");
        Ok(cons_list)
    }

    // wrapper for constrainst extraction
    pub fn get_cons_from_cov(&self, cov: CodeCoverage, work_dir: &Path) -> Result<Vec<Constraint>> {
        // analysis for unselected branches
        let cons_path = self.deopt.get_constraints_path(work_dir);
        if cons_path.is_file() {
            // read file to bytes
            let json_slice = buffer_read_to_bytes(&cons_path)?;
            let cons_list: Vec<Constraint> = serde_json::from_slice(&json_slice)?;
            return Ok(cons_list);
        }
        let cons_list = cov.collect_rev_constraints_from_cov_by_pool()?;
        Ok(cons_list)
    }

    fn get_cov_profdata(&self, cov_bin: &Path, corpus_dirs: &[&Path]) -> Result<PathBuf> {
        let work_dir = get_file_dirname(cov_bin);
        let profdata: PathBuf = crate::deopt::Deopt::get_coverage_file_by_dir(&work_dir);
        if profdata.is_file() {
            fs::remove_file(&profdata)?;
        }
        self.execute_cov_fuzzer_pool(cov_bin, corpus_dirs, &profdata)?;

        Ok(profdata)
    }

    // main function for this module
    pub fn instru_cov_fuzzer_run(
        &self,
        cov_fuzzer: &Path,
        corpus_dirs: &[&Path],
        fuzzer_src: &Path,
    ) -> Result<()> {
        let work_dir = get_file_parent_dir(cov_fuzzer);
        // cov run and get profdata
        let profdata = self.get_cov_profdata(cov_fuzzer, corpus_dirs)?;

        // collect contraints from cov data
        let cov = self.get_code_cov_from_profdata(cov_fuzzer, fuzzer_src, &profdata)?;
        self.extract_cons_from_cov(cov, work_dir)?;
        Ok(())
    }
}
