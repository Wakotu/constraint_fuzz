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
