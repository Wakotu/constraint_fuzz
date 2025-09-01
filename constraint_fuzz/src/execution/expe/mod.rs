use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use crate::{
    deopt::utils::{get_file_dirname, get_formatted_time},
    execution::Compile,
    feedback::branches::constraints::UBConstraint,
};

use super::{logger::TimeUsage, Executor};
// use crate::deopt::Deopt;
// use color_eyre::eyre::Result;
// use crate::feedback::branches::constraints::collect_constraints_from_cov;
use color_eyre::eyre::Result;

pub mod instru_cov_run;
pub mod paths;

// #[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// pub enum CovFormat {
//     SHOW,
//     JSON,
//     LCOV,
// }

impl Executor {
    pub fn build_expe_fuzzer(&self, program_path: &Path, work_dir: &Path) -> Result<()> {
        log::trace!("build expe fuzzer: {program_path:?}");

        let time_logger = TimeUsage::new(&work_dir);
        // let mut transformer = Transformer::new(program_path, &self.deopt)?;
        //
        // transformer.add_fd_sanitizer()?;
        // transformer.preprocess()?;

        let binary_out = self.deopt.get_expe_fuzzer_path(work_dir)?;

        self.deopt
            .copy_library_init_file(&get_file_dirname(program_path))?;

        self.compile(vec![program_path], &binary_out, super::Compile::FUZZER)?;
        time_logger.log("expe build")?;
        Ok(())
    }

    pub fn run_expe_fuzzer(&self, work_dir: &Path, corpus_dirs: &[&Path]) -> Result<()> {
        let fuzzer = self.deopt.get_expe_fuzzer_path(work_dir)?;
        log::trace!("run expe fuzzer: {fuzzer:?}");
        let time_logger = TimeUsage::new(&work_dir);

        let res = self.execute_fuzzer(&fuzzer, corpus_dirs);
        time_logger.log("expe fuzz")?;
        if let Err(err) = res {
            log::error!(
                "fuzzer running error: {}, {}",
                err,
                fuzzer.to_string_lossy()
            );
        }
        Ok(())
    }

    // fn show_lib_cov_from_profdata(&self, profdata: &Path) -> Result<()> {
    //     let cov_lib = get_cov_lib_path(&self.deopt, true);
    //     log::debug!("lib during llvm-cov invocation: {cov_lib:?}");
    //     let output = Command::new("llvm-cov")
    //         .arg("show")
    //         .arg(cov_lib)
    //         .arg(format!("--instr-profile={}", profdata.to_string_lossy()))
    //         .arg("--format=html")
    //         .output()?;
    //     if !output.status.success() {
    //         eyre::bail!("Failed to show cov view from {profdata:?}\ncmd: {output:?}");
    //     }
    //     let html_output = output.stdout.as_slice();
    //
    //     // writes to file
    //     let work_dir = get_file_dirname(profdata);
    //     let html_path = work_dir.join("cov.html");
    //     fs::write(&html_path, html_output)?;
    //
    //     Ok(())
    // }

    // fn show_each_cons(&self, cons_list: &Vec<Constraint>, work_dir: &Path) -> Result<()> {
    //     let show_dir = self.deopt.get_expe_constraints_show_dir(work_dir)?;
    //     for cons in cons_list {
    //         let fname = cons.get_show_filename()?;
    //         let fpath = show_dir.join(&fname);
    //         // log::debug!("cons fpath: {:?}", fpath);
    //         let content = cons.get_show_content()?;
    //         Deopt::write_wtih_buffer(&fpath, content.as_bytes())?;
    //     }
    //     Ok(())
    // }

    fn save_cons_list(&self, cons_list: &Vec<UBConstraint>, work_dir: &Path) -> Result<PathBuf> {
        let fpath = self.deopt.get_constraints_path(work_dir);
        let file = File::create(&fpath)?;
        // let mut writer = BufWriter::new(file);
        let writer = BufWriter::new(file);

        // let toml_str = toml::to_string(&cons_list)?;
        // writer.write_all(toml_str.as_bytes())?;

        serde_json::to_writer(writer, cons_list)?;

        Ok(fpath)
    }

    // program path is arbitrary
    pub fn cov_procedure(
        &self,
        program_path: &Path,
        work_dir: &Path,
        corpus_dirs: &[&Path],
    ) -> Result<()> {
        log::info!("Expe cov procedure started");
        let time_logger = TimeUsage::new(&work_dir);

        // build cov_fuzzer
        let cov_fuzzer = self.deopt.get_expe_cov_fuzzer_path(work_dir)?;
        self.compile(vec![program_path], &cov_fuzzer, Compile::COVERAGE)?;

        // insrumented cov fuzzer run
        self.instru_cov_fuzzer_run(&cov_fuzzer, corpus_dirs, program_path)?;

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

    fn fuzzer_procedure(
        &self,
        program_path: &Path,
        work_dir: &Path,
        corpus_list: &[&Path],
    ) -> Result<()> {
        log::info!("Expe fuzzer procedure started");
        self.build_expe_fuzzer(program_path, &work_dir)?;
        self.run_expe_fuzzer(&work_dir, &corpus_list)?;
        Ok(())
    }

    pub fn run_expe<P: AsRef<Path>>(&self, program_path: P) -> Result<()> {
        log::info!("Expe run started");
        let program_path = program_path.as_ref();
        let work_dir = self.deopt.get_expe_work_dir(program_path)?;
        let expe_corpus = self.deopt.get_expe_corpus_dir(&work_dir)?;
        let lib_corpus = self.deopt.get_library_build_corpus_dir()?;
        let shared_corpus = self.deopt.get_library_shared_corpus_dir()?;
        let corpus_list: [&Path; 3] = [&expe_corpus, &lib_corpus, &shared_corpus];

        self.fuzzer_procedure(program_path, &work_dir, &corpus_list)?;
        self.cov_procedure(program_path, &work_dir, &corpus_list)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{deopt::Deopt, setup_test_run_entry};

    #[test]
    fn test_expe_run() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let deopt = Deopt::new("libaom")?;
        let executor = Executor::new(&deopt)?;
        executor.run_expe("/struct_fuzz/constraint_fuzz/examples/libaom/example_fuzzer.cc")?;
        Ok(())
    }
}
