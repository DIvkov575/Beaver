use clap::{self, Parser};


// #[derive(Debug, Parser)]
// #[command(
//     version,
//     about,
//     long_about = None,
//     after_help = "Note: `trash -h` prints a short and concise overview while `trash --help` gives all \
//                  details.",
// )]
// pub struct Args {

// }

use std::io;

use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::app;

#[derive(Debug, Parser)]
pub struct ManArgs {}

impl ManArgs {
    pub fn run(&self) -> Result<()> {
        let man = clap_mangen::Man::new(app::Args::command());
        man.render(&mut io::stdout())?;
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub enum Command {
    /// initialize beaver config dir
    Init(),
    /// Tmp
    Tmp(),
    /// Generates manpages
    Manpage(),
}
impl Command {
    pub fn run(self, config_args: &super::ConfigArgs) -> Result<()> {
        use Command::*;
        match self {
            Init() => Init,
            // Tmp(args) => args.run(config_args),
            Manpage(args) => args.run(),
        }
    }
}