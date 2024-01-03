use std::fs::{File, OpenOptions};
use std::path::Path;
use anyhow::Result;
use spinoff::{Spinner, spinners, Color};
use std::process::Command;
use serde_yaml::{Mapping, Value};

pub fn deploy(path_arg: &str) -> Result<()> {
    let path = Path::new(path_arg);

    validate_config_path(&path)?;
    generate_vector_config(&path)?;


    Ok(())
}

fn generate_vector_config(path: &Path) -> Result<()> {
    let beaver_config_file = File::open(path.join("beaver_config.yaml"))?;
    let mut vector_config_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path.join("assets/vector.yaml"))
        .unwrap();

    let beaver_config: Mapping = serde_yaml::from_reader(&beaver_config_file)?;
    let vector_config: Mapping = Mapping::from_iter([
        ("sources".into(), beaver_config[&Value::String("sources".into())].clone()),
        ("transforms".into(), beaver_config[&Value::String("transforms".into())].clone()),
    ]);

    serde_yaml::to_writer(&vector_config_file, &vector_config).unwrap();



    Ok(())
}

fn validate_config_path(path: &Path) -> Result<()> {
    if !path.join("beaver_config.yaml").exists() {
        return Err(anyhow::anyhow!("config path does not exist or broken"));
    }
    Ok(())
}