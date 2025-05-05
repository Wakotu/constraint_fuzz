use cc_wrapper::run;
use cc_wrapper::utils::reports::init_report_utils;
use color_eyre::eyre::Result;

fn main() -> Result<()> {
    init_report_utils()?;
    run()?;
    Ok(())
}
