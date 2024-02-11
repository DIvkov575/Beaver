use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use include_bytes_zstd::include_bytes_zstd;
use inquire::{Select, Text};
use spinoff::{Color, Spinner, spinners};
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::sigma::create_pysigma_converter;

pub fn init(force: bool, dev:bool, path: Option<String>) -> Result<()> {
    let regions: Vec<&str> = vec!["northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];
    let region;
    let project;

    println!("\n---- Beaver: Setup Wizard 🧙‍----");

    if dev {
        region = "";
        project = String::from("");
    } else {
        region = Select::new("Select GCP Region:", regions).prompt()?;
        project = Text::new("Enter GCP project-id:").prompt()?;
    }
    loop {
        let config_path: String;
        match path {
            Some(x) => config_path = x,
            None => config_path = Text::new("Enter config dir name:").prompt()?,
        }

        let path = PathBuf::from(&config_path);
        if path.exists() & !force  {
            println!("Directory already exists");
            std::process::exit(0);
        } else if path.exists() & force{
            println!("Directory already exists... ");
            println!("Deleting dir: {:?}", path.as_os_str());
            fs::remove_dir_all(path)?;
        }

        // let mut spinner = Spinner::new(spinners::Dots, "Creating Config Dir...", Color::Blue);
        create_config_dir(&config_path, region, &project)?;
        // spinner.success("Config Directory Created");
        break;
    }

    Ok(())
}

pub fn create_config_dir(file_path: &str, region: &str, project: &str) -> Result<()> {
   /// creates all necessary files and subdirectories for config dir
   /// Creates dirs (detections, artifacts, log)
   /// Creates files(
   /// - artifacts/vector.yaml -
   /// - artifacts/resources.yaml - resource names/ids
   /// )
   /// log/log1.log - output file for beaver internal logs during startup
   /// logging_config.yaml - log.rs config for beaver internal logs during startup
   /// sigma_generate.py - python script for converting sigma rules into beaver compatible detections
   /// beaver_config.yaml - general config file


    let path = Path::new(file_path); // path to config dir
    fs::create_dir_all(&path)?;
    fs::create_dir_all(path.join("detections"))?;
    fs::create_dir_all(path.join("artifacts"))?;
    File::create(path.join("artifacts/vector.yaml"))?;
    File::create(path.join("artifacts/resources.yaml"))?;

    let config: Config = Config::new(region, project, None);
    let config_file = format!("\
beaver:
    project_id: {project}
    region: {region}


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

    let mut resources = Resources::empty(&config);
    resources.config_path = path.as_os_str().to_str().unwrap().to_string();
    resources.save();

    let mut beaver_conf_file = OpenOptions::new().write(true).create(true).open(path.join("beaver_config.yaml")).unwrap();
    beaver_conf_file.write(config_file.as_bytes())?;

    let mut sigma_generate= OpenOptions::new().write(true).create(true).open(path.join("detections").join("sigma_generate.py")).unwrap();
    sigma_generate.write(&include_bytes_zstd!("src/beaver_config/detections/sigma_generate.py", 21))?;

    let mut sigma_install_file = OpenOptions::new().write(true).create(true).open(path.join("detections").join("install.sh")).unwrap();
    sigma_install_file.write(&include_bytes_zstd!("src/beaver_config/detections/install.sh", 21))?;

    let mut sigma_install_file = OpenOptions::new().write(true).create(true).open(path.join("detections").join("generate.sh")).unwrap();
    sigma_install_file.write(&include_bytes_zstd!("src/beaver_config/detections/generate.sh", 21))?;

    //TODO: testing purposes
    let mut test_sigma_files = OpenOptions::new().write(true).create(true).open(path.join("detections").join("se.yml")).unwrap();
    test_sigma_files.write(&include_bytes_zstd!("src/beaver_config/detections/se.yml", 21))?;

    create_pysigma_converter(path)?;


    Ok(())
}
