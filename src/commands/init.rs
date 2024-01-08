use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use inquire::{Select, Text};
use spinoff::{Color, Spinner, spinners};

pub fn init() -> Result<()> {
    let regions: Vec<&str> = vec!["northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];

    println!("\n---- Beaver: Setup Wizard 🧙‍----");
    let region = Select::new("Select GCP Region:", regions).prompt()?;
    let project = &Text::new("Enter GCP project-id:").prompt()?;
    loop {
        let config_path = &Text::new("Enter config dir name:").prompt()?;
        if PathBuf::from(&config_path).exists() {
            println!("Directory already exists");
        } else {
            let mut spinner = Spinner::new(spinners::Dots, "Creating Config Dir...", Color::Blue);
            create_config_dir(&config_path, region, project)?;
            spinner.success("Config Directory Created");
            break;
        }
    }

    Ok(())
}

pub fn create_config_dir(file_path: &str, region: &str, project: &str) -> Result<()> {
    println!("asdfasdf");
    let path = Path::new(file_path);
    fs::create_dir_all(&path)?;
    fs::create_dir_all(path.join("detections"))?;
    fs::create_dir_all(path.join("artifacts"))?;
    fs::create_dir_all(path.join("log"))?;
    File::create(path.join("artifacts/vector.yaml"))?;
    File::create(path.join("log/log1.log"))?;

    let config = format!("\
project_id: {project}
region: {region}
bq_location: \"us\"

sources:
  pubsub_in:
    type: gcp_pubsub
    project: \"{project}\"
    subscription: \"input-sub-1\"
    decoding:
      codec: \"json\"
transforms:
  transform1:
    type: remap
    inputs:
      - pubsub-in
");


    let mut beaver_conf_file = OpenOptions::new().write(true).create(true).open(path.join("beaver_config.yaml")).unwrap();
    beaver_conf_file.write(config.as_bytes())?;

    let mut log_conf = OpenOptions::new().write(true).create(true).open(path.join("logging_config.yaml")).unwrap();
    log_conf.write(include_bytes!("../beaver_config/logging_config.yaml"))?;


    Ok(())
}
