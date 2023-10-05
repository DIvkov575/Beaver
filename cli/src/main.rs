use anyhow::{Result};
use clap;
mod config_parsing;
mod config;


fn main() -> Result<()> {
    config_parsing::handle_creation("../test")?;
    
    Ok(())
}