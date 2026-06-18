use std::path::Path;
use anyhow::Result;
use clap::{self, Subcommand};

use crate::lib::grafana::preview;
use crate::lib::utilities::validate_config_path;

#[derive(Subcommand, Debug)]
pub enum DashboardCommand {
    #[command(about = "Preview the Grafana dashboard locally via Docker")]
    Preview {
        #[arg(short, long)]
        path: Option<String>,
    },
    #[command(about = "Export Grafana dashboard JSON to a file")]
    Export {
        #[arg(short, long)]
        path: Option<String>,
        #[arg(short, long)]
        output: Option<String>,
    },
}

impl DashboardCommand {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Preview { path } => {
                let p = path.unwrap_or_else(|| ".".to_string());
                let config_path = Path::new(&p);
                validate_config_path(config_path)?;
                preview::run_preview(config_path)
            }
            Self::Export { path, output } => {
                let p = path.unwrap_or_else(|| ".".to_string());
                let config_path = Path::new(&p);
                validate_config_path(config_path)?;
                let out = output.unwrap_or_else(|| {
                    config_path.join("artifacts/grafana-dashboard.json").to_string_lossy().to_string()
                });
                preview::export_dashboard(config_path, Path::new(&out))
            }
        }
    }
}
