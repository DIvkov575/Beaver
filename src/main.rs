mod commands;
mod lib;

use anyhow::Result;
use clap::{self, Parser};
use log::info;


pub fn main() -> Result<()> {
    /// entry point and logger initialization
    log4rs::init_file("./src/beaver_config/logging_config.yaml", Default::default()).unwrap();

    commands::Args::parse().run()?;
    Ok(())
}
