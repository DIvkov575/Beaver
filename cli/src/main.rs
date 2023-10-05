use std::path::Path;
use std::fs::{File};
use anyhow::{Result};


fn _parse_config_dir(file_path: &str) -> Result<()> {
    let _path = Path::new(file_path);

    Ok(())
}


fn create_config_dir(file_path: &str) -> Result<()> {
   // file_path is full file_path to directory 
    let path = Path::new(file_path);
    File::create(path.join("BeaverConfig.yaml"))?;

    Ok(())
}

fn handle_creation(file_path: &str) -> Result<()>  {
    // file path is relative env 
    let path = Path::new(&std::env::current_dir()?).join(file_path);

    if path.exists() {
        if path.is_file() { panic!("File exists in place of dir"); }

        if path.join("BeaverConfig.yaml").exists() {
            return Ok(());

        } else if path.read_dir()?.next().is_none() { 
            create_config_dir(&path.to_str().unwrap())?; // if dir empty
        } else {
            panic!("Dir Not Empty");
        }

    } else {
        std::fs::create_dir(&path)?;
        create_config_dir(&path.to_str().unwrap())?;
    }

    
    



    Ok(())
}

fn main() {
    handle_creation("test").unwrap();
}