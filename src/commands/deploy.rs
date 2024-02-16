use std::cell::RefCell;
use std::fs::{File};
use std::io::read_to_string;
use std::path::Path;
use anyhow::{anyhow, Result};
use clap::builder::Resettable::Reset;
use crate::lib::{bq::{
    self
}, config::Config, crj, dataflow, detections_gen, gcs, pubsub};
use crate::lib::resources::Resources;
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    /// Command reads configration (generated with init) from a path from arg "path_arg" and creates resources within gcloud
    validate_config_path(&Path::new(path_arg))?;
    check_for_bq()?;
    check_for_gcloud()?;

    let path = Path::new(path_arg);
    let resources_file = File::open(path.join("artifacts").join("resources.yaml"))?;
    let a = read_to_string(&resources_file)?;
    println!("{:?}", a);
    // let b =path.join("artifacts").join("resources.yaml").exists();



    // let vector_path_binding = path.join("artifacts/vector.yaml");
    // let vector_path = vector_path_binding.to_str().ok_or(anyhow!("path `<config>/artifacts/vector.yaml`"))?;

    let config: Config = Config::from_path(&path);
    let mut resources = Resources::empty(&config);
    resources.bucket_name = RefCell::new(Some(String::from("templates_1")));



    // sigma::generate_detections(&path)?;
    // detections_gen::generate_detections_file(&path)?;
    // println!("{:?}", resources);
    dataflow::create_template(&path, &resources, &config)?;
    dataflow::execute_template(&path, &resources, &config)?;


    // bq::create(&resources, &config)?;
    // pubsub::create_output_pubsub(&resources, &config)?;
    // utilities::generate_vector_config(&path, &resources, &config)?;
    // gcs::create_bucket(&resources, &config)?;
    // gcs::upload_to_bucket(vector_path, &resources, &config)?;
    // crj::create_vector(&resources, &config)?;
    // crj::execute_crj(&resources, &config)?;

    // resources.save();

    Ok(())
}


