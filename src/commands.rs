use anyhow::Result;
use clap::{self, Parser};

mod dashboard;
use dashboard::DashboardCommand;
mod deploy;
use deploy::deploy;
mod destroy;
use destroy::destroy;
mod init;
use init::init;
mod repair;
use repair::{refresh_detections, repair_dataflow};


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
    #[command(about="Relaunch the Dataflow job for an existing deploy")]
    RepairDataflow {
        #[arg(short, long)]
        path: String,
        /// Use cancel (immediate kill) instead of drain (graceful flush)
        #[arg(long, default_value_t = false)]
        cancel: bool,
    },
    #[command(about="Recompile sigma rules and relaunch Dataflow with new detections")]
    RefreshDetections {
        #[arg(short, long)]
        path: String,
        /// Use cancel (immediate kill) instead of drain (graceful flush)
        #[arg(long, default_value_t = false)]
        cancel: bool,
    },
    #[command(about = "Grafana dashboard preview and export")]
    Dashboard {
        #[command(subcommand)]
        command: DashboardCommand,
    },
}
impl Command {
    pub fn run(self) -> Result<()> {
        use Command::*;
        match self {
            Init{force, dev, path} => init(force, dev, path),
            Deploy{path} => deploy(&path),
            Destroy{path} => destroy(&path),
            RepairDataflow{path, cancel} => repair_dataflow(&path, cancel),
            RefreshDetections{path, cancel} => refresh_detections(&path, cancel),
            Dashboard { command } => command.run(),
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
