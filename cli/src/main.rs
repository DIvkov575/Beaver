// mod config_parsing;
// mod config;
mod commands;

use std::process::Command;
use anyhow::Result;
use clap::{self, Parser};
// use exitcode::ExitCode;

// fn terraform_init() -> Result<()> {
    // let output = Command::new("terraform").arg("init").output()?;
//     print!("{:#?}", output);
//     // print!("{:#?}", output);
//     Ok(())
// }

// fn terraform_plan() -> Result<()> {
//     let output = Command::new("terraform").arg("").output()?;
//     print!("{:#?}", output);
//     // print!("{:#?}", output);
//     Ok(())
// }


fn main() -> Result<()> {
    // match try_main() {
    //     Ok(()) => ExitCode::Success.exit(),
    //     Err(e) => ExitCode::Error.exit_with_msg(format!("{e:#}")),
    // }

    // print!("{:#?}", Command::new("terraform").output()?);
    

    commands::Args::parse().run()?;
    Ok(())
}

// fn try_main() -> Result<()> {
//     commands::Args::parse().run()
// }