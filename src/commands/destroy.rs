use anyhow::{anyhow, Result};
use log::{error, info};
use std::path::Path;

use crate::lib::config::Config;
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::{check_for_bq, check_for_gcloud, validate_config_path};
use crate::lib::{bq, cloud_build, cold_storage, crs, dashboard, dataflow, gcs, grafana, notifications, pubsub, service_accounts, sigma_beam_io};

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteStep {
    Dashboard(String),
    LogMetric(String),
    AlertPolicy(String),
    NotificationChannel(String),
    DataflowJob(String),
    CrsService(String),
    PubsubBqSubscription(String),
    PubsubSubscription2(String),
    PubsubTopic(String),
    BqDataset(String),
    ArtifactImage(String),
    ArtifactRepo(String),
    GcsBucket(String),
    VectorSa(String),
    DataflowSa(String),
    ExportScheduledQuery(String),
    EventsView(String),
    ColdTable(String),
    BigLakeConnection(String),
    ExportSa(String),
    AlertsSubscription(String),
    AlertsTopic(String),
    DlqTopic(String),
    AlertsTable(String),
    RulesPrefix(String),  // bucket name; rules live at gs://<bucket>/rules/
    DataflowStagingBucket(String),
    GrafanaService(String),
    GrafanaImage(String),
    GrafanaSa(String),
}

/// Reverse of creation: alert policies → notification channels → dataflow →
/// CRS → pubsub subs → topic → bq → image → repo → bucket. Policies before
/// channels because policies reference channel IDs. Dataflow before pubsub
/// so it stops reading from the output topic before we delete the topic.
/// Bucket last because deploy writes debris (template, staging) into it.
pub fn plan(res: &Resources) -> Vec<DeleteStep> {
    let mut steps = Vec::new();

    // Dashboard references the log metric, so the dashboard must go first.
    if !res.dashboard_id.is_empty() {
        steps.push(DeleteStep::Dashboard(res.dashboard_id.clone()));
    }
    if !res.log_metric_name.is_empty() {
        steps.push(DeleteStep::LogMetric(res.log_metric_name.clone()));
    }
    for policy in &res.alert_policies {
        steps.push(DeleteStep::AlertPolicy(policy.clone()));
    }
    for channel in &res.notification_channels {
        steps.push(DeleteStep::NotificationChannel(channel.clone()));
    }
    if !res.dataflow_pipeline_name.is_empty() {
        steps.push(DeleteStep::DataflowJob(res.dataflow_pipeline_name.clone()));
    }
    // sigma_beam alerts/DLQ: the alerts subscription depends on its topic
    // and the alerts BQ table — delete the subscription first. Topic +
    // table deletes are independent of each other after that.
    if !res.alerts_subscription_id.is_empty() {
        steps.push(DeleteStep::AlertsSubscription(res.alerts_subscription_id.clone()));
    }
    if !res.alerts_topic_id.is_empty() {
        steps.push(DeleteStep::AlertsTopic(res.alerts_topic_id.clone()));
    }
    if !res.dlq_topic_id.is_empty() {
        steps.push(DeleteStep::DlqTopic(res.dlq_topic_id.clone()));
    }
    if !res.crs_instance.is_empty() {
        steps.push(DeleteStep::CrsService(res.crs_instance.clone()));
    }
    if !res.output_pubsub.bq_subscription_id.is_empty() {
        steps.push(DeleteStep::PubsubBqSubscription(res.output_pubsub.bq_subscription_id.clone()));
    }
    if !res.output_pubsub.subscription_id_2.is_empty() {
        steps.push(DeleteStep::PubsubSubscription2(res.output_pubsub.subscription_id_2.clone()));
    }
    if !res.output_pubsub.topic_id.is_empty() {
        steps.push(DeleteStep::PubsubTopic(res.output_pubsub.topic_id.clone()));
    }
    // Cold tier: scheduled query, view, external table, BigLake connection.
    // Must precede BqDataset removal so the view/cold-table delete first
    // (and the connection is not in the dataset).
    if !res.export_scheduled_query_id.is_empty() {
        steps.push(DeleteStep::ExportScheduledQuery(res.export_scheduled_query_id.clone()));
    }
    // Delete SA after the scheduled query that uses it, before BqDataset
    // teardown (no ordering dependency with the dataset, but keep together).
    if res.export_sa_managed && !res.export_sa_email.is_empty() {
        steps.push(DeleteStep::ExportSa(res.export_sa_email.clone()));
    }
    if !res.events_view_id.is_empty() {
        steps.push(DeleteStep::EventsView(res.events_view_id.clone()));
    }
    if !res.cold_table_id.is_empty() {
        steps.push(DeleteStep::ColdTable(res.cold_table_id.clone()));
    }
    if !res.biglake_connection_id.is_empty() {
        steps.push(DeleteStep::BigLakeConnection(res.biglake_connection_id.clone()));
    }
    // alerts BQ table must go before the dataset that holds it.
    if !res.alerts_table_id.is_empty() {
        steps.push(DeleteStep::AlertsTable(res.alerts_table_id.clone()));
    }
    if !res.biq_query.dataset_id.is_empty() {
        steps.push(DeleteStep::BqDataset(res.biq_query.dataset_id.clone()));
    }
    if !res.vector_artifact_url.is_empty() {
        steps.push(DeleteStep::ArtifactImage(res.vector_artifact_url.clone()));
    }
    if !res.artifact_registry_repo.is_empty() {
        steps.push(DeleteStep::ArtifactRepo(res.artifact_registry_repo.clone()));
    }
    // Clear the rules prefix before nuking the bucket so any per-object
    // lifecycle hooks don't fight us on the recursive bucket delete.
    if !res.rules_gcs_prefix.is_empty() && !res.bucket_name.is_empty() {
        steps.push(DeleteStep::RulesPrefix(res.bucket_name.clone()));
    }
    if !res.bucket_name.is_empty() {
        steps.push(DeleteStep::GcsBucket(res.bucket_name.clone()));
    }
    if !res.dataflow_staging_bucket.is_empty() {
        steps.push(DeleteStep::DataflowStagingBucket(res.dataflow_staging_bucket.clone()));
    }
    // Grafana: service first (consumer), then image, then SA.
    if !res.grafana_service_url.is_empty() {
        steps.push(DeleteStep::GrafanaService(res.grafana_service_url.clone()));
    }
    if !res.grafana_image_url.is_empty() {
        steps.push(DeleteStep::GrafanaImage(res.grafana_image_url.clone()));
    }
    if res.grafana_sa_managed && !res.grafana_sa_email.is_empty() {
        steps.push(DeleteStep::GrafanaSa(res.grafana_sa_email.clone()));
    }
    // Service accounts last: only after every consumer (CRS, Dataflow) and
    // every IAM-bound resource (sub, topic, bucket) is gone, so resource-scoped
    // bindings clean up automatically when the resource disappears. Only delete
    // when beaver-managed; user-supplied SAs are someone else's lifecycle.
    if res.vector_sa_managed && !res.vector_sa_email.is_empty() {
        steps.push(DeleteStep::VectorSa(res.vector_sa_email.clone()));
    }
    if res.dataflow_sa_managed && !res.dataflow_sa_email.is_empty() {
        steps.push(DeleteStep::DataflowSa(res.dataflow_sa_email.clone()));
    }

    steps
}

