mod commands;
use anyhow::Result;
use clap::{self, Parser};




fn main() -> Result<()> {
    commands::Args::parse().run()?;
    Ok(())
}
