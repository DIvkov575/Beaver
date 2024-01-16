use std::fs::{File, OpenOptions};
use std::path::Path;
use anyhow::{anyhow, Result};
use crate::lib::{bq::{
    self
}, config::Config, crj, gcs, pubsub};
use crate::lib::resources::Resources;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    /// Command reads configration (generated with init) from a path from arg "path_arg" and creates resources within gcloud
    validate_config_path(&Path::new(path_arg))?;
    check_for_bq()?;
    check_for_gcloud()?;
    let path = Path::new(path_arg);
    let resources_file = File::open(path.join("artifacts/resources.yaml"))?;
    let vector_path = path.join("artifacts/vector.yaml").to_str().ok_or(anyhow!("path `<config>/artifacts/vector.yaml`"))?;
    let config: Config = Config::from_path(&path);
    let mut resources: Resources =  serde_yaml::from_reader(resources_file)?;

    bq::create(&resources, &config)?;
    pubsub::create_output_pubsub(&resources, &config)?;
    utilities::generate_vector_config(&path, &resources, &config)?;
    gcs::create_bucket(&resources, &config)?;
    gcs::upload_to_bucket(vector_path, &resources, &config)?;
    crj::create_vector(&resources, &config)?;
    crj::execute_crj(&resources, &config)?;

    resources.save();

    Ok(())
}


