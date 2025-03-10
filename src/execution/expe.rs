use core::panic;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    deopt::utils::{
        create_dir_if_nonexist, get_cov_lib_path, get_file_dirname, get_formatted_time,
    },
    execution::Compile,
    program::transform::Transformer,
};

use super::{
    logger::{ProgramError, TimeUsage},
    Executor,
};
use crate::deopt::Deopt;
// use color_eyre::eyre::Result;
use color_eyre::eyre::Result;

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
    pub fn get_expe_work_dir(&self, program_path: &Path) -> Result<PathBuf> {
        let expe_dir = self.get_library_expe_dir()?;
        let harn_name = Self::get_harn_name(program_path)?;
        let work_dir = expe_dir.join(harn_name);
        create_dir_if_nonexist(&work_dir)?;
        Ok(work_dir)
    }

    pub fn get_expe_fuzzer_path(&self, work_dir: &Path) -> Result<PathBuf> {
        let binary_out = work_dir.join("fuzzer");
        Ok(binary_out)
    }

    pub fn get_expe_cov_fuzzer_path(&self, work_dir: &Path) -> Result<PathBuf> {
        let binary_out = work_dir.join("cov_fuzzer");
        Ok(binary_out)
    }

    pub fn get_expe_corpus_dir(&self, work_dir: &Path) -> Result<PathBuf> {
        let corpus_dir = work_dir.join("corpus");
        create_dir_if_nonexist(&corpus_dir)?;
        Ok(corpus_dir)
    }
}

impl Executor {
    pub fn build_expe_fuzzer(&self, program_path: &Path, work_dir: &Path) -> Result<PathBuf> {
        log::trace!("build expe fuzzer: {program_path:?}");

        let time_logger = TimeUsage::new(work_dir.to_owned());
        // let mut transformer = Transformer::new(program_path, &self.deopt)?;
        //
        // transformer.add_fd_sanitizer()?;
        // transformer.preprocess()?;

        let binary_out = self.deopt.get_expe_fuzzer_path(work_dir)?;

        self.deopt
            .copy_library_init_file(&get_file_dirname(program_path))?;

        self.compile(vec![program_path], &binary_out, super::Compile::FUZZER)?;
        time_logger.log("expe build")?;
        Ok(binary_out)
    }

    pub fn run_expe_fuzzer(&self, fuzzer: &Path, work_dir: &Path, corpus_dir: &Path) -> Result<()> {
        log::trace!("run expe fuzzer: {fuzzer:?}");
        let time_logger = TimeUsage::new(work_dir.to_owned());

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

    fn get_cov_profdata(&self, cov_bin: &Path, corpus_dirs: Vec<&Path>) -> Result<PathBuf> {
        let work_dir = get_file_dirname(cov_bin);
        let profdata: PathBuf = crate::deopt::Deopt::get_coverage_file_by_dir(&work_dir);
        if profdata.is_file() {
            fs::remove_file(&profdata)?;
        }
        self.execute_cov_fuzzer_pool(cov_bin, corpus_dirs, &profdata)?;

        Ok(profdata)
    }

    fn show_lib_cov_from_profdata(&self, profdata: &Path) -> Result<()> {
        let cov_lib = get_cov_lib_path(&self.deopt, true);
        let output = Command::new("llvm-cov")
            .arg("show")
            .arg(cov_lib)
            .arg(format!("--instr-profile={}", profdata.to_string_lossy()))
            .arg("--format=html")
            .output()?;
        if !output.status.success() {
            eyre::bail!("Failed to show cov view from {profdata:?}\ncmd: {output:?}");
        }
        let html_output = output.stdout.as_slice();

        // writes to file
        let work_dir = get_file_dirname(profdata);
        let html_path = work_dir.join("cov.html");
        fs::write(&html_path, html_output)?;

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
        let cov_fuzzer = self.deopt.get_expe_cov_fuzzer_path(work_dir)?;
        self.compile(vec![program_path], &cov_fuzzer, Compile::COVERAGE)?;

        let profdata = self.get_cov_profdata(
            &cov_fuzzer,
            vec![corpus_dir, &self.deopt.get_library_shared_corpus_dir()?],
        )?;

        self.show_lib_cov_from_profdata(&profdata)?;

        // run and report
        // let coverage = self.collect_code_coverage(
        //     Some(program_path),
        //     &cov_fuzzer,
        //     vec![corpus_dir, &self.deopt.get_library_shared_corpus_dir()?],
        // )?;

        // may be remove corpus dir invocation to add

        time_logger.log("expe coverage collection")?;

        Ok(())
    }

    pub fn run_expe(&self, program_path: &Path) -> Result<()> {
        let work_dir = self.deopt.get_expe_work_dir(program_path)?;
        let corpus_dir = self.deopt.get_expe_corpus_dir(&work_dir)?;

        let fuzzer_path = self.build_expe_fuzzer(program_path, &work_dir)?;
        self.run_expe_fuzzer(&fuzzer_path, &work_dir, &corpus_dir)?;
        self.expe_cov_collect(program_path, &work_dir, &corpus_dir)?;
        Ok(())
    }
}
