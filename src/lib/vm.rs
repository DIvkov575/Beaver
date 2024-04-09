use std::process::Command;
use anyhow::Result;
use clap::parser::ValueSource::CommandLine;
use rand::distributions::Alphanumeric;
use rand::Rng;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::MiscError;

const vector_template_name: &str =  "beaver-vector-template";
pub fn create_mig_template(config: &Config, resources: &mut Resources) -> Result<()> {
    let args = vec!["compute", "instances", "instance-template", "create-with-container", vector_template_name, "--container-image", "docker.io/timberio/vector:latest-alpine"];
    let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
    log_output(&output)?;
    Ok(())
}

pub fn exec_mig_template(config: &Config, resources: &Resources) -> Result<()> {
    // gcloud compute instance-groups managed create example-group \
    // --base-instance-name nginx-vm \
    // --size 3 \
    // --template nginx-template,

    let x = "vector-mig-1";
    let x1 = "beaver-vector-1";
    let size = 1;
    let args = vec!["compute", "instance-groups", "managed", "create", x,
                    "--base-instance-name", x1,
                    "--size", size,
                    "--template", vector_template_name];
    let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
    log_output(&output)?;

   Ok(())
}

