use std::path::Path;
use anyhow::{anyhow, Result};
use log::info;
use crate::lib::{bq::{
    self
}, config::Config, crj, dataflow, detections_gen, gcs, pubsub};
use crate::lib::resources::Resources;
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    /// Command reads configration (generated with init) from a path from arg "path_arg" and creates resources within gcloud
    info!("=======New Deployment======");
    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let _vector_path_binding = path.join("artifacts/vector.yaml");
    let vector_path = _vector_path_binding.to_str().ok_or(anyhow!("path `<config>/artifacts/vector.yaml`"))?;
    let config: Config = Config::from_path(&path);
    let mut resources = Resources::empty(&config, &path);

    sigma::generate_detections(&path)?;
    detections_gen::generate_detections_file(&path)?;

    bq::create(&mut resources, &config)?;
    pubsub::create(&mut resources, &config)?;

    utilities::generate_vector_config(&path, &resources, &config)?;
    gcs::create_bucket(&mut resources, &config)?;
    gcs::upload_to_bucket(vector_path, &resources, &config)?;

    crj::create_vector(&mut resources, &config)?;
    crj::execute_crj(&resources, &config)?;

    dataflow::create_template(&path, &resources, &config)?;
    dataflow::execute_template(&resources, &config)?;

    resources.save();

    Ok(())
}


