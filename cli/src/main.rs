// mod config_parsing;
// mod config;
mod commands;

use anyhow::Result;
use clap::{self, Parser};
// use exitcode::ExitCode;



fn main() -> Result<()> {
    // match try_main() {
    //     Ok(()) => ExitCode::Success.exit(),
    //     Err(e) => ExitCode::Error.exit_with_msg(format!("{e:#}")),
    // }

    commands::Args::parse().run()?;
    Ok(())
}

// fn try_main() -> Result<()> {
//     commands::Args::parse().run()
// }