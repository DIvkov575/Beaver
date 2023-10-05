// #![feature(fs_try_exists)]
use std::{fs, thread::panicking};
use anyhow::{Context, Result};

fn create_config_dir(file_path: String) -> Result<()>  {
    let path = std::path::Path::new(&file_path);
    if path.exists() {
        if path.is_file() {
            panic!("File exists in place of dir");
        }
        let files = path.read_dir()?;

        for file in files {
           let a = file.unwrap().file_name();
            print!("{:#?}", a);
        }
        
        

    }

    
    



    Ok(())
}

fn main() {
    create_config_dir("test".to_string()).unwrap();
}