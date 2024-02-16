use std::path::Path;
use anyhow::Result;
use run_script::ScriptOptions;
use crate::lib::config::Config;
use crate::lib::resources::Resources;

pub fn create_template(path_to_config: &Path, resources: &Resources, config: &Config) -> Result<()> {
    /// executes sh script which loads a venv from detections -> executes beam script to upload template
    let bucket = resources.bucket_name.borrow().clone().unwrap();
    let detections_path = path_to_config.join("detections");
    let staging = format!("gs://{}/staging", bucket);
    let templates= format!("gs://{}/templates", bucket);

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

fn execute_template(config_path: &Path, resources: &Resources, config: &Config) -> Result<()> {
    todo!()
}