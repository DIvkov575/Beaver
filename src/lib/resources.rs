use std::cell::{Cell, Ref, RefCell};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::pubsub::PubSub;
use crate::lib::service_accounts::SA;


#[derive(Deserialize, Serialize)]
pub struct Resources {
    #[serde(skip)]
    pub config_path: String,
    pub biq_query: RefCell<Option<BqTable>>,
    pub output_pubsub: RefCell<Option<PubSub>>,
    pub cron_job: RefCell<String>,
    pub compute_sa: RefCell<SA>,
    pub gcs_bucket: RefCell<Option<String>>,
    pub crj_instance: RefCell<String>,
}

impl Resources {
    pub fn empty() -> Self {
        Self {
            config_path: String::new(),
            biq_query: RefCell::new(None),
            output_pubsub: RefCell::new(None),
            cron_job: RefCell::new(String::new()),
            compute_sa: RefCell::new(SA::empty()),
            gcs_bucket: RefCell::new(None),
            crj_instance: RefCell::new(String::new()),
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

// impl Drop for Resources {
//     fn drop(&mut self) {
//         let mut file = std::fs::OpenOptions::new()
//             .write(true)
//             .create(true)
//             .open(Path::new(&self.config_path).join("artifacts/resources.yaml"))
//             .unwrap();
//         let mut ser = serde_yaml::Serializer::new(&mut file);
//
//         self.serialize(&mut ser).unwrap();
//         println!("Beaver: Resources graceful serialized");
//     }
//
// }
