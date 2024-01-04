
mod crj;
mod config;
mod gcs;
mod bq;

use anyhow::Result;
use std::collections::HashMap;
use std::error::Error;
use std::iter::Map;
use std::panic::panic_any;
use std::process::{Command, Output, Stdio};
use crj::*;
use config::*;
use serde_yaml;
use serde_yaml::Mapping;
use crate::bq::*;
use crate::gcs::Bucket;

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new("us-east1", "neon-circle-400322");

    // Bucket::attempt_create("bucket", &config)?;
    // let a = Bucket::create_bucket(&config)?;
    // println!("{:?}", a);


    check_for_python()?;


    Ok(())
}


pub fn check_for_python() -> Result<(String, String)> {
    //!  - Inner line doc
    ///  - Outer line doc (exactly 3 slashes)
    /// Output Result< python command, pip command >

    let py_version: Option<String>;
    let py3_version: Option<String>;
    let pip_version: Option<String>;
    let pip3_version: Option<String>;

    let mut py_command;
    let mut pip_command;

    match Command::new("python").arg("--version").output() {
        Ok(output) => py_version = Some(String::from_utf8(output.stdout)?),
        Err(E) => py_version = None,
    }
    match Command::new("python3").arg("--version").output() {
        Ok(output) => py3_version = Some(String::from_utf8(output.stdout)?),
        Err(E) => py3_version = None,
    }
    match Command::new("pip").arg("--version").output() {
        Ok(output) => pip_version = Some(String::from_utf8(output.stdout)?),
        Err(E) => pip_version = None,
    }
    match Command::new("pip3").arg("--version").output() {
        Ok(output) => pip3_version = Some(String::from_utf8(output.stdout)?),
        Err(E) => pip3_version = None,
    }

    if py_version.is_none() && py3_version.is_none() { panic!("Please have python installed on your system as `python` or `python3`"); }
    if pip_version.is_none() && pip3_version.is_none() { panic!("Please have pip installed on your system as `pip` or `pip3`"); }

    if py3_version.is_some() { py_command = "python3".to_string(); }
    else { py_command = "python".to_string()}

    if pip3_version.is_some() { pip_command = "pip3".to_string(); }
    else { pip_command = "pip".to_string()}

    Ok((py_command, pip_command))
}
