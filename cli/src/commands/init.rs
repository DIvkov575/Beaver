use std::path::Path;
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

pub fn validate_config(file_path: &str) -> Result<()>  {
    // file path is relative env 
    let path = Path::new(&std::env::current_dir()?).join(file_path);

    if path.exists() {
        if path.is_file() { 
            // println!("File already exists at {}", path.to_str().unwrap());
            
            
        }        // if file
        if path.join("BeaverConfig.yaml").exists() { return Ok(());   // if valid beaver config
        } else if path.read_dir()?.next().is_none() {                      // if dir empty
            create_config_dir(&path.to_str().unwrap())?; 
        } else {
            panic!("Something went wrong (probably invalid config)");      // if dir populated w/ wrong files
        }

    } else {
        std::fs::create_dir(&path)?;
        create_config_dir(&path.to_str().unwrap())?;
    }

    Ok(())
}


#[derive(Clone, Debug)]
struct ConfigValidator { }
impl StringValidator for ConfigValidator {
    fn validate(&self, input: &str) -> std::result::Result<inquire::validator::Validation, inquire::CustomUserError> {
        let path = Path::new(&std::env::current_dir()?).join(input);
        if path.exists() {
            if path.is_file() {
                // return inquire::validator::Validation::Invalid("Path is file".into());
                return Err("Path Provided was a file".into());

            } else if path.read_dir()?.next().is_none() {
                create_config_dir(&path.to_str().unwrap())?; 
                print!("Config directory created at {:#?}", path);
                return Ok(inquire::validator::Validation::Valid);
            }
        } else {
            return Ok(inquire::validator::Validation::Valid);
        }
        
    }
}

pub fn init() -> Result<()> {
    let regions: Vec<&str> = vec![ "northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];

    // let config_dir_path = inquire::Text::new("Path to Beaver Config Directory:").prompt()?;
    // validate_config(&config_dir_path)?;
    println!("---- Beaver: Setup Wizard ----");
    let config_dir_path = Text::new("Configuration Directory Path:")
        .with_validator(ConfigValidator{})
        .prompt()?;

    
    // validate_config(&config_dir_path)?;
    // let region = Select::new("Select GCP Region:", regions).prompt()?;
    // let project = &Text::new("Enter GCP project-id:").prompt()?;
        

    Ok(())    
}