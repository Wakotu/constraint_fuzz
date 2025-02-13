use std::path::{Path, PathBuf};

use crate::{
    deopt::utils::{create_dir_if_nonexist, get_file_dirname},
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

    pub fn get_harn_expe_dir(&self) -> Result<PathBuf> {
        let expe_dir = self.get_library_expe_dir()?;
        let harn_expe_dir: PathBuf = [expe_dir, self.config.project_name.clone().into()]
            .iter()
            .collect();
        create_dir_if_nonexist(&harn_expe_dir)?;
        Ok(harn_expe_dir)
    }
}

impl Executor {
    pub fn build_expe_fuzzer(&self, program_path: &Path) -> Result<()> {
        let time_logger = TimeUsage::new(get_file_dirname(program_path));
        let mut transformer = Transformer::new(program_path, &self.deopt)?;
        transformer.add_fd_sanitizer()?;
        transformer.preprocess()?;

        let mut binary_out = PathBuf::from(program_path);
        binary_out.set_extension("out");

        self.deopt
            .copy_library_init_file(&get_file_dirname(program_path))?;

        self.compile(vec![program_path], &binary_out, super::Compile::FUZZER)?;
        time_logger.log("build fuzzer")?;
        Ok(())
    }

    pub fn run_expe(&self, program_path: &Path) -> Result<()> {
        self.build_expe_fuzzer(program_path)?;

        Ok(())
    }
}
