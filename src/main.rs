mod commands;
mod lib;

use anyhow::Result;
use clap::{self, Parser};




fn main() -> Result<()> {
    log4rs::init_file("./src/beaver_config/logging_config.yaml", Default::default()).unwrap();

    commands::Args::parse().run()?;
    Ok(())
}
