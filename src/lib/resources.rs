use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::path::{Path, PathBuf};
use log::info;
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::config::Config;
use crate::lib::pubsub::PubSub;
use crate::lib::service_accounts::SA;


#[derive(Deserialize, Serialize, Debug)]
pub struct Resources {
    pub config_path: String,
    pub biq_query: BqTable, // output datalake
    pub output_pubsub: PubSub, // vector output pubsub?
    pub compute_sa: SA, // service_account for access delegation
    pub bucket_name: String, // staging area + compute template store
    pub crj_instance: String, //cloud run job - vector
}

impl Resources {
    pub fn empty(config: &Config, path: &Path) -> Self {
        Self {
            config_path: path.as_os_str().to_str().unwrap().to_string(),
            biq_query: BqTable::empty(&config),
            output_pubsub: PubSub::empty(),
            compute_sa: SA::empty(),
            bucket_name: String::new(),
            crj_instance: String::new(),
        }
    }

    pub fn save(&self) {
        info!("serializing resources...");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(Path::new(&self.config_path).join("artifacts/resources.yaml"))
            .unwrap();
        let mut ser = serde_yaml::Serializer::new(&mut file);

        self.serialize(&mut ser).unwrap();

        println!("Beaver: Resources graceful serialized");

    }

}
