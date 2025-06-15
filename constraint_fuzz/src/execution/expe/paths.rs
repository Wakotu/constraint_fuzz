use crate::deopt::utils::create_dir_if_nonexist;
use crate::execution::expe::get_formatted_time;
use crate::execution::Path;
use crate::execution::PathBuf;
use crate::Deopt;

use color_eyre::eyre::Result;

// add extra paths to Deopt
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

    pub fn get_constraints_path(&self, work_dir: &Path) -> PathBuf {
        work_dir.join("constraints.json")
    }

    pub fn get_expe_constraints_show_dir(&self, work_dir: &Path) -> Result<PathBuf> {
        let show_dir = work_dir.join("constraints_show");
        create_dir_if_nonexist(&show_dir)?;
        Ok(show_dir)
    }
}
