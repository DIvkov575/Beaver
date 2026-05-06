use std::path::Path;
use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use crate::lib::bq::BqTable;
use crate::lib::config::Config;
use crate::lib::pubsub::PubSub;


#[derive(Deserialize, Serialize, Debug)]
pub struct Resources {
    pub config_path: String,
    pub biq_query: BqTable,
    pub output_pubsub: PubSub,
    pub bucket_name: String,
    pub artifact_registry_repo: String,
    pub crs_instance: String,
    pub vector_artifact_url: String,
    pub crs_schedule_job_id: String,
    pub dataflow_pipeline_name: String,
    pub scheduler_job_name: String,
}

impl Resources {
    pub fn empty(config: &Config, path: &Path) -> Self {
        Self {
            config_path: path.as_os_str().to_str().unwrap().to_string(),
            biq_query: BqTable::empty(&config),
            output_pubsub: PubSub::empty(),
            bucket_name: String::new(),
            artifact_registry_repo: String::new(),
            crs_instance: String::new(),
            vector_artifact_url: String::new(),
            crs_schedule_job_id: String::new(),
            dataflow_pipeline_name: String::new(),
            scheduler_job_name: String::new(),
        }
    }

    pub fn save(&self) {
        info!("serializing resources...");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(Path::new(&self.config_path).join("artifacts/resources.yaml"))
            .unwrap();
        let mut ser = serde_yaml::Serializer::new(&mut file);
        self.serialize(&mut ser).unwrap();
    }
}


/// Persisting wrapper around `Resources`. Each `record_*` method mutates the
/// underlying struct and writes it to disk before returning, so a crash or
/// failed retry mid-deploy still leaves a complete record of what got created.
pub struct Tracker<'a> {
    res: &'a mut Resources,
}

impl<'a> Tracker<'a> {
    pub fn new(res: &'a mut Resources) -> Self { Self { res } }

    pub fn resources(&self) -> &Resources { self.res }

    fn persist(&self) -> Result<()> {
        self.res.save();
        Ok(())
    }

    pub fn record_bq_dataset(&mut self, dataset_id: String) -> Result<()> {
        self.res.biq_query.dataset_id = dataset_id;
        self.persist()
    }

    pub fn record_bq_table(&mut self, table_id: String) -> Result<()> {
        self.res.biq_query.table_id = table_id;
        self.persist()
    }

    pub fn record_pubsub_topic(&mut self, id: String) -> Result<()> {
        self.res.output_pubsub.topic_id = id;
        self.persist()
    }

    pub fn record_pubsub_bq_subscription(&mut self, id: String) -> Result<()> {
        self.res.output_pubsub.bq_subscription_id = id;
        self.persist()
    }

    pub fn record_pubsub_subscription_2(&mut self, id: String) -> Result<()> {
        self.res.output_pubsub.subscription_id_2 = id;
        self.persist()
    }

    pub fn record_bucket(&mut self, name: String) -> Result<()> {
        self.res.bucket_name = name;
        self.persist()
    }

    pub fn record_artifact_repo(&mut self, name: String) -> Result<()> {
        self.res.artifact_registry_repo = name;
        self.persist()
    }

    pub fn record_image(&mut self, url: String) -> Result<()> {
        self.res.vector_artifact_url = url;
        self.persist()
    }

    pub fn record_crs_instance(&mut self, name: String) -> Result<()> {
        self.res.crs_instance = name;
        self.persist()
    }

    pub fn record_scheduler_job(&mut self, name: String) -> Result<()> {
        self.res.crs_schedule_job_id = name.clone();
        self.res.scheduler_job_name = name;
        self.persist()
    }

    pub fn forget_bq(&mut self) -> Result<()> {
        self.res.biq_query.dataset_id.clear();
        self.res.biq_query.table_id.clear();
        self.persist()
    }

    pub fn forget_pubsub_topic(&mut self) -> Result<()> {
        self.res.output_pubsub.topic_id.clear();
        self.persist()
    }

    pub fn forget_pubsub_bq_subscription(&mut self) -> Result<()> {
        self.res.output_pubsub.bq_subscription_id.clear();
        self.persist()
    }

    pub fn forget_pubsub_subscription_2(&mut self) -> Result<()> {
        self.res.output_pubsub.subscription_id_2.clear();
        self.persist()
    }

    pub fn forget_bucket(&mut self) -> Result<()> {
        self.res.bucket_name.clear();
        self.persist()
    }

    pub fn forget_artifact_repo(&mut self) -> Result<()> {
        self.res.artifact_registry_repo.clear();
        self.persist()
    }

    pub fn forget_image(&mut self) -> Result<()> {
        self.res.vector_artifact_url.clear();
        self.persist()
    }

    pub fn forget_crs_instance(&mut self) -> Result<()> {
        self.res.crs_instance.clear();
        self.persist()
    }

    pub fn forget_scheduler_job(&mut self) -> Result<()> {
        self.res.scheduler_job_name.clear();
        self.res.crs_schedule_job_id.clear();
        self.persist()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::config::Config;
    use std::fs;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Config) {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("artifacts")).unwrap();
        let config = Config::new("us-east1", "test-project", None);
        (dir, config)
    }

