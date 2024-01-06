use crate::lib::bq::BqTable;

pub struct Resources<'a> {
    pub biq_query: Option<BqTable<'a>>,
    pub gcs_bucket: Vec<String>,
    pub output_pubsub: Option<String>
}

impl Resources {
    pub fn empty() -> Self {
        Self {
            biq_query: None,
            gcs_bucket: Vec::new(),
            output_pubsub: None,
        }
    }
}
