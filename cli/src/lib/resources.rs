use std::cell::{Cell, Ref, RefCell};
use crate::lib::bq::BqTable;
use crate::lib::pubsub::PubSub;

pub struct Resources {
    pub biq_query: Option<RefCell<BqTable>>,
    pub gcs_bucket: RefCell<Vec<String>>,
    pub output_pubsub: Option<RefCell<PubSub>>
}

impl Resources {
    pub fn empty() -> Self {
        Self {
            biq_query: None,
            gcs_bucket: RefCell::new(Vec::new()),
            output_pubsub: None,
        }
    }
}
