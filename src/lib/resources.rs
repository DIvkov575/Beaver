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
    pub dataflow_pipeline_name: String,
    #[serde(default)]
    pub notification_channels: Vec<String>,
    #[serde(default)]
    pub alert_policies: Vec<String>,
    #[serde(default)]
    pub vector_sa_email: String,
    #[serde(default)]
    pub vector_sa_managed: bool,
    #[serde(default)]
    pub dataflow_sa_email: String,
    #[serde(default)]
    pub dataflow_sa_managed: bool,
    #[serde(default)]
    pub dashboard_id: String,
    #[serde(default)]
    pub log_metric_name: String,
    #[serde(default)]
    pub biglake_connection_id: String,
    #[serde(default)]
    pub cold_bucket_prefix: String,
    #[serde(default)]
    pub cold_table_id: String,
    #[serde(default)]
    pub events_view_id: String,
    #[serde(default)]
    pub export_scheduled_query_id: String,
    #[serde(default)]
    pub export_sa_email: String,
    #[serde(default)]
    pub export_sa_managed: bool,
    // grafana-related resources
    #[serde(default)]
    pub grafana_service_url: String,
    #[serde(default)]
    pub grafana_sa_email: String,
    #[serde(default)]
    pub grafana_sa_managed: bool,
    #[serde(default)]
    pub grafana_image_url: String,
    // sigma_beam-related resources
    #[serde(default)]
    pub rules_gcs_prefix: String,
    #[serde(default)]
    pub alerts_topic_id: String,
    #[serde(default)]
    pub alerts_subscription_id: String,
    #[serde(default)]
    pub alerts_table_id: String,
    #[serde(default)]
    pub dlq_topic_id: String,
    #[serde(default)]
    pub dataflow_staging_bucket: String,
}

