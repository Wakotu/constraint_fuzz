pub mod cc_args;
pub mod utils;
use std::process::Command;

use cc_args::CCArgs;
use color_eyre::eyre::Result;

fn edit_params() -> Result<Vec<String>> {
    let mut args = CCArgs::from_cli()?;
    args.transform()?;
    Ok(args.output())
}

pub fn run() -> Result<()> {
    let args = edit_params()?;
    let mut child = Command::new(&args[0]).args(&args[1..]).spawn()?;
    child.wait()?;
    Ok(())
}
