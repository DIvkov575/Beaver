mod crj;
mod config;

use std::collections::HashMap;
use std::error::Error;
use std::iter::Map;
use std::process::{Command, Stdio};
use crj::*;
use config::*;
use serde_yaml;
use serde_yaml::Mapping;

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new("us-east1", "neon-circle-400322");
    let mut b: Mapping = serde_yaml::from_reader(std::fs::File::open("tmp.yaml")?)?;

    mount_gcs_crj("", "", &mut b).unwrap();





    Ok(())
}




