pub mod reports {

    use color_eyre::eyre::Result;
    use colored::*;

    fn my_format(
        write: &mut dyn std::io::Write,
        now: &mut flexi_logger::DeferredNow,
        record: &log::Record,
    ) -> std::io::Result<()> {
        let level = match record.level() {
            log::Level::Error => "ERROR".red().bold(),
            log::Level::Warn => "WARN".yellow().bold(),
            log::Level::Info => "INFO".green().bold(),
            log::Level::Debug => "DEBUG".blue().bold(),
            log::Level::Trace => "TRACE".purple().bold(),
        };
        write!(
            write,
            "[{}] {} - {}",
            now.now().format("%Y-%m-%d %H:%M:%S"),
            level,
            record.args()
        )?;
        Ok(())
    }

    pub fn init_flexi_logger() -> Result<()> {
        flexi_logger::Logger::try_with_env_or_str("debug")?
            .format(my_format)
            .start()?;
        Ok(())
    }

    pub fn init_report_utils() -> Result<()> {
        init_flexi_logger()?;
        color_eyre::install()?;
        Ok(())
    }
}

pub mod paths {
    use color_eyre::eyre::Result;
    use std::{
        env,
        path::{Path, PathBuf},
    };

    pub fn get_lib_dir() -> Result<PathBuf> {
        let ins_pre = match option_env!("FSP_INSTALL_PREFIX") {
            Some(val) => val.to_string(),
            None => {
                let home = env::var("HOME")?;
                format!("{}/.local/lib", home)
            }
        };
        let name = option_env!("FSP_NAME").unwrap_or("func_stack_pass");
        let lib_dir = Path::new(&ins_pre).join(name);

        Ok(lib_dir)
    }

    pub fn get_plugin_path() -> Result<PathBuf> {
        let lib_dir = get_lib_dir()?;
        let plugin_name = option_env!("FSP_PLUGIN_LIB").unwrap_or("func_stack_plugin");
        let plg_entry = format!("lib{}.so", plugin_name);
        let fpath = lib_dir.join(&plg_entry);
        Ok(fpath)
    }

    pub fn get_impl_lib_name() -> &'static str {
        option_env!("FSP_IMPL_LIB").unwrap_or("func_stack")
    }
}
