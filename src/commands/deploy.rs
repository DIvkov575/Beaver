use std::path::Path;
use std::process::exit;
use anyhow::{anyhow, Result};
use log::info;
use spinoff::{Color, Spinner, spinners};
use crate::lib::{bq::{
    self
}, config::Config, crs, dataflow, detections_gen, gcs, pubsub, cloud_build};
use crate::lib::resources::{Resources, Tracker};
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    info!("=======New Deployment======");
    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let config: Config = Config::from_path(&path);
    let mut resources = Resources::empty(&config, &path);
    let mut tracker = Tracker::new(&mut resources);

    sigma::generate_detections(&path)?;
    detections_gen::generate_detections_file(&path)?;

    bq::create(&mut tracker, &config)?;
    pubsub::create(&mut tracker, &config)?;
    gcs::create_bucket(&mut tracker, &config)?;

    utilities::generate_vector_config(&path, tracker.resources(), &config)?;
    cloud_build::create_docker_image(&path, &mut tracker, &config)?;
    crs::create_vector(&mut tracker, &config)?;

    dataflow::create_template(&path, &mut tracker, &config)?;
    dataflow::create_pipeline(&mut tracker, &config)?;

    Ok(())
}
