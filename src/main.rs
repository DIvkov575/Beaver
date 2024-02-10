mod commands;
mod lib;

use anyhow::Result;
use clap::{self, Parser};

pub fn main() -> Result<()> {
    commands::Args::parse().run()?;

    Ok(())
}