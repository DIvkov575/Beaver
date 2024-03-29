use std::path::Path;
use std::process::Command;
use anyhow::Result;
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;



pub fn create_template(path_to_config: &Path, resources: &Resources, config: &Config) -> Result<()> {
    /// executes sh script which loads a venv from detections -> executes beam script to upload template
    let bucket = resources.bucket_name.borrow().clone().unwrap();
    let detections_path = path_to_config.join("detections");
    let staging = format!("gs://{}/staging", bucket);
    let templates= format!("gs://{}/templates/{}", bucket, "template1");

    let args = vec![
        detections_path.to_str().unwrap().to_string(),
        config.project.clone(),
        staging,
        templates,
        config.region.clone(),
    ];

    let (code, output, error) = run_script::run(
        r#"
        cd $1
        source venv/bin/activate
        python ../artifacts/detections_gen.py \
            --runner DataflowRunner \
            --project $2 \
            --staging_location $3 \
            --template_location $4 \
            --region $5
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    println!("output:{:?}, err:{}", output, error);

    Ok(())
}

fn execute_template(resources: &Resources, config: &Config) -> Result<()> {
    // dataflow.jobs.get
    // dataflow.workItems.lease
    // dataflow.workItems.update
    // dataflow.workItems.sendMessage
    // dataflow.streamingWorkItems.getWork
    // dataflow.streamingWorkItems.commitWork
    // dataflow.streamingWorkItems.getData
    // dataflow.shuffle.read
    // dataflow.shuffle.write

    let bucket_name = resources.bucket_name.borrow().clone().unwrap();
    let template_path = format!("gs://{}/templates/beaver-detections-template", bucket_name);
    let args = vec!["dataflow", "jobs", "run", "beaver-detections", "--gcs-location", template_path, "--region", config.region, "--enable-streaming-engine"];
    Command::new("gcloud").args(args).output()?;

    Ok(())
}