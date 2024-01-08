use std::cell::{Cell, RefCell};
use std::error::Error;
use std::fmt::format;
use std::fs::{File, OpenOptions};
use std::io::Stdout;
use std::net::Shutdown::Read;
use std::path::Path;
use std::process::{exit, Stdio};

use anyhow::{anyhow, Result};
use serde_yaml::{Mapping, Value};

use crate::lib::{
    config::Config,
    pubsub, bq::{
        BqTable,
        self
    },
    gcs,
    resources,
    crj,
    cron,
};
use crate::lib::pubsub::PubSub;
use crate::lib::resources::Resources;
macro_rules! get {
    ($config: ident,  $($b:literal,)*) => {
        &$config$([&Value::String($b.into())])*.clone().as_str().unwrap().to_owned()
    };
}

pub fn deploy(path_arg: &str) -> Result<()> {

    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    let beaver_config: Mapping = serde_yaml::from_reader(File::open(path.join("beaver_config.yaml"))?)?;
    // let region =beaver_config[&Value::String("region".into())].clone().as_str().unwrap().to_owned();
        // &beaver_config.as_ref()[&Value::String("project_id".into())].clone().as_str().unwrap().to_owned(),
    let region = get!(beaver_config, "region",);
    let project_id= get!(beaver_config, "project_id",);

    let config: Config = Config::new(
        &region,
        &region,
        None
    );

    let mut resources: Resources = Resources::empty();
    resources.config_path = path.to_str().unwrap().to_string();
    resources.biq_query = Some(RefCell::new(BqTable::new(config.project, "beaver_data_warehouse", "table1")));
    resources.crj_instance = RefCell::new(String::from("beaver-vector-instance-1"));

    // bq::check_for_bq()?;
    // bq::create_dataset(&resources, &config)?;
    // bq::create_table(&resources, &config)?;
    pubsub::create_pubsub_to_bq_subscription(&resources, &config)?;
    generate_vector_config(&path, &resources, &config)?;

    gcs::create_bucket(&resources, &config)?;
    gcs::upload_to_bucket(
        path
            .join("artifacts/vector.yaml")
            .to_str()
            .ok_or(anyhow!("path `<config>/artifacts/vector.yaml`"))?,
        &resources, &config
    )?;

    crj::create_vector(&resources, &config)?;

    // let scheduler = "11 * * * *";
    // cron::create_scheduler(scheduler, &resources, &config)?;





    Ok(())
}

#[macro_export]


fn generate_vector_config(path: &Path, resources: &Resources, config: &Config ) -> Result<()> {
    let beaver_config: Mapping = serde_yaml::from_reader(&File::open(path.join("beaver_config.yaml"))?)?;
    let vector_config_file = OpenOptions::new().write(true).create(true).open(path.join("artifacts/vector.yaml"))?;
    let output_pubsub = resources.output_pubsub.as_ref().unwrap().borrow();
    let sources_yaml =  beaver_config[&Value::String("sources".into())].clone();
    let transforms_yaml =  beaver_config[&Value::String("transforms".into())].clone();

    let transforms = transforms_yaml
        .as_mapping()
        .iter()
        .map(|mapping| mapping
            .iter()
            .map(|(key ,value)|
                key.as_str().unwrap().to_string()
            ).collect::<Vec<String>>()[0].clone())
        .map(|x| Value::String(x))
        .collect::<Vec<Value>>();


    let sinks_yaml: Value = Value::Mapping(Mapping::from_iter([
        (Value::String("bq_writing_pubsub".into()), Value::Mapping(Mapping::from_iter([
            (Value::String("type".into()), Value::String("gcp_pubsub".into())),
            (Value::String("inputs".into()), Value::Sequence(serde_yaml::Sequence::from(transforms))),
            (Value::String("project".into()), Value::String(config.project.into())),
            (Value::String("topic".into()), Value::String(output_pubsub.topic_id.clone().into())),
            (Value::String("encoding".into()), Value::Mapping(Mapping::from_iter([
                ("codec".into(), "json".into())
            ]))),
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

fn validate_config_path(path: &Path) -> Result<()> {
    if !path.join("beaver_config.yaml").exists() {
        return Err(anyhow::anyhow!("config path does not exist or broken"));
    }
    Ok(())
}