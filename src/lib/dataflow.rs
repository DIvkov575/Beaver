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
    let subscription_id = resources.output_pubsub.subscription_id_2.clone();

    let detections_path = path_to_config.join("detections");
    let staging = format!("gs://{}/staging", bucket);
    let templates= format!("gs://{}/templates/{}", bucket, "beaver-detection-template");
    // let subscription = format!("projects/{}/subscriptions/{}", &config.project, &subscription_id);
    let subscription = subscription_id;

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
            $2 \
            $3 \
            --staging_location $4 \
            --template_location $5 \
            --project $6 \
            --region $7 \
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    println!("output: {:?}", output);
    warn!("{}", error);

    Ok(())
}


pub fn execute_template(resources: &Resources, config: &Config) -> Result<()> {
    log_func_call!();
    info!("executing dataflow template...");
    // dataflow.jobs.get
    // dataflow.workItems.lease
    // dataflow.workItems.update
    // dataflow.workItems.sendMessage
    // dataflow.streamingWorkItems.getWork
    // dataflow.streamingWorkItems.commitWork
    // dataflow.streamingWorkItems.getData
    // dataflow.shuffle.read
    // dataflow.shuffle.write

    let bucket_name = resources.bucket_name.clone();
    let template_path = format!("gs://{}/templates/beaver-detection-template", bucket_name);
    
    // Use the pipeline name from resources, or a default if not set
    let job_name = if resources.dataflow_pipeline_name.is_empty() {
        "beaver-detections"
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
        
        // Generate a random string for the pipeline name
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        pipeline_name = format!("beaver-detections-{random_string}");
        
        info!("Creating and running dataflow pipeline: {}...", pipeline_name);
        
        let args = vec![
            "dataflow", "flex-template", "run", &pipeline_name,
            "--template-file-gcs-location", &template_path,
            "--region", &config.region,
            "--enable-streaming-engine"
        ];
        let flags: Vec<&str> = Vec::from(["--project", &config.project]);

        let output = Command::new("gcloud").args(args).args(flags).output()?;
        
        if output.stderr.len() > 0 {
            error!("{:?}", String::from_utf8(output.stderr.clone())?);
        }
        
        if output.status.success() {
            log_output(&output)?;
            info!("Pipeline {} created and running", pipeline_name);
            
            // Save the pipeline name to resources
            resources.dataflow_pipeline_name = pipeline_name.clone();
            
            // After creating the pipeline, execute the template
            execute_template(resources, config)?;
            
            break;
        } else {
            warn!("Failed to create pipeline {}, retrying...", pipeline_name);
            continue;
        }
    }
    
    Ok(())
}