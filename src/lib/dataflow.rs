use std::path::Path;
use anyhow::Result;
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;

pub fn create_template(path_to_config: &Path, resources: &Resources, config: &Config) -> Result<()> {
    /// executes sh script which loads a venv from detections -> executes beam script to upload template


    let bucket = resources.bucket_name.borrow().unwrap();
    let detections_path = path_to_config.join("detections");
    let args = vec![
        detections_path.to_str().unwrap().to_string(),
        config.project,
        format!("gs://{bucket}/staging"),
        format!("gs://{bucket}/templates/"),
        config.region,
    ];

    let (code, output, error) = run_script::run(
        r#"
        cd $1
        source venv/bin/activate
        python -m ../artifacts/detections_gen \
            --runner DataflowRunner \
            --project $2 \
            --staging_location $3 \
            --template_location $4 \
            --region $5
        "#,
        &args,
        &ScriptOptions::new(),
    )?;

    // python -m detections_gen \
    // --runner DataflowRunner \
    // --project neon-circle-400322 \
    // --staging_location gs://templates_1/staging \
    // --template_location gs://templates_1/templates/class_template_1 \
    // --region us-east1


    Ok(())
}