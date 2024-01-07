use std::cell::{Cell, Ref, RefCell};
use crate::lib::bq::BqTable;
use crate::lib::pubsub::PubSub;

pub struct Resources {
    pub biq_query: Option<RefCell<BqTable>>,
    pub gcs_bucket: RefCell<String>,
    pub output_pubsub: Option<RefCell<PubSub>>,
    pub crj_instance: RefCell<String>,
}

impl Resources {
    pub fn empty() -> Self {
        Self {
            biq_query: None,
            output_pubsub: None,
            gcs_bucket: RefCell::new(String::new()),
            crj_instance: RefCell::new(String::new()),
        }
    }
}
