use std::arch::aarch64::uint64x1_t;
use anyhow::Result;
use std::error::Error;
use std::fs::read_dir;
use std::io::{Read, Stderr, stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};
use log::{info, log};
use crate::lib::resources::Resources;


pub fn create_pysigma_converter(path_to_config_dir: &Path) -> Result<()> {
    /// Creates virtualenv, activates env, installs matano-pysigma-backend from "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"
    let path = path_to_config_dir.join("detections");
    let path_binding1 = path.join("venv");

    // let mut child = Command::new("sh").stdin(Stdio::piped())
    //     .stderr(Stdio::inherit())
    //     .stdout(Stdio::inherit())
    //     .spawn()?;
    //
    // let stdin = child.stdin.as_mut().unwrap();
    //
    // stdin.write_fmt(format_args!("cd {:?}\n", path))?;
    // stdin.write_all(b"python3 -m venv venv\n")?;
    // stdin.write_all(b"source venv/bin/activate\n")?;
    // stdin.write_all(b"pip3 list\n")?;
    //
    // println!("stdio: {:?}", child.stdout.as_ref().unwrap());
    // println!("stderr: {:?}", child.stderr.as_ref().unwrap());
    //
    //
    // child.stdin.as_mut().unwrap()
    //     .write_all(b"ls\n")?;

    // child.stdin.as_mut().unwrap().write("cd ./src/beav && echo && ls".as_bytes())?;





    // Command::new("python3").args(["-m", "venv", path_binding1.to_str().unwrap()]).spawn().unwrap().wait_with_output()?;;
    // let path_binding2 = path.join("venv").join("bin").join("activate").canonicalize()?;

    // let mut args = String::new();
    // args += &format!("source {}", path_binding2.to_str().unwrap());
    // args += "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git";

    // Command::new("sh").arg(args).stdout(Stdio::inherit()).spawn().unwrap().wait_with_output()?;
    // println!("{:?}", output);


    // let mut child = Command::new("sh").stdin(Stdio::piped()).spawn()?;
    // child.stdin.as_mut().unwrap().write_fmt(format_args!("python3 -m venv {} &&", path_binding1.to_str().unwrap()))?;
    // child.stdin.as_mut().unwrap().write_fmt(format_args!("source {} &&", path_binding2.to_str().unwrap()))?;
    // child.stdin.as_mut().unwrap().write_fmt(format_args!("pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"))?;
    child.kill()?;

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