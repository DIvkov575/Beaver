use anyhow::Result;
use spinoff::{Spinner, spinners, Color};
use std::process::Command;

pub fn deploy() -> Result<()> {

    let mut spinner = Spinner::new(spinners::Dots, "Deploying Terraform...", Color::Blue); 
        Command::new("terraform").arg("-chdir=infra").arg("apply").spawn()?;
    spinner.success("Done!");

    Ok(())
}