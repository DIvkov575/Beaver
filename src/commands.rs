use std::io;
use anyhow::Result;
use clap::{self, Parser};

mod deploy;
use deploy::deploy;
mod destroy;
// use destroy::destroy;
mod init;
use init::init;


#[derive(Parser, Debug)]
pub enum Command {
    #[command(about="Create a config")]
    Init {
        #[arg(short, long, action)]
        force: bool,
        #[arg(short, long, action)]
        dev: bool,
        #[arg(short, long)]
        path: Option<String>
    },
    #[command(about="Create Beaver instance on GCP")]
    Deploy {
        #[arg(short, long)]
        path: String
    },
    #[command(about="Destroy Beaver instance")]
    Destroy {
        #[arg(short, long)]
        path: String
    },
}
impl Command {
    pub fn run(self) -> Result<()> {
        use Command::*;
        match self {
            Init{force, dev, path} => init(force, dev, path),
            Deploy{path} => deploy(&path),
            Destroy{path} => Ok(()),
        }
    }
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None,)]
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