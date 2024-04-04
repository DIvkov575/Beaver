use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::process::Command;
use log::info;
use serde_yaml::{Mapping, Value};
use crate::lib::config::Config;
use crate::lib::resources::Resources;

#[macro_export]
macro_rules! get {($config: ident,  $($b:literal,)*) => {
    // $config - Serde_yaml Mapping
    // $b - indexs
    $config$([&Value::String($b.into())])*.clone()
};}

pub fn generate_vector_config(path: &Path, resources: &Resources, config: &Config ) -> Result<()> {
    info!("generating vector config...");

    let beaver_config: Mapping = serde_yaml::from_reader(&File::open(path.join("beaver_config.yaml"))?)?;
    let vector_config_file = OpenOptions::new().write(true).create(true).open(path.join("artifacts/vector.yaml"))?;

    let output_pubsub= &resources.output_pubsub;

    let sources_yaml = get!(beaver_config, "sources",);
    let transforms_yaml = get!(beaver_config, "transforms",);
    let transforms = get_transforms(&transforms_yaml);



    // !! add condition for batch insert
    if let Some(batch) = get!(beaver_config, "beaver",).get("batch") {
        if let Some(timeout_sec) = batch.get("timeout_sec") {
            println!("asdfklj");
        }
    }
    // let batch_window= get!(beaver_config, "beaver", "batch", "timeout_sec",);
    // let batch_size= get!(beaver_config, "beaver", "batch", "max_events",);


    let sinks_yaml: Value = Value::Mapping(Mapping::from_iter([
        (Value::String("bq_writing_pubsub".into()), Value::Mapping(Mapping::from_iter([
            (Value::String("type".into()), Value::String("gcp_pubsub".into())),
            (Value::String("inputs".into()), Value::Sequence(serde_yaml::Sequence::from(transforms))),
            (Value::String("project".into()), Value::String(config.project.clone().into())),
            (Value::String("topic".into()), Value::String(output_pubsub.topic_id.clone().into())),
            (Value::String("encoding".into()), Value::Mapping(Mapping::from_iter([
                ("codec".into(), "json".into())
            ]))),
            // (Value::String("batch".into()), Value::Mapping(Mapping::from_iter([
            //     ("timeout_sec".into(), batch_window),
            //     ("max_events".into(), batch_size),
            // ]))),

        ])))
    ]));

    let vector_config: Mapping = Mapping::from_iter([
        ("sources".into(), sources_yaml),
        ("transforms".into(), transforms_yaml),
        ("sinks".into(), sinks_yaml)
    ]);

    serde_yaml::to_writer(&vector_config_file, &vector_config).unwrap();

    Ok(())
}

fn get_transforms(transforms_yaml: &Value) -> Vec<Value> {
    let transforms = transforms_yaml
        .as_mapping()
        .iter()
        .map(|mapping| mapping
            .iter()
            .map(|(key, value)|
                key.as_str().unwrap().to_string()
            ).collect::<Vec<String>>()[0].clone())
        .map(|x| Value::String(x))
        .collect::<Vec<Value>>();
    transforms
}

pub fn validate_config_path(path: &Path) -> anyhow::Result<()> {
    if !path.join("../beaver_config/beaver_config.yaml").exists() {
        return Err(anyhow::anyhow!("config path does not exist or broken"));
    }
    Ok(())
}
pub fn check_for_python3() -> anyhow::Result<()> {
    match Command::new("python3").arg("--version").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have gcloud (google-cloud-sdk) installed"),
    }
}
pub fn check_for_gcloud() -> anyhow::Result<()> {
    match Command::new("gcloud").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have gcloud (google-cloud-sdk) installed"),
    }
}
pub fn check_for_bq() -> anyhow::Result<()> {
    match Command::new("bq").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have bq (biqquery utility tool installed)"),
    }
}

pub fn overlap<T: Eq>(a: &[T], b:&[T]) -> bool {
    for n1 in a{
        for n2 in b{
            if n1 == n2 {
                return true
            }
        }
    }
    return false
}