    fn read_back(path: &Path) -> Resources {
        let yaml = fs::read_to_string(path.join("artifacts/resources.yaml")).unwrap();
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[test]
    fn empty_resources_roundtrip() {
        let (dir, config) = setup();
        let res = Resources::empty(&config, dir.path());
        res.save();

        let back = read_back(dir.path());
        assert_eq!(back.biq_query.project_id, "test-project");
        assert!(back.bucket_name.is_empty());
        assert!(back.crs_instance.is_empty());
    }

    #[test]
    fn record_bq_persists_dataset_and_table() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_bq_dataset("ds_abc".into()).unwrap();
        t.record_bq_table("table1".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.biq_query.dataset_id, "ds_abc");
        assert_eq!(back.biq_query.table_id, "table1");
    }

    #[test]
    fn record_bq_dataset_persists_before_table_recorded() {
        // Simulates create_dataset succeeding then create_table failing — the
        // dataset must already be on disk so destroy can find it.
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_bq_dataset("ds_partial".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.biq_query.dataset_id, "ds_partial");
        assert!(back.biq_query.table_id.is_empty());
    }

    #[test]
    fn sequential_records_accumulate_on_disk() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_bucket("beaver_xyz".into()).unwrap();
        t.record_pubsub_topic("beaver_topic_1".into()).unwrap();
        t.record_crs_instance("beaver-vector-instance-abcd".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.bucket_name, "beaver_xyz");
        assert_eq!(back.output_pubsub.topic_id, "beaver_topic_1");
        assert_eq!(back.crs_instance, "beaver-vector-instance-abcd");
    }

    #[test]
    fn record_persists_after_each_call_not_only_at_end() {
        // Simulates the deploy crashing mid-pipeline: even if subsequent record_*
        // calls never happen, the resources written so far must be on disk.
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        {
            let mut t = Tracker::new(&mut res);
            t.record_pubsub_topic("topic_persisted".into()).unwrap();
            // Tracker dropped here without further calls (simulating panic).
        }

        let back = read_back(dir.path());
        assert_eq!(back.output_pubsub.topic_id, "topic_persisted");
    }

    #[test]
    fn record_scheduler_job_writes_both_fields() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_scheduler_job("beaver-cron-zzz".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.scheduler_job_name, "beaver-cron-zzz");
        assert_eq!(back.crs_schedule_job_id, "beaver-cron-zzz");
    }

    #[test]
    fn forget_clears_field_and_persists() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_bucket("beaver_xyz".into()).unwrap();
        assert_eq!(read_back(dir.path()).bucket_name, "beaver_xyz");

        t.forget_bucket().unwrap();
        assert!(read_back(dir.path()).bucket_name.is_empty());
    }

    #[test]
    fn forget_scheduler_job_clears_both_fields() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_scheduler_job("beaver-cron-zzz".into()).unwrap();
        t.forget_scheduler_job().unwrap();

        let back = read_back(dir.path());
        assert!(back.scheduler_job_name.is_empty());
        assert!(back.crs_schedule_job_id.is_empty());
    }

    #[test]
    fn forget_one_field_leaves_others_intact() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_bucket("bkt".into()).unwrap();
        t.record_pubsub_topic("topic".into()).unwrap();
        t.forget_bucket().unwrap();

        let back = read_back(dir.path());
        assert!(back.bucket_name.is_empty());
        assert_eq!(back.output_pubsub.topic_id, "topic");
    }

    #[test]
    fn save_truncates_existing_file() {
        // record_image writes a long URL; a subsequent record_image with a shorter
        // value must not leave trailing bytes from the longer write.
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);

        t.record_image("us-east1-docker.pkg.dev/p/repo/very-long-image-name-aaaaaaaaaaaa".into()).unwrap();
        t.record_image("short".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.vector_artifact_url, "short");
    }
}
