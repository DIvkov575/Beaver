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

pub fn deploy(path_arg: &str) -> Result<()> {
    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    let beaver_config: Mapping = serde_yaml::from_reader(File::open(path.join("beaver_config.yaml"))?)?;
    let region: String = beaver_config[&Value::String("region".into())].clone().as_str().unwrap().to_owned();
    let project: String = beaver_config[&Value::String("project".into())].clone().as_str().unwrap().to_owned();
    let config: Config = Config::new(&region, &project, None);

    bq::check_for_bq()?;
    let bq_table = BqTable::new(config.project, "beaver_data_warehouse", "table1");

    bq::create_dataset(bq_table.dataset_id, &config)?;
    bq::create_table(bq_table.dataset_id, bq_table.table_id, &config)?;
    let topic_id = pubsub::create_pubsub_topic(&config)?;

    pubsub::create_bq_subscription(&topic_id, &bq_table, &config)?;


    generate_vector_config(&path)?;



    Ok(())
}

// fn read_config(path: &Path) -> Result<Config> {
//    let beaver_config: Mapping = serde_yaml::from_reader(File::open(path.join("beaver_config.yaml"))?)?;
//
//     let region: String = beaver_config[&Value::String("region".into())].clone().as_str().unwrap().to_owned();
//     let project: String = beaver_config[&Value::String("project".into())].clone().as_str().unwrap().to_owned();
//     Ok(Config::new(&region, &project, None))
// }

fn generate_vector_config(path: &Path, resources: &Resources) -> Result<()> {
    let beaver_config_file = File::open(path.join("beaver_config.yaml"))?;
    let mut vector_config_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path.join("artifacts/vector.yaml"))
        .unwrap();

    let beaver_config: Mapping = serde_yaml::from_reader(&beaver_config_file)?;

    let sources =  beaver_config[&Value::String("sources".into())].clone();
    let transforms=  beaver_config[&Value::String("transforms".into())].clone();

    println!("{:?}", transforms);



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