/// Best-effort: failures are logged and the loop continues. Successful deletes
/// trigger the matching `forget_*` so a partial destroy leaves an accurate
/// `resources.yaml`. `delete` is injected so tests can swap in a fake.
fn execute<F>(steps: Vec<DeleteStep>, tracker: &mut Tracker, mut delete: F)
where
    F: FnMut(&DeleteStep) -> Result<()>,
{
    for step in steps {
        match delete(&step) {
            Err(e) => error!("destroy step {:?} failed: {}", step, e),
            Ok(()) => {
                let forget = match &step {
                    DeleteStep::Dashboard(_) => tracker.forget_dashboard(),
                    DeleteStep::LogMetric(_) => tracker.forget_log_metric(),
                    DeleteStep::AlertPolicy(id) => tracker.forget_alert_policy(id),
                    DeleteStep::NotificationChannel(id) => tracker.forget_notification_channel(id),
                    DeleteStep::DataflowJob(_) => tracker.forget_dataflow_pipeline(),
                    DeleteStep::CrsService(_) => tracker.forget_crs_instance(),
                    DeleteStep::PubsubBqSubscription(_) => tracker.forget_pubsub_bq_subscription(),
                    DeleteStep::PubsubSubscription2(_) => tracker.forget_pubsub_subscription_2(),
                    DeleteStep::PubsubTopic(_) => tracker.forget_pubsub_topic(),
                    DeleteStep::BqDataset(_) => tracker.forget_bq(),
                    DeleteStep::ArtifactImage(_) => tracker.forget_image(),
                    DeleteStep::ArtifactRepo(_) => tracker.forget_artifact_repo(),
                    DeleteStep::GcsBucket(_) => tracker.forget_bucket(),
                    DeleteStep::DataflowStagingBucket(_) => tracker.forget_dataflow_staging_bucket(),
                    DeleteStep::VectorSa(_) => tracker.forget_vector_sa(),
                    DeleteStep::DataflowSa(_) => tracker.forget_dataflow_sa(),
                    DeleteStep::ExportScheduledQuery(_) => tracker.forget_export_scheduled_query(),
                    DeleteStep::ExportSa(_) => tracker.forget_export_sa(),
                    DeleteStep::EventsView(_) => tracker.forget_events_view(),
                    DeleteStep::ColdTable(_) => tracker.forget_cold_table(),
                    DeleteStep::BigLakeConnection(_) => tracker.forget_biglake_connection(),
                    DeleteStep::AlertsSubscription(_) => tracker.forget_alerts_subscription(),
                    DeleteStep::AlertsTopic(_) => tracker.forget_alerts_topic(),
                    DeleteStep::DlqTopic(_) => tracker.forget_dlq_topic(),
                    DeleteStep::AlertsTable(_) => tracker.forget_alerts_table(),
                    DeleteStep::RulesPrefix(_) => tracker.forget_rules_prefix(),
                    DeleteStep::GrafanaService(_) => tracker.forget_grafana_service(),
                    DeleteStep::GrafanaImage(_) => tracker.forget_grafana_image(),
                    DeleteStep::GrafanaSa(_) => tracker.forget_grafana_sa(),
                };
                if let Err(e) = forget {
                    error!("forget after {:?} failed: {}", step, e);
                }
            }
        }
    }
}

