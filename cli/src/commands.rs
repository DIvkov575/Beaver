use std::fmt::{Display, Formatter};
use std::io;
use std::path::Component::ParentDir;
use anyhow::Result;
use clap::{self, Parser};

mod deploy;
use deploy::deploy;
mod destroy;
use destroy::destroy;
mod init;
use init::init;

use crate::commands::Command::Deploy;



// #[derive(Debug)]
// struct Error<'a > { pub source: &'a str }
// impl Display for Error { fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { write!(f, "My Error?!") } }
// impl std::error::Error for Error { fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.source) } }


#[derive(Parser, Debug)]
pub enum Command {
    #[command(about="Create a config")]
    Init,
    #[command(about="Create Beaver instance on GCP")]
    Deploy,
    #[command(about="Destroy Beaver instance")]
    Destroy,
}
impl Command {
    pub fn run(self, config_args: ConfigArgs) -> Result<()> {
        use Command::*;
        match self {
            Init => init(),
            Deploy => {
                if let Some(path) = config_args.config_path {
                    deploy(path.as_str())?;
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Missing config argument eg. `beaver --config=\"./conf\" deploy`"))
                }
            }
            Destroy => destroy(),
            _ => {print!("No command was passed"); return Ok(());}
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
    #[clap(flatten)]
    config_args: ConfigArgs,

}
impl Args {
    pub fn run(self) -> Result<()> {
        match self.command {
            None => print!("No Command was passed"),
            Some(command) => command.run(self.config_args)?
        }
        Ok(())
    }
}


#[derive(Debug, Parser)]
pub struct ConfigArgs {
    #[arg(
        short='c',
        long="config"
    )]
    pub config_path: Option<String>
}