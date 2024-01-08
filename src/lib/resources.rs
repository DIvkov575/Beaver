use std::cell::{Cell, Ref, RefCell};
use std::path::{Path, PathBuf};
use serde::Serialize;
use crate::lib::bq::BqTable;
use crate::lib::pubsub::PubSub;
use crate::lib::service_accounts::SA;


#[derive(Serialize)]
pub struct Resources {
    pub config_path: String,
    pub biq_query: Option<RefCell<BqTable>>,
    pub gcs_bucket: RefCell<String>,
    pub output_pubsub: Option<RefCell<PubSub>>,
    pub crj_instance: RefCell<String>,
    pub cron_job: RefCell<SA>,
    pub compute_sa: RefCell<SA>,
}

impl Resources {
    pub fn empty() -> Self {
        Self {
            biq_query: None,
            output_pubsub: None,
            config_path: String::new(),
            cron_job: RefCell::new(SA::empty()),
            compute_sa: RefCell::new(SA::empty()),
            gcs_bucket: RefCell::new(String::new()),
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
