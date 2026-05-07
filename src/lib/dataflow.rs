use std::path::Path;
use std::process::Command;
use anyhow::Result;
use log::{error, info, warn};
use rand::distributions::Alphanumeric;
use rand::Rng;
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::{log_func_call, MiscError};

pub fn create_template(path_to_config: &Path, resources: &Resources, config: &Config) -> Result<()> {
    log_func_call!();
    info!("creating template...");

    let bucket = resources.bucket_name.clone();
    let subscription = resources.output_pubsub.subscription_id_2.clone();

    let detections_path = path_to_config.join("detections");
    let staging = format!("gs://{}/staging", bucket);
    let template_name = "beaver-detection-template";
    let templates = format!("gs://{}/templates/{}", bucket, template_name);

    let args = vec![
        detections_path.to_str().unwrap().to_string(),
        config.project.clone(),
        subscription,
        staging,
        templates,
        config.project.to_string(),
        config.region.clone(),
    ];

    let (_, output, error) = run_script::run(
        r#"
        cd $1
        source venv/bin/activate
        python ../artifacts/detections_gen.py \
            --runner=DataflowRunner \
            --project $2 \
            --subscription $3 \
            --staging_location $4 \
            --template_location $5 \
            --region $7 \
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    println!("output: {:?}", output);
    warn!("{}", error);

    Ok(())
}


pub fn execute_template(resources: &mut Resources, config: &Config) -> Result<()> {
    log_func_call!();
    info!("executing dataflow template...");

    let bucket_name = resources.bucket_name.clone();
    let template_path = format!("gs://{}/templates/beaver-detection-template", bucket_name);

    let job_name = if resources.dataflow_pipeline_name.is_empty() {
        let default_name = "beaver-detections";
        resources.dataflow_pipeline_name = default_name.to_string();
        default_name
    } else {
        &resources.dataflow_pipeline_name
    };

    let args = vec!["dataflow", "jobs", "run", job_name, "--gcs-location", &template_path, "--region", &config.region, "--enable-streaming-engine"];
    let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
    log_output(&output)?;

    Ok(())
}

pub fn create_pipeline(resources: &mut Resources, config: &Config) -> Result<()> {
    log_func_call!();

    let mut random_string: String;
    let mut pipeline_name: String;
    let bucket_name = resources.bucket_name.clone();
    let template_path = format!("gs://{}/templates/beaver-detection-template", bucket_name);

    let mut ctr = 0usize;
    loop {
        if ctr >= 3 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        pipeline_name = format!("beaver-detections-{random_string}");

        info!("creating dataflow pipeline: {}", pipeline_name);

        let args = vec![
            "dataflow", "flex-template", "run", &pipeline_name,
            "--template-file-gcs-location", &template_path,
            "--region", &config.region,
            "--enable-streaming-engine"
        ];
        let flags: Vec<&str> = Vec::from(["--project", &config.project]);

        let output = Command::new("gcloud").args(args).args(flags).output()?;

        if !output.stderr.is_empty() {
            error!("{:?}", String::from_utf8(output.stderr.clone())?);
        }

        if output.status.success() {
            log_output(&output)?;
            resources.dataflow_pipeline_name = pipeline_name.clone();
            execute_template(resources, config)?;
            break;
        } else {
            warn!("failed to create pipeline {}, retrying", pipeline_name);
            continue;
        }
    }

    Ok(())
}