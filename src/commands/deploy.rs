use std::fs::{File, OpenOptions};
use std::path::Path;

use anyhow::{anyhow, Result};
use serde_yaml::{Mapping, Value};

use crate::lib::{bq::{
    self
}, config::Config, crj, gcs, };
use crate::lib::resources::Resources;
use crate::lib::utilities::{generate_vector_config, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    validate_config_path(&Path::new(path_arg))?;
    bq::check_for_bq()?;
    bq::check_for_gcloud()?;

    let path = Path::new(path_arg);
    let config: Config = Config::from_path(&path);
    let mut resources: Resources =  serde_yaml::from_reader(
        File::open(path.join("artifacts/resources.yaml"))?
    )?;

    resources.biq_query.borrow_mut().as_mut().unwrap().create(&config)?;
    resources.output_pubsub.borrow_mut().as_mut().unwrap().create(&resources, &config)?;
    generate_vector_config(&path, &resources, &config)?;

    gcs::create_bucket(&resources, &config)?;
    gcs::upload_to_bucket(
        path
            .join("artifacts/vector.yaml")
            .to_str()
            .ok_or(anyhow!("path `<config>/artifacts/vector.yaml`"))?,
        &resources, &config)?;

    crj::create_vector(&resources, &config)?;
    crj::execute_crj(&resources, &config)?;

    Ok(())
}


