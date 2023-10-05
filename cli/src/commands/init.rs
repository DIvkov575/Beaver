use std::{path::Path, thread::panicking};
use std::fs::File;
use anyhow::Result;
use inquire::{Text, Select, validator::StringValidator};

pub fn _parse_config_dir(file_path: &str) -> Result<()> {
   // file_path is full file_path to directory 
    let _path = Path::new(file_path);

    Ok(())
}


pub fn create_config_dir(file_path: &str) -> Result<()> {
   // file_path is full file_path to directory 
    let path = Path::new(file_path);
    File::create(path.join("BeaverConfig.yaml"))?;
    
    Ok(())
}


pub fn init() -> Result<()> {
    let regions: Vec<&str> = vec![ "northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];

    // let config_dir_path = inquire::Text::new("Path to Beaver Config Directory:").prompt()?;
    // validate_config(&config_dir_path)?;
    println!("---- Beaver: Setup Wizard ----");
    let config_dir_name= Text::new("Configuration Directory Path:").prompt()?;
    let config_dir_path = Path::new(&std::env::current_dir()?).join(&config_dir_name);
    
     


    // if config_dir_path.is_file() {
    //     panic!("File exists at  {}", config_dir_name);
    // } else if config_dir_path.read_dir().next()
    
    
    // !Path::new(&config_dir_path).join("BeaverConfig.yaml").exists() {
    //     panic!("")
    // }
    


    
    // validate_config(&config_dir_path)?;
    // let region = Select::new("Select GCP Region:", regions).prompt()?;
    // let project = &Text::new("Enter GCP project-id:").prompt()?;
        

    Ok(())    
}