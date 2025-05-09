use std::path::Path;
use std::process::exit;
use anyhow::{anyhow, Result};
use log::info;
use spinoff::{Color, Spinner, spinners};
use crate::lib::{bq::{
    self
}, config::Config, crs, dataflow, detections_gen, gcs, pubsub, cloud_build, cron};
use crate::lib::resources::Resources;
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    /// Command reads configration (generated with init) from a path from arg "path_arg" and creates resources within gcloud
    info!("=======New Deployment======");
    let mut spinner: Spinner;
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
    gcs::create_bucket(&mut resources, &config)?;


    utilities::generate_vector_config(&path, &resources, &config)?;
    cloud_build::create_docker_image(&path, &mut resources, &config)?;
    crs::create_vector(&mut resources, &config)?;

    cron::create_crs_restart_schedule(&mut resources, &config)?;

    // dataflow::create_template(&path, &resources, &config)?;
    // dataflow::create_pipeline(&mut resources, &config)?;

    resources.save();

    Ok(())
}

    // spinner = Spinner::new(spinners::Dots, "generating detections...", Color::Blue);
    // spinner.success("detections generated");
    // spinner = Spinner::new(spinners::Dots, "creating gcp resources...", Color::Blue);
    // spinner.success("gcp resources created");
    // spinner = Spinner::new(spinners::Dots, "creating crs vector...", Color::Blue);
    // spinner.success("CRS Vector Created");