impl Resources {
    pub fn empty(config: &Config, path: &Path) -> Self {
        // Canonicalize so `save()` works regardless of the cwd commands are
        // later invoked from (e.g. repair). Falls back to the raw path if the
        // dir can't be resolved.
        let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        Self {
            config_path: abs.as_os_str().to_str().unwrap().to_string(),
            biq_query: BqTable::empty(config),
            output_pubsub: PubSub::empty(),
            bucket_name: String::new(),
            artifact_registry_repo: String::new(),
            crs_instance: String::new(),
            vector_artifact_url: String::new(),
            dataflow_pipeline_name: String::new(),
            notification_channels: Vec::new(),
            alert_policies: Vec::new(),
            vector_sa_email: String::new(),
            vector_sa_managed: false,
            dataflow_sa_email: String::new(),
            dataflow_sa_managed: false,
            dashboard_id: String::new(),
            log_metric_name: String::new(),
            biglake_connection_id: String::new(),
            cold_bucket_prefix: String::new(),
            cold_table_id: String::new(),
            events_view_id: String::new(),
            export_scheduled_query_id: String::new(),
            export_sa_email: String::new(),
            export_sa_managed: false,
            grafana_service_url: String::new(),
            grafana_sa_email: String::new(),
            grafana_sa_managed: false,
            grafana_image_url: String::new(),
            rules_gcs_prefix: String::new(),
            alerts_topic_id: String::new(),
            alerts_subscription_id: String::new(),
            alerts_table_id: String::new(),
            dlq_topic_id: String::new(),
            dataflow_staging_bucket: String::new(),
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


/// Persisting wrapper around `Resources`. Each `record_*`/`forget_*` writes
/// the struct to disk before returning, so a crash mid-deploy still leaves an
/// accurate record of what's on GCP.
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

    pub fn record_dataflow_pipeline(&mut self, name: String) -> Result<()> {
        self.res.dataflow_pipeline_name = name;
        self.persist()
    }

    pub fn record_notification_channel(&mut self, id: String) -> Result<()> {
        self.res.notification_channels.push(id);
        self.persist()
    }

    pub fn record_alert_policy(&mut self, id: String) -> Result<()> {
        self.res.alert_policies.push(id);
        self.persist()
    }

    pub fn record_vector_sa(&mut self, email: String, managed: bool) -> Result<()> {
        self.res.vector_sa_email = email;
        self.res.vector_sa_managed = managed;
        self.persist()
    }

    pub fn record_dataflow_sa(&mut self, email: String, managed: bool) -> Result<()> {
        self.res.dataflow_sa_email = email;
        self.res.dataflow_sa_managed = managed;
        self.persist()
    }

    pub fn record_dashboard(&mut self, id: String) -> Result<()> {
        self.res.dashboard_id = id;
        self.persist()
    }

    pub fn record_log_metric(&mut self, name: String) -> Result<()> {
        self.res.log_metric_name = name;
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

    pub fn forget_dataflow_pipeline(&mut self) -> Result<()> {
        self.res.dataflow_pipeline_name.clear();
        self.persist()
    }

    pub fn forget_notification_channel(&mut self, id: &str) -> Result<()> {
        self.res.notification_channels.retain(|x| x != id);
        self.persist()
    }

    pub fn forget_alert_policy(&mut self, id: &str) -> Result<()> {
        self.res.alert_policies.retain(|x| x != id);
        self.persist()
    }

    pub fn forget_vector_sa(&mut self) -> Result<()> {
        self.res.vector_sa_email.clear();
        self.res.vector_sa_managed = false;
        self.persist()
    }

    pub fn forget_dataflow_sa(&mut self) -> Result<()> {
        self.res.dataflow_sa_email.clear();
        self.res.dataflow_sa_managed = false;
        self.persist()
    }

    pub fn forget_dashboard(&mut self) -> Result<()> {
        self.res.dashboard_id.clear();
        self.persist()
    }

    pub fn forget_log_metric(&mut self) -> Result<()> {
        self.res.log_metric_name.clear();
        self.persist()
    }

    pub fn record_biglake_connection(&mut self, id: String) -> Result<()> {
        self.res.biglake_connection_id = id;
        self.persist()
    }
    pub fn forget_biglake_connection(&mut self) -> Result<()> {
        self.res.biglake_connection_id.clear();
        self.persist()
    }
    pub fn record_cold_bucket_prefix(&mut self, p: String) -> Result<()> {
        self.res.cold_bucket_prefix = p;
        self.persist()
    }
    pub fn forget_cold_bucket_prefix(&mut self) -> Result<()> {
        self.res.cold_bucket_prefix.clear();
        self.persist()
    }
    pub fn record_cold_table(&mut self, id: String) -> Result<()> {
        self.res.cold_table_id = id;
        self.persist()
    }
    pub fn forget_cold_table(&mut self) -> Result<()> {
        self.res.cold_table_id.clear();
        self.persist()
    }
    pub fn record_events_view(&mut self, id: String) -> Result<()> {
        self.res.events_view_id = id;
        self.persist()
    }
    pub fn forget_events_view(&mut self) -> Result<()> {
        self.res.events_view_id.clear();
        self.persist()
    }
    pub fn record_export_scheduled_query(&mut self, id: String) -> Result<()> {
        self.res.export_scheduled_query_id = id;
        self.persist()
    }
    pub fn forget_export_scheduled_query(&mut self) -> Result<()> {
        self.res.export_scheduled_query_id.clear();
        self.persist()
    }

    pub fn record_export_sa(&mut self, email: String, managed: bool) -> Result<()> {
        self.res.export_sa_email = email;
        self.res.export_sa_managed = managed;
        self.persist()
    }
    pub fn forget_export_sa(&mut self) -> Result<()> {
        self.res.export_sa_email.clear();
        self.res.export_sa_managed = false;
        self.persist()
    }

    pub fn record_grafana_service(&mut self, url: String) -> Result<()> {
        self.res.grafana_service_url = url;
        self.persist()
    }
    pub fn record_grafana_sa(&mut self, email: String, managed: bool) -> Result<()> {
        self.res.grafana_sa_email = email;
        self.res.grafana_sa_managed = managed;
        self.persist()
    }
    pub fn record_grafana_image(&mut self, url: String) -> Result<()> {
        self.res.grafana_image_url = url;
        self.persist()
    }
    pub fn forget_grafana_service(&mut self) -> Result<()> {
        self.res.grafana_service_url.clear();
        self.persist()
    }
    pub fn forget_grafana_sa(&mut self) -> Result<()> {
        self.res.grafana_sa_email.clear();
        self.res.grafana_sa_managed = false;
        self.persist()
    }
    pub fn forget_grafana_image(&mut self) -> Result<()> {
        self.res.grafana_image_url.clear();
        self.persist()
    }

    pub fn record_rules_prefix(&mut self, p: String) -> Result<()> {
        self.res.rules_gcs_prefix = p;
        self.persist()
    }
    pub fn forget_rules_prefix(&mut self) -> Result<()> {
        self.res.rules_gcs_prefix.clear();
        self.persist()
    }
    pub fn record_alerts_topic(&mut self, id: String) -> Result<()> {
        self.res.alerts_topic_id = id;
        self.persist()
    }
    pub fn forget_alerts_topic(&mut self) -> Result<()> {
        self.res.alerts_topic_id.clear();
        self.persist()
    }
    pub fn record_alerts_subscription(&mut self, id: String) -> Result<()> {
        self.res.alerts_subscription_id = id;
        self.persist()
    }
    pub fn forget_alerts_subscription(&mut self) -> Result<()> {
        self.res.alerts_subscription_id.clear();
        self.persist()
    }
    pub fn record_alerts_table(&mut self, id: String) -> Result<()> {
        self.res.alerts_table_id = id;
        self.persist()
    }
    pub fn forget_alerts_table(&mut self) -> Result<()> {
        self.res.alerts_table_id.clear();
        self.persist()
    }
    pub fn record_dlq_topic(&mut self, id: String) -> Result<()> {
        self.res.dlq_topic_id = id;
        self.persist()
    }
    pub fn forget_dlq_topic(&mut self) -> Result<()> {
        self.res.dlq_topic_id.clear();
        self.persist()
    }
    pub fn record_dataflow_staging_bucket(&mut self, name: String) -> Result<()> {
        self.res.dataflow_staging_bucket = name;
        self.persist()
    }
    pub fn forget_dataflow_staging_bucket(&mut self) -> Result<()> {
        self.res.dataflow_staging_bucket.clear();
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
    fn cold_tier_fields_roundtrip() {
        let (dir, config) = setup();
        let mut res = Resources::empty(&config, dir.path());
        let mut t = Tracker::new(&mut res);
        t.record_biglake_connection("conn1".into()).unwrap();
        t.record_cold_bucket_prefix("parquet".into()).unwrap();
        t.record_cold_table("events_cold".into()).unwrap();
        t.record_events_view("events_all".into()).unwrap();
        t.record_export_scheduled_query("sq1".into()).unwrap();

        let back = read_back(dir.path());
        assert_eq!(back.biglake_connection_id, "conn1");
        assert_eq!(back.cold_bucket_prefix, "parquet");
        assert_eq!(back.cold_table_id, "events_cold");
        assert_eq!(back.events_view_id, "events_all");
        assert_eq!(back.export_scheduled_query_id, "sq1");
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
