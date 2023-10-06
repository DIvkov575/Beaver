use std::io;
use anyhow::Result;
use clap::{self, Parser};

mod init;
use init::init;

#[derive(Parser, Debug)]
pub enum Command {
    Init { path: Option<String> },
    Manpage,
}
impl Command {
    pub fn run(self) -> Result<()> {
        use Command::*;
        match self {
            Init {path } => init(path),
            _ => {print!("asldkfj"); return Ok(());}
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    version,
    about,
    long_about = None,
)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Option<Command>,
}
impl Args {
    pub fn run(self) -> Result<()> {
        match self.command {
            None => print!("No Command was passed"),
            Some(command) => command.run()?,
        }
        Ok(())
    }
}