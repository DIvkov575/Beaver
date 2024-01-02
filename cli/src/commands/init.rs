use crate::commands::deploy::deploy;
use std::{path::Path, thread::panicking};
use std::fs::File;
use anyhow::Result;
use inquire::{Text, Select, Confirm};
use std::process::Command;
use spinoff::{Spinner, spinners, Color};





pub fn create_config_dir(file_path: &str) -> Result<()> {
    let path = Path::new(file_path);
    File::create(path.join("BeaverConfig.yaml"))?;
    
    Ok(())
}


pub fn init() -> Result<()> {
    let regions: Vec<&str> = vec![ "northamerica-northeast1", "us-west4", "southamerica-east1", "australia-southeast1", "asia-southeast2", "australia-southeast2", "asia-south1", "asia-northeast2", "australia-east", "asia-east2", "europe-north1", "asia-northeast1", "asia-east1", "europe-west2", "us-central1", "europe-west1", "us-east1", "us-east4", "southamerica-west1", "us-west2", "asia-south2", "europe-west6", "asia-southeast1", "europe-west4", "europe-north2", "europe-west3", "us-west1", "us-west3", "europe-west5", "australia-central2"];

    println!("\n---- Beaver: Setup Wizard ----");
    let region = Select::new("Select GCP Region:", regions).prompt()?;
    let project = &Text::new("Enter GCP project-id:").prompt()?;
    let config_path = &Text::new("Enter path to config directory:").prompt()?;

    // if !Confirm::new("Do you have an existing Beaver Directory (y/n)").prompt()? {
    //     let _name= Text::new("Config Directory Path:").prompt()?;
    //     let path = Path::new(&std::env::current_dir()?).join(&_name);
    //     std::fs::create_dir_all(&path)?;
    //     create_config_dir(path.to_str().unwrap())?;
    // }

    // let mut spinner = Spinner::new(spinners::Dots, "Initializing Terraform...", Color::Blue);
    //     Command::new("terraform").arg("init").output()?;
    // spinner.success("Terraform Initialized");



    deploy()?;
        
// terraform: innit, validate, plan, app,y, destroty

    Ok(())    
}