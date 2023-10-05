mod config_parsing;
mod config;
mod commands;

use anyhow::{Result};
use clap::{self, Parser};
use commands::Command;

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = None,
    after_help = "Note: `trash -h` prints a short and concise overview while `trash --help` gives all \
                 details.",
)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Option<Command>,
}
impl Args {
    pub fn run(self) -> Result<()> {
        match self.command {
            None => print!("None"),
            Some(command) => print!("some"),
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    config_parsing::handle_creation("../test")?;
    
    Ok(())
}