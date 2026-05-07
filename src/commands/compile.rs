use anyhow::Result;
use std::path::Path;

use crate::lib::{detections_gen, sigma};

/// Runs the two detection-compilation stages and stops — does not touch GCP.
/// Inputs read from `<path>/detections/input/*.yml`; output ends up in
/// `<path>/detections/output/<rule>/` and `<path>/artifacts/detections_gen.py`.
pub fn compile(path_arg: &str) -> Result<()> {
    let path = Path::new(path_arg);
    sigma::generate_detections(path)?;
    detections_gen::generate_detections_file(path)?;
    Ok(())
}
