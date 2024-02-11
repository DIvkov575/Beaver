use anyhow::Result;
use std::io::{Read, Stderr, stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};


pub fn create_pysigma_converter(path_to_config: &Path) -> Result<()> {
    /// Creates virtualenv, activates env, installs matano-pysigma-backend from "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"
    let path = path_to_config.join("detections");
    Command::new("sh").args([path.join("install.sh"), path]).spawn().unwrap().wait_with_output()?;

    Ok(())
}

pub fn generate_detections(path_to_config: &Path) -> Result<()> {
    let path = path_to_config.join("detections");
    Command::new("sh")
        .args([path.join("generate.sh"), path])
        .spawn()?
        .wait_with_output()?;

    Ok(())
}