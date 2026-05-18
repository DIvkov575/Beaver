//! Stage rule YAMLs and the Dataflow entrypoint for sigma_beam.
//!
//! Before sigma_beam this module AST-spliced per-rule predicate functions
//! into `detections_template.py`. The new sigma_beam pipeline loads rules
//! from GCS at worker startup, so the codegen now collapses to a copy
//! step: stage the validated rule YAMLs under `artifacts/rules/` for
//! upload, and copy `detections_template.py` to `artifacts/detections_gen.py`
//! so the rest of the deploy machinery keeps working unchanged.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use log::info;

/// Stage everything Dataflow needs into `<config>/artifacts/`:
///   - `artifacts/detections_gen.py` — pipeline entrypoint (= detections_template.py)
///   - `artifacts/rules/*.yml`       — Sigma rule YAMLs (copied from `detections/input/`)
pub fn generate_detections_file(config_path: &Path) -> Result<()> {
    info!("staging Dataflow artifacts for sigma_beam...");

    let detections_dir = config_path.join("detections");
    let artifacts = config_path.join("artifacts");
    let rules_dir = artifacts.join("rules");

    fs::create_dir_all(&rules_dir)
        .with_context(|| format!("creating {}", rules_dir.display()))?;

    let template_src = detections_dir.join("detections_template.py");
    let template_dst = artifacts.join("detections_gen.py");
    fs::copy(&template_src, &template_dst)
        .with_context(|| format!("copying {} → {}",
                                  template_src.display(), template_dst.display()))?;
    info!("staged template → {}", template_dst.display());

    // Copy every *.yml / *.yaml from detections/input/ into artifacts/rules/.
    // sigma_beam will upload that directory to GCS later in the deploy flow.
    let input_dir = detections_dir.join("input");
    let mut staged = 0usize;
    if input_dir.is_dir() {
        for entry in fs::read_dir(&input_dir)? {
            let entry = entry?;
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("yml") | Some("yaml") => {
                    let name = path.file_name().unwrap();
                    fs::copy(&path, rules_dir.join(name))?;
                    staged += 1;
                }
                _ => {}
            }
        }
    }
    info!("staged {} rule(s) under {}", staged, rules_dir.display());
    Ok(())
}