fn dispatch_real(step: &DeleteStep, config: &Config, dataset_id: &str) -> Result<()> {
    match step {
        DeleteStep::Dashboard(id) => dashboard::delete_dashboard(id, &config.project),
        DeleteStep::LogMetric(name) => dashboard::delete_log_metric(name, &config.project),
        DeleteStep::AlertPolicy(id) => notifications::delete_policy(id),
        DeleteStep::NotificationChannel(id) => notifications::delete_channel(id),
        DeleteStep::DataflowJob(name) => dataflow::delete_job(name, config),
        DeleteStep::CrsService(name) => crs::delete_crs(name, config),
        DeleteStep::PubsubBqSubscription(id) => pubsub::delete_subscription(id, config),
        DeleteStep::PubsubSubscription2(id) => pubsub::delete_subscription(id, config),
        DeleteStep::PubsubTopic(id) => pubsub::delete_topic(id, config),
        DeleteStep::BqDataset(id) => bq::delete_dataset(id, &config.project),
        DeleteStep::ArtifactImage(url) => cloud_build::delete_image(url, config),
        DeleteStep::ArtifactRepo(name) => cloud_build::delete_repo(name, config),
        DeleteStep::GcsBucket(name) => gcs::delete_bucket(name),
        DeleteStep::DataflowStagingBucket(name) => gcs::delete_bucket(name),
        DeleteStep::VectorSa(email) => service_accounts::delete_sa(email, &config.project),
        DeleteStep::DataflowSa(email) => service_accounts::delete_sa(email, &config.project),
        DeleteStep::ExportScheduledQuery(id) =>
            cold_storage::destroy_scheduled_query(id, &config.project),
        DeleteStep::ExportSa(email) =>
            service_accounts::delete_sa(email, &config.project),
        DeleteStep::EventsView(view) =>
            cold_storage::destroy_view(dataset_id, view, &config.project),
        DeleteStep::ColdTable(t) =>
            cold_storage::destroy_cold_table(dataset_id, t, &config.project),
        DeleteStep::BigLakeConnection(id) =>
            cold_storage::destroy_connection(id, &config.project, &config.region),
        DeleteStep::AlertsSubscription(id) =>
            sigma_beam_io::destroy_alerts_subscription(id, config),
        DeleteStep::AlertsTopic(id) =>
            sigma_beam_io::destroy_alerts_topic(id, config),
        DeleteStep::DlqTopic(id) =>
            sigma_beam_io::destroy_dlq_topic(id, config),
        DeleteStep::AlertsTable(t) =>
            sigma_beam_io::destroy_alerts_table(&config.project, dataset_id, t),
        DeleteStep::RulesPrefix(bucket) =>
            sigma_beam_io::destroy_rules_prefix(bucket),
        DeleteStep::GrafanaService(name) =>
            grafana::deploy::delete_grafana_service(name, config),
        DeleteStep::GrafanaImage(url) =>
            grafana::deploy::delete_grafana_image(url, config),
        DeleteStep::GrafanaSa(email) =>
            service_accounts::delete_sa(email, &config.project),
    }
}

