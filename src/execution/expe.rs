use std::path::{Path, PathBuf};

use crate::{deopt::utils::get_file_dirname, program::transform::Transformer};

use super::{
    logger::{ProgramError, TimeUsage},
    Executor,
};
use eyre::Result;

impl Executor {
    pub fn build_fuzzer(&self, program_path: &Path) -> Result<()> {
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
        self.build_fuzzer(program_path)?;

        Ok(())
    }
}
