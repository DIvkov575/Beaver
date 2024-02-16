use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::config::Config;
use crate::lib::pubsub::PubSub;
use crate::lib::service_accounts::SA;


#[derive(Deserialize, Serialize, Debug)]
pub struct Resources {
    #[serde(skip)]
    pub config_path: String,
    pub biq_query: RefCell<Option<BqTable>>,
    pub output_pubsub: RefCell<Option<PubSub>>,
    #[serde(skip)]
    pub compute_sa: RefCell<SA>,
    pub bucket_name: RefCell<Option<String>>,
    #[serde(skip)]
    pub crj_instance: RefCell<String>,
}

impl Resources {
    pub fn empty(config: &Config) -> Self {
        Self {
            config_path: String::new(),
            biq_query: RefCell::new(
                Some(BqTable::empty(&config))
            ),
            output_pubsub: RefCell::new(
                Some(PubSub::empty())
            ),
            compute_sa: RefCell::new(
                SA::empty()
            ),
            bucket_name: RefCell::new(
                Some(String::new())
            ),
            crj_instance: RefCell::new(
                String::new()
            ),
        }
    }

    pub fn save(&self) {
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