pub fn destroy(path_arg: &str) -> Result<()> {
    info!("=======Destroying Resources======");
    let path = Path::new(path_arg);
    validate_config_path(path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let resources_path = path.join("artifacts/resources.yaml");
    if !resources_path.exists() {
        return Err(anyhow!("Resources file not found at {}", resources_path.display()));
    }
    let yaml = std::fs::read_to_string(&resources_path)?;
    let mut resources: Resources = serde_yaml::from_str(&yaml)?;
    // Older resources.yaml stored config_path relative to deploy cwd; rewrite
    // to absolute so Tracker.save() works no matter where destroy is run from.
    if let Ok(abs) = path.canonicalize() {
        resources.config_path = abs.as_os_str().to_str().unwrap().to_string();
    }

    let config = Config::from_path(path);
    let steps = plan(&resources);

    if steps.is_empty() {
        info!("nothing to destroy; resources.yaml is empty");
        return Ok(());
    }

    let dataset_id = resources.biq_query.dataset_id.clone();
    let mut tracker = Tracker::new(&mut resources);
    execute(steps, &mut tracker, |step| dispatch_real(step, &config, &dataset_id));

    if plan(tracker.resources()).is_empty() {
        std::fs::remove_file(&resources_path).ok();
        info!("destroy complete; resources.yaml removed");
    } else {
        info!("destroy partial; resources.yaml retains failed entries");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::resources::Resources;
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn config() -> Config { Config::new("us-east1", "p", None) }
    fn empty_resources() -> Resources { Resources::empty(&config(), &PathBuf::from("/tmp/x")) }

    /// Resources rooted at a tempdir so `Tracker::persist` writes somewhere harmless.
    fn resources_in_tempdir() -> (TempDir, Resources) {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("artifacts")).unwrap();
        let res = Resources::empty(&config(), dir.path());
        (dir, res)
    }

    fn fully_populated() -> Resources {
        let mut res = empty_resources();
        res.dataflow_pipeline_name = "df".into();
        res.crs_instance = "crs".into();
        res.vector_sa_email = "v@x.iam".into();
        res.vector_sa_managed = true;
        res.dataflow_sa_email = "d@x.iam".into();
        res.dataflow_sa_managed = true;
        res.output_pubsub.bq_subscription_id = "s1".into();
        res.output_pubsub.subscription_id_2 = "s2".into();
        res.output_pubsub.topic_id = "t".into();
        res.biq_query.dataset_id = "ds".into();
        res.biq_query.table_id = "table1".into();
        res.vector_artifact_url = "img".into();
        res.artifact_registry_repo = "repo".into();
        res.bucket_name = "bkt".into();
        res
    }

    #[test]
    fn plan_empty_resources_is_empty() {
        let res = empty_resources();
        assert!(plan(&res).is_empty());
    }

    #[test]
    fn plan_skips_fields_never_recorded() {
        // Crashed right after BQ creation: only bq fields populated.
        let mut res = empty_resources();
        res.biq_query.dataset_id = "ds_x".into();
        res.biq_query.table_id = "table1".into();

        let steps = plan(&res);
        assert_eq!(steps, vec![DeleteStep::BqDataset("ds_x".into())]);
    }

    #[test]
    fn plan_orders_subs_before_topic() {
        let mut res = empty_resources();
        res.output_pubsub.topic_id = "t".into();
        res.output_pubsub.bq_subscription_id = "s1".into();
        res.output_pubsub.subscription_id_2 = "s2".into();

        let steps = plan(&res);
        let topic_idx = steps.iter().position(|s| matches!(s, DeleteStep::PubsubTopic(_))).unwrap();
        let s1_idx = steps.iter().position(|s| matches!(s, DeleteStep::PubsubBqSubscription(_))).unwrap();
        let s2_idx = steps.iter().position(|s| matches!(s, DeleteStep::PubsubSubscription2(_))).unwrap();
        assert!(s1_idx < topic_idx, "bq sub must precede topic");
        assert!(s2_idx < topic_idx, "sub2 must precede topic");
    }

    #[test]
    fn plan_orders_image_before_repo() {
        let mut res = empty_resources();
        res.vector_artifact_url = "us-east1-docker.pkg.dev/p/r/img".into();
        res.artifact_registry_repo = "r".into();

        let steps = plan(&res);
        let img_idx = steps.iter().position(|s| matches!(s, DeleteStep::ArtifactImage(_))).unwrap();
        let repo_idx = steps.iter().position(|s| matches!(s, DeleteStep::ArtifactRepo(_))).unwrap();
        assert!(img_idx < repo_idx, "image must precede repo");
    }

    #[test]
    fn execute_calls_dispatcher_in_planned_order() {
        let (_dir, mut res) = resources_in_tempdir();
        res.output_pubsub.topic_id = "t".into();
        res.output_pubsub.bq_subscription_id = "s1".into();
        res.bucket_name = "bkt".into();
        let steps = plan(&res);

        let calls: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let mut tracker = Tracker::new(&mut res);
        execute(steps, &mut tracker, |step| {
            calls.borrow_mut().push(format!("{:?}", step));
            Ok(())
        });

        let calls = calls.into_inner();
        assert_eq!(calls.len(), 3);
        assert!(calls[0].contains("PubsubBqSubscription"));
        assert!(calls[1].contains("PubsubTopic"));
        assert!(calls[2].contains("GcsBucket"));
    }

    #[test]
    fn execute_forgets_field_after_successful_delete() {
        let (_dir, mut res) = resources_in_tempdir();
        res.bucket_name = "bkt".into();
        let steps = plan(&res);

        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |_| Ok(()));
        }
        assert!(res.bucket_name.is_empty());
    }

    #[test]
    fn execute_does_not_forget_when_delete_fails() {
        let (_dir, mut res) = resources_in_tempdir();
        res.bucket_name = "bkt".into();
        let steps = plan(&res);

        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |_| Err(anyhow!("boom")));
        }
        // Field stays so a re-run of destroy can retry it.
        assert_eq!(res.bucket_name, "bkt");
    }

    #[test]
    fn execute_continues_after_a_step_fails() {
        let (_dir, mut res) = resources_in_tempdir();
        res.bucket_name = "bkt".into();
        res.output_pubsub.topic_id = "t".into();
        let steps = plan(&res);

        let calls: RefCell<usize> = RefCell::new(0);
        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |step| {
                *calls.borrow_mut() += 1;
                if matches!(step, DeleteStep::PubsubTopic(_)) {
                    Err(anyhow!("simulated failure on topic"))
                } else {
                    Ok(())
                }
            });
        }

        assert_eq!(*calls.borrow(), 2, "both steps must be attempted even when one fails");
        assert_eq!(res.output_pubsub.topic_id, "t", "failed delete should not clear field");
        assert!(res.bucket_name.is_empty(), "successful delete should clear field");
    }

    #[test]
    fn execute_full_pipeline_clears_everything_when_all_succeed() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("artifacts")).unwrap();
        let mut res = fully_populated();
        res.config_path = dir.path().to_str().unwrap().to_string();

        let steps = plan(&res);
        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |_| Ok(()));
        }

        assert!(plan(&res).is_empty(), "plan must be empty after successful destroy");
    }

    /// Full create→destroy E2E for the bq + pubsub + gcs subset (CRS and image
    /// have their own pair tests). Verified by exact-name describe so the
    /// assertions are strongly consistent.
    #[test]
    #[ignore]
    fn e2e_simple_resources_round_trip() {
        use crate::lib::{bq, gcs, pubsub};
        use crate::lib::test_helpers::{
            bq_dataset_exists, bucket_exists, pubsub_subscription_exists,
            pubsub_topic_exists, test_config,
        };

        let config = test_config();
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("artifacts")).unwrap();
        let mut res = Resources::empty(&config, dir.path());

        // Run the create half.
        {
            let mut tracker = Tracker::new(&mut res);
            bq::create(&mut tracker, &config).expect("bq create");
            pubsub::create(&mut tracker, &config).expect("pubsub create");
            gcs::create_bucket(&mut tracker, &config).expect("gcs create");
        }

        // Snapshot before destroy clears the struct.
        let dataset = res.biq_query.dataset_id.clone();
        let topic = res.output_pubsub.topic_id.clone();
        let sub_bq = res.output_pubsub.bq_subscription_id.clone();
        let sub_2 = res.output_pubsub.subscription_id_2.clone();
        let bucket = res.bucket_name.clone();

        assert!(bq_dataset_exists(&dataset, &config.project), "dataset {} missing", dataset);
        assert!(pubsub_topic_exists(&topic, &config.project), "topic {} missing", topic);
        assert!(pubsub_subscription_exists(&sub_bq, &config.project), "bq sub {} missing", sub_bq);
        assert!(pubsub_subscription_exists(&sub_2, &config.project), "sub2 {} missing", sub_2);
        assert!(bucket_exists(&bucket), "bucket {} missing", bucket);

        let steps = plan(&res);
        let dataset_id = res.biq_query.dataset_id.clone();
        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |s| dispatch_real(s, &config, &dataset_id));
        }


        assert!(!bq_dataset_exists(&dataset, &config.project), "dataset {} leaked", dataset);
        assert!(!pubsub_topic_exists(&topic, &config.project), "topic {} leaked", topic);
        assert!(!pubsub_subscription_exists(&sub_bq, &config.project), "bq sub {} leaked", sub_bq);
        assert!(!pubsub_subscription_exists(&sub_2, &config.project), "sub2 {} leaked", sub_2);
        assert!(!bucket_exists(&bucket), "bucket {} leaked", bucket);

        assert!(plan(&res).is_empty(), "plan must be empty after destroy");
    }

    #[test]
    fn execute_empty_steps_is_noop() {
        let (_dir, mut res) = resources_in_tempdir();
        let steps = plan(&res);
        assert!(steps.is_empty());

        let mut tracker = Tracker::new(&mut res);
        let calls: RefCell<usize> = RefCell::new(0);
        execute(steps, &mut tracker, |_| {
            *calls.borrow_mut() += 1;
            Ok(())
        });
        assert_eq!(*calls.borrow(), 0);
    }

    #[test]
    fn plan_full_state_in_reverse_creation_order() {
        let mut res = empty_resources();
        res.dataflow_pipeline_name = "df".into();
        res.crs_instance = "crs".into();
        res.output_pubsub.bq_subscription_id = "s1".into();
        res.output_pubsub.subscription_id_2 = "s2".into();
        res.output_pubsub.topic_id = "t".into();
        res.biq_query.dataset_id = "ds".into();
        res.biq_query.table_id = "table1".into();
        res.vector_artifact_url = "img".into();
        res.artifact_registry_repo = "repo".into();
        res.bucket_name = "bkt".into();
        res.vector_sa_email = "v@x.iam".into();
        res.vector_sa_managed = true;
        res.dataflow_sa_email = "d@x.iam".into();
        res.dataflow_sa_managed = true;

        let steps = plan(&res);
        assert_eq!(steps, vec![
            DeleteStep::DataflowJob("df".into()),
            DeleteStep::CrsService("crs".into()),
            DeleteStep::PubsubBqSubscription("s1".into()),
            DeleteStep::PubsubSubscription2("s2".into()),
            DeleteStep::PubsubTopic("t".into()),
            DeleteStep::BqDataset("ds".into()),
            DeleteStep::ArtifactImage("img".into()),
            DeleteStep::ArtifactRepo("repo".into()),
            DeleteStep::GcsBucket("bkt".into()),
            DeleteStep::VectorSa("v@x.iam".into()),
            DeleteStep::DataflowSa("d@x.iam".into()),
        ]);
    }

    #[test]
    fn plan_skips_unmanaged_service_accounts() {
        let mut res = empty_resources();
        res.vector_sa_email = "supplied@x.iam".into();
        res.vector_sa_managed = false;
        res.dataflow_sa_email = "also-supplied@x.iam".into();
        res.dataflow_sa_managed = false;
        // User-supplied SAs are not in beaver's lifecycle; planner ignores them.
        assert!(plan(&res).is_empty());
    }

    #[test]
    fn plan_includes_dataflow_staging_bucket() {
        let mut res = empty_resources();
        res.dataflow_staging_bucket = "dataflow-staging-us-east1-123456".into();
        res.bucket_name = "beaver_abc".into();

        let steps = plan(&res);
        let staging_idx = steps.iter().position(|s| matches!(s, DeleteStep::DataflowStagingBucket(_))).unwrap();
        let main_idx = steps.iter().position(|s| matches!(s, DeleteStep::GcsBucket(_))).unwrap();
        assert!(staging_idx > main_idx, "staging bucket should be deleted after main bucket");
    }
}
