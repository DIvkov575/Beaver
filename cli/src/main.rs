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
    /// The command to run.
    #[clap(subcommand)]
    pub command: Option<Command>,

    // #[clap(flatten)]
    // config_args: ConfigArgs,

    // #[clap(flatten)]
    // put_args: put::PutArgs,
}
impl Args {
    pub fn run(self) -> Result<()> {
        match self.command {
            None => self.put_args.run(&self.config_args)?,
            Some(command) => command.run(&self.config_args)?,
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    config_parsing::handle_creation("../test")?;
    
    Ok(())
}