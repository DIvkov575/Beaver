use std::cell::{Cell, RefCell};
use std::fmt::format;
use std::fs::{File, OpenOptions};
use std::path::Path;

use anyhow::Result;
use serde_yaml::{Mapping, Value};

use crate::lib::{
    config::Config,
    pubsub, bq::{
        BqTable,
        self
    },
    resources
};
use crate::lib::pubsub::PubSub;
use crate::lib::resources::Resources;

pub fn deploy(path_arg: &str) -> Result<()> {
    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    let beaver_config: Mapping = serde_yaml::from_reader(File::open(path.join("beaver_config.yaml"))?)?;
    let region: String = beaver_config[&Value::String("region".into())].clone().as_str().unwrap().to_owned();
    let project: String = beaver_config[&Value::String("project_id".into())].clone().as_str().unwrap().to_owned();
    let config: Config = Config::new(&region, &project, None);

    let mut resources: Resources = Resources::empty();
    resources.biq_query = Some(RefCell::new(BqTable::new(config.project, "beaver_data_warehouse", "table1")));
    resources.output_pubsub = Some(RefCell::new(PubSub::empty()));

    bq::check_for_bq()?;
    // bq::create_dataset(&resources, &config)?;
    // bq::create_table(&resources, &config)?;

    pubsub::create_pubsub_to_bq_subscription(&resources, &config)?;

    // generate_vector_config(&path, &resources, &config)?;



    Ok(())
}

fn generate_vector_config(path: &Path, resources: &Resources, config: &Config ) -> Result<()> {
    let beaver_config: Mapping = serde_yaml::from_reader(&File::open(path.join("beaver_config.yaml"))?)?;
    let mut vector_config_file = OpenOptions::new().write(true).create(true)
        .open(path.join("artifacts/vector.yaml"))?;
    // let pubsub = resources.output_pubsub.as

    let sources_yaml =  beaver_config[&Value::String("sources".into())].clone();
    let transforms_yaml =  beaver_config[&Value::String("transforms".into())].clone();

    let transforms: Vec<Value> = transforms_yaml
        .as_mapping()
        .iter()
        .map(|mapping| mapping
            .iter()
            .map(|(key ,value)|
                format!("\n\t\t-{}", key.as_str().unwrap())
            ).collect())
        .collect();

let sinks= serde_yaml::from_str(&format!("\
bq_writing_pubsub:
    inputs: {:#?}
    type: gcp_pubsub
    project: {}

", transforms, config.project))?;


    // }

    // let vector_config: Mapping = Mapping::from_iter([
    //     ("sources".into(), beaver_config[&Value::String("sources".into())].clone()),
    //     ("transforms".into(), beaver_config[&Value::String("transforms".into())].clone()),
    // ]);
    //
    //
    // serde_yaml::to_writer(&vector_config_file, &vector_config).unwrap();



    Ok(())
}

fn validate_config_path(path: &Path) -> Result<()> {
    if !path.join("beaver_config.yaml").exists() {
        return Err(anyhow::anyhow!("config path does not exist or broken"));
    }
    Ok(())
}