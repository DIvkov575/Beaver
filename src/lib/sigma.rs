use anyhow::Result;
use std::io::{Read, Stderr, stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};


pub fn create_pysigma_converter(path_to_config_dir: &Path) -> Result<()> {
    /// Creates virtualenv, activates env, installs matano-pysigma-backend from "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"
    let path = path_to_config_dir.join("detections").join("install.sh");
    Command::new("sh").args([path, path_to_config_dir.join("detections")]).spawn().unwrap().wait_with_output()?;

    Ok(())
}

pub fn generate_detections(path_to_config: &Path) -> Result<()> {
    let path = path_to_config.join("detections");
    // let venv_activation_path= path.join("venv").join("bin").join("activate").canonicalize()?;
    let venv_activation_path= PathBuf::from("venv").join("bin").join("activate");

    let mut child = Command::new("sh").stdin(Stdio::piped()).spawn()?;
    child.stdin.as_mut().unwrap().write_fmt(format_args!("cd {} &&", path.to_str().unwrap()))?;
    child.stdin.as_mut().unwrap().write_fmt(format_args!("source {} &&", venv_activation_path.to_str().unwrap()))?;
    child.stdin.as_mut().unwrap().write_fmt(format_args!("python3 sigma_generate.py se.yml"))?;
    // for node in read_dir(&detections_path)?.map(|x|x.unwrap().path()) {
    //     let file_name= node.file_name().unwrap();
    //     let extension = node.extension().unwrap().to_str().unwrap();
    //     if (extension == "yaml") ||(extension == "yml") {
    //         child.stdin.as_mut().unwrap().write_fmt(format_args!("python3 sigma_generate.py {:?}", file_name))?;
    //     }
    // }

    println!("{:?}", child.stdout.as_ref().unwrap());
    child.kill()?;

    Ok(())
}