use anyhow::Result;
use std::process::Command;

pub fn destroy() -> Result<()> {
    std::process::Command::new("terraform").arg("destroy").spawn()?;
    Ok(())
}