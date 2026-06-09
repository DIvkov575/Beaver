//! Targeted repair commands. Each repair re-runs a small slice of `deploy`
//! against an existing `resources.yaml`, on the assumption that upstream
//! dependencies (SAs, grants, bucket, etc.) are still healthy. Use destroy +
//! deploy if you need to rebuild everything.

use std::path::Path;
use anyhow::{anyhow, Result};
use log::info;
use spinoff::{spinners, Color, Spinner};

use crate::lib::{config::Config, dataflow, detections_gen, sigma};
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::{check_for_bq, check_for_gcloud, validate_config_path};

fn step<T, F: FnOnce() -> Result<T>>(label: &str, f: F) -> Result<T> {
    let mut sp = Spinner::new(spinners::Dots, label.to_string(), Color::Blue);
    match f() {
        Ok(v) => { sp.success(label); Ok(v) }
        Err(e) => { sp.fail(label); Err(e) }
    }
}

/// Re-runs the Dataflow template + job launch + wait-for-running steps using
/// the existing `resources.yaml`. SAs, grants, bucket and CRS are reused — if
/// those are broken, run a full destroy/deploy instead. Idempotent if the job
/// is already RUNNING.
pub fn repair_dataflow(path_arg: &str, use_cancel: bool) -> Result<()> {
    info!("=======Repairing Dataflow======");
    println!("\nBeaver repair-dataflow");
    println!("======================\n");

    let path = Path::new(path_arg);
    validate_config_path(path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let resources_path = path.join("artifacts/resources.yaml");
    if !resources_path.exists() {
        return Err(anyhow!("resources.yaml not found at {}", resources_path.display()));
    }
    let yaml = std::fs::read_to_string(&resources_path)?;
    let mut resources: Resources = serde_yaml::from_str(&yaml)?;
    // Older resources.yaml files store a relative config_path; rewrite to the
    // absolute form of the path the user passed so `Tracker.save()` works
    // from any cwd.
    if let Ok(abs) = path.canonicalize() {
        resources.config_path = abs.as_os_str().to_str().unwrap().to_string();
    }
    let config = Config::from_path(path);

    let current_name = resources.dataflow_pipeline_name.clone();
    if current_name.is_empty() {
        return Err(anyhow!(
            "no Dataflow job recorded in resources.yaml; run `beaver deploy` first"
        ));
    }

    // Probe state. If RUNNING, nothing to do.
    let state = step("probe current Dataflow state", || {
        dataflow::current_state(&current_name, &config)
    })?;
    match state.as_deref() {
        Some("Running") => {
            println!("\nDataflow job {} is already Running — nothing to repair.", current_name);
            return Ok(());
        }
        Some(other) => {
            println!("current state: {} — relaunching", other);
        }
        None => {
            println!("no job by that name in region — relaunching");
        }
    }

    // Only attempt shutdown if the job is known to be active. Jobs in terminal
    // states or jobs that no longer exist (None) don't need a stop command.
    let needs_shutdown = matches!(
        state.as_deref(),
        Some("Running") | Some("Queued") | Some("Pending") | Some("Draining")
    );

    if needs_shutdown {
        if use_cancel {
            let _ = step("cancel existing job (immediate)", || {
                dataflow::delete_job(&current_name, &config)
            });
        } else {
            step("drain existing job (flushing windows)", || {
                dataflow::drain_job(&current_name, &config)
            })?;
            step("wait for drain to complete (up to 10 min)", || {
                dataflow::wait_for_drained(&current_name, &config)
            })?;
        }
    }

    let mut tracker = Tracker::new(&mut resources);

    step("re-upload Dataflow template", || {
        dataflow::create_template(path, &mut tracker, &config)
    })?;
    step("launch new Dataflow streaming job", || {
        dataflow::create_pipeline(&mut tracker, &config)
    })?;
    let new_name = tracker.resources().dataflow_pipeline_name.clone();
    step("wait for Dataflow workers to come up (up to 5 min)", || {
        dataflow::wait_for_running(&new_name, &config)
    })?;

    println!("\nRepaired: Dataflow job {} is Running.\n", new_name);
    Ok(())
}

/// Recompiles Sigma rules, regenerates the detections Python module, re-uploads
/// the Dataflow template, cancels the running job, and launches a fresh one
/// with the new code. Use after editing rules in `beaver_config/sigma/`.
pub fn refresh_detections(path_arg: &str, use_cancel: bool) -> Result<()> {
    info!("=======Refreshing Detections======");
    println!("\nBeaver refresh-detections");
    println!("=========================\n");

    let path = Path::new(path_arg);
    validate_config_path(path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let resources_path = path.join("artifacts/resources.yaml");
    if !resources_path.exists() {
        return Err(anyhow!("resources.yaml not found at {}", resources_path.display()));
    }
    let yaml = std::fs::read_to_string(&resources_path)?;
    let mut resources: Resources = serde_yaml::from_str(&yaml)?;
    if let Ok(abs) = path.canonicalize() {
        resources.config_path = abs.as_os_str().to_str().unwrap().to_string();
    }
    let config = Config::from_path(path);

    let current_name = resources.dataflow_pipeline_name.clone();
    if current_name.is_empty() {
        return Err(anyhow!(
            "no Dataflow job recorded in resources.yaml; run `beaver deploy` first"
        ));
    }

    step("compile sigma rules", || {
        sigma::setup_detections_venv(path)?;
        sigma::generate_detections(path)?;
        detections_gen::generate_detections_file(path)
    })?;

    // Only attempt shutdown if the job is in an active state. Terminal or
    // unknown states (job deleted from GCP) skip straight to relaunch.
    let state = dataflow::current_state(&current_name, &config).unwrap_or(None);
    let needs_shutdown = matches!(
        state.as_deref(),
        Some("Running") | Some("Queued") | Some("Pending") | Some("Draining")
    );

    if needs_shutdown {
        if use_cancel {
            let _ = step("cancel existing job (immediate)", || {
                dataflow::delete_job(&current_name, &config)
            });
        } else {
            step("drain existing job (flushing windows)", || {
                dataflow::drain_job(&current_name, &config)
            })?;
            step("wait for drain to complete (up to 10 min)", || {
                dataflow::wait_for_drained(&current_name, &config)
            })?;
        }
    }

    let mut tracker = Tracker::new(&mut resources);

    step("re-upload Dataflow template", || {
        dataflow::create_template(path, &mut tracker, &config)
    })?;
    step("launch new Dataflow streaming job", || {
        dataflow::create_pipeline(&mut tracker, &config)
    })?;
    let new_name = tracker.resources().dataflow_pipeline_name.clone();
    step("wait for Dataflow workers to come up (up to 5 min)", || {
        dataflow::wait_for_running(&new_name, &config)
    })?;

    let mode = if needs_shutdown {
        if use_cancel { "cancelled" } else { "drained" }
    } else {
        "was already stopped"
    };
    println!(
        "\nRefreshed: old job {} {}, new job {} is Running with new detections.\n",
        current_name, mode, new_name
    );
    Ok(())
}
