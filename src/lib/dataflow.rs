use std::path::Path;
use std::process::Command;
use anyhow::Result;
use log::{error, info, warn};
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::log_func_call;

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
