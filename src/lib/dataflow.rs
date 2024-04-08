use std::path::Path;
use std::process::Command;
use anyhow::Result;
use log::{error, info, warn};
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;



pub fn create_template(path_to_config: &Path, resources: &Resources, config: &Config) -> Result<()> {
    info!("creating template...");
    /// executes sh script which loads a venv from detections -> executes beam script to upload template
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
            --region $6 \
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    println!("output: {:?}", output);
    // println!("error: {:?}", error);
    warn!("{}", error);

    Ok(())
}

pub fn execute_template(resources: &Resources, config: &Config) -> Result<()> {
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
    println!("{:?}", template_path);
    let args = vec!["dataflow", "jobs", "run", "beaver-detections", "--gcs-location", &template_path, "--region", &config.region, "--enable-streaming-engine"];
    Command::new("gcloud").args(args).output()?;

    Ok(())
}