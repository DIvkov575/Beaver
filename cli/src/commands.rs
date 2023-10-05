use std::io;
use anyhow::Result;
use clap::{self, Parser};

mod init;
use init::init;


// fn manpage() -> Result<()> {
//     let man = clap_mangen::Man::new(app::Args::command());
//     man.render(&mut io::stdout())?;
//     Ok(())
// }

#[derive(Parser, Debug)]
pub enum Command {
    Init,
    Manpage,
}
impl Command {
    pub fn run(self) -> Result<()> {
        use Command::*;
        match self {
            Init => init(),
            _ => {print!("asldkfj"); return Ok(());}

            // Tmp(args) => args.run(config_args),
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