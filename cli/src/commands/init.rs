use std::{path::Path, thread::panicking};
use std::fs::File;
use anyhow::Result;
use inquire::{Text, Select, validator::StringValidator};



pub fn create_config_dir(file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    File::create(path.join("BeaverConfig.yaml"))?;
    
    Ok(())
}


pub fn init(option_path: Option<String>) -> Result<()> {
    let regions: Vec<&str> = vec![ "northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];

    println!("---- Beaver: Setup Wizard ----");

    if let Some(path) = &option_path {
        panic!("passing a path not yet implemented {}", path);
    } else {
        let _config_dir_name= Text::new("Configuration Directory Path:").prompt()?;
        let config_dir_path = Path::new(&std::env::current_dir()?).join(&_config_dir_name);
        std::fs::create_dir_all(&config_dir_path)?;
        create_config_dir(config_dir_path.to_str().unwrap())?;
        
        let region = Select::new("Select GCP Region:", regions).prompt()?;
        let project = &Text::new("Enter GCP project-id:").prompt()?;

    }
    
        

    Ok(())    
}