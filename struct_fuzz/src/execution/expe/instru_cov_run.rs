use crate::deopt::utils::get_file_parent_dir;
use crate::execution::get_file_dirname;
use crate::feedback::clang_coverage::CodeCoverage;
use color_eyre::eyre::Result;
use std::fs;
use std::path::PathBuf;

use std::path::Path;

use crate::execution::Executor;

impl Executor {
    pub fn handle_unselected_branch(&self, cov: CodeCoverage, work_dir: &Path) -> Result<()> {
        // analysis for unselected branches
        // TODO: to change
        let cons_list = cov.collect_rev_constraints_from_cov_by_pool()?;
        self.save_cons_list(&cons_list, work_dir)?;
        self.show_each_cons(&cons_list, work_dir)?;
        log::debug!("Constraint Collection done");
        Ok(())
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
    pub fn instru_cov_fuzzer_run(
        &self,
        cov_fuzzer: &Path,
        corpus_dirs: &[&Path],
        fuzzer_src: &Path,
    ) -> Result<()> {
        let work_dir = get_file_parent_dir(cov_fuzzer);
        let profdata = self.get_cov_profdata(cov_fuzzer, corpus_dirs)?;

        let cov = self.get_code_cov_from_profdata(cov_fuzzer, fuzzer_src, &profdata)?;
        self.handle_unselected_branch(cov, work_dir)?;
        Ok(())
    }
}
