use anyhow::Result;
use clap::Parser;
use spinoff::{Spinner, spinners, Color};
use std::process::{Command, Stdio};
use log::{debug, error, info, trace, warn};
use log::Level::Error;

pub fn tmp() -> Result<()> {

    let mut spinner = Spinner::new(spinners::Dots, "Deploying Terraform...", Color::Blue);
    let status1 = Command::new("terraform").arg("-chdir=../infra").arg("apply").arg("-auto-approve").output()?;

    // if status1.stderr.is_empty() {
    //     spinner.success("Terraform Done!")
    // } else {
    //     let err_msg = String::from_utf8(status1.stderr)?;
    //     error!("{}", err_msg);
    //     std::process::exit(1);
    // }

    let mut spinner = Spinner::new(spinners::Dots, "Maven Time...", Color::Blue);
    Command::new("mvn").arg("-f ../lib/java/pipe-1/pom.xml").arg("compile exec:java").spawn()?;
    // spinner.success("Maven Done!");


    Ok(())
}