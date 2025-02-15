use core::panic;
use std::path::{Path, PathBuf};

use crate::{
    deopt::utils::{create_dir_if_nonexist, get_file_dirname, get_formatted_time},
    execution::Compile,
    program::transform::Transformer,
};

use super::{
    logger::{ProgramError, TimeUsage},
    Executor,
};
use crate::deopt::Deopt;
use eyre::Result;

impl Deopt {
    pub fn get_library_expe_dir(&self) -> Result<PathBuf> {
        let lib_build_dir = self.get_library_build_dir()?;
        let expe_dir: PathBuf = [lib_build_dir, "expe".into()].iter().collect();
        create_dir_if_nonexist(&expe_dir)?;
        Ok(expe_dir)
    }

    fn get_harn_name(program_path: &Path) -> Result<String> {
        if let Some(basename) = program_path.file_stem() {
            let basename = basename.to_str().unwrap_or_else(|| {
                panic!("Could not convert basename to string");
            });
            let time_suffix = get_formatted_time();
            let harn_name = format!("{}-{}", basename, time_suffix);
            Ok(harn_name)
        } else {
            eyre::bail!(
                "Failed to extract file stem of program_path {}",
                program_path.to_string_lossy()
            );
        }
    }

    // work dir
    pub fn get_harn_expe_dir(&self, program_path: &Path) -> Result<PathBuf> {
        let expe_dir = self.get_library_expe_dir()?;
        let harn_name = self.get_harn_expe_dir(program_path)?;
        let harn_expe_dir: PathBuf = [expe_dir, harn_name.into()].iter().collect();
        create_dir_if_nonexist(&harn_expe_dir)?;
        Ok(harn_expe_dir)
    }

    pub fn get_expe_fuzzer_path(&self, program_path: &Path) -> Result<PathBuf> {
        let harn_expe_dir = self.get_harn_expe_dir(program_path)?;

        let binary_out: PathBuf = [harn_expe_dir, "fuzzer".into()].iter().collect();
        Ok(binary_out)
    }

    pub fn get_expe_cov_fuzzer_path(&self, program_path: &Path) -> Result<PathBuf> {
        let harn_expe_dir = self.get_harn_expe_dir(program_path)?;

        let binary_out: PathBuf = [harn_expe_dir, "cov_fuzzer".into()].iter().collect();
        Ok(binary_out)
    }

    pub fn get_expe_corpus_dir(&self, program_path: &Path) -> Result<PathBuf> {
        let expe_dir = self.get_harn_expe_dir(program_path)?;
        let corpus_dir: PathBuf = [expe_dir, "corpus".into()].iter().collect();
        Ok(corpus_dir)
    }
}

impl Executor {
    pub fn build_expe_fuzzer(&self, program_path: &Path, work_dir: &Path) -> Result<PathBuf> {
        log::trace!("build expe fuzzer: {program_path:?}");

        let time_logger = TimeUsage::new(work_dir.to_owned());
        let mut transformer = Transformer::new(program_path, &self.deopt)?;

        transformer.add_fd_sanitizer()?;
        transformer.preprocess()?;

        let binary_out = self.deopt.get_expe_fuzzer_path(program_path)?;

        self.deopt
            .copy_library_init_file(&get_file_dirname(program_path))?;

        self.compile(vec![program_path], &binary_out, super::Compile::FUZZER)?;
        time_logger.log("expe build")?;
        Ok(binary_out)
    }

    pub fn run_expe_fuzzer(&self, fuzzer: &Path, work_dir: &Path, corpus_dir: &Path) -> Result<()> {
        log::trace!("run expe fuzzer: {fuzzer:?}");
        let time_logger = TimeUsage::new(work_dir.to_owned());

        // execute fuzzer for duration timeout.
        crate::deopt::utils::create_dir_if_nonexist(corpus_dir)?;

        let res = self.execute_fuzzer(
            fuzzer,
            vec![corpus_dir, &self.deopt.get_library_shared_corpus_dir()?],
        );
        time_logger.log("expe fuzz")?;
        if let Err(err) = res {
            log::error!(
                "fuzzer running error: {}, {}",
                err.to_string(),
                fuzzer.to_string_lossy()
            );
        }
        Ok(())
    }

    pub fn expe_cov_collect(
        &self,
        program_path: &Path,
        work_dir: &Path,
        corpus_dir: &Path,
    ) -> Result<()> {
        log::trace!("expe cov build: {program_path:?}");
        let time_logger = TimeUsage::new(work_dir.to_owned());

        // build
        let cov_fuzzer = self.deopt.get_expe_cov_fuzzer_path(program_path)?;
        self.compile(vec![program_path], &cov_fuzzer, Compile::COVERAGE)?;

        // run and report
        let coverage = self.collect_code_coverage(
            Some(program_path),
            &cov_fuzzer,
            vec![corpus_dir, &self.deopt.get_library_shared_corpus_dir()?],
        )?;

        time_logger.log("expe coverage collection")?;

        Ok(())
    }

    pub fn run_expe(&self, program_path: &Path) -> Result<()> {
        let work_dir = self.deopt.get_harn_expe_dir(program_path)?;
        let corpus_dir = self.deopt.get_expe_corpus_dir(program_path)?;

        let fuzzer_path = self.build_expe_fuzzer(program_path, &work_dir)?;
        self.run_expe_fuzzer(&fuzzer_path, &work_dir, &corpus_dir)?;
        self.expe_cov_collect(program_path, &work_dir, &corpus_dir)?;
        Ok(())
    }
}
