use anyhow::{anyhow, Result};
use log::{error, info};
use std::path::Path;

use crate::lib::config::Config;
use crate::lib::resources::{Resources, Tracker};
use crate::lib::utilities::{check_for_bq, check_for_gcloud, validate_config_path};
use crate::lib::{bq, cloud_build, crs, gcs, pubsub};

#[derive(Debug, PartialEq, Eq)]
pub enum DeleteStep {
    CrsService(String),
    PubsubBqSubscription(String),
    PubsubSubscription2(String),
    PubsubTopic(String),
    BqDataset(String),
    ArtifactImage(String),
    ArtifactRepo(String),
    GcsBucket(String),
}

/// Reverse of creation: CRS → pubsub subs → topic → bq → image → repo → bucket.
/// Bucket goes last because deploy may write debris into it that we don't want
/// to lose access to mid-tear-down. Skips empty fields.
pub fn plan(res: &Resources) -> Vec<DeleteStep> {
    let mut steps = Vec::new();

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
    if !res.biq_query.dataset_id.is_empty() {
        steps.push(DeleteStep::BqDataset(res.biq_query.dataset_id.clone()));
    }
    if !res.vector_artifact_url.is_empty() {
        steps.push(DeleteStep::ArtifactImage(res.vector_artifact_url.clone()));
    }
    if !res.artifact_registry_repo.is_empty() {
        steps.push(DeleteStep::ArtifactRepo(res.artifact_registry_repo.clone()));
    }
    if !res.bucket_name.is_empty() {
        steps.push(DeleteStep::GcsBucket(res.bucket_name.clone()));
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
                    DeleteStep::CrsService(_) => tracker.forget_crs_instance(),
                    DeleteStep::PubsubBqSubscription(_) => tracker.forget_pubsub_bq_subscription(),
                    DeleteStep::PubsubSubscription2(_) => tracker.forget_pubsub_subscription_2(),
                    DeleteStep::PubsubTopic(_) => tracker.forget_pubsub_topic(),
                    DeleteStep::BqDataset(_) => tracker.forget_bq(),
                    DeleteStep::ArtifactImage(_) => tracker.forget_image(),
                    DeleteStep::ArtifactRepo(_) => tracker.forget_artifact_repo(),
                    DeleteStep::GcsBucket(_) => tracker.forget_bucket(),
                };
                if let Err(e) = forget {
                    error!("forget after {:?} failed: {}", step, e);
                }
            }
        }
    }
}

fn dispatch_real(step: &DeleteStep, config: &Config) -> Result<()> {
    match step {
        DeleteStep::CrsService(name) => crs::delete_crs(name, config),
        DeleteStep::PubsubBqSubscription(id) => pubsub::delete_subscription(id, config),
        DeleteStep::PubsubSubscription2(id) => pubsub::delete_subscription(id, config),
        DeleteStep::PubsubTopic(id) => pubsub::delete_topic(id, config),
        DeleteStep::BqDataset(id) => bq::delete_dataset(id, &config.project),
        DeleteStep::ArtifactImage(url) => cloud_build::delete_image(url, config),
        DeleteStep::ArtifactRepo(name) => cloud_build::delete_repo(name, config),
        DeleteStep::GcsBucket(name) => gcs::delete_bucket(name),
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

    let config = Config::from_path(path);
    let steps = plan(&resources);

    if steps.is_empty() {
        info!("nothing to destroy; resources.yaml is empty");
        return Ok(());
    }

    let mut tracker = Tracker::new(&mut resources);
    execute(steps, &mut tracker, |step| dispatch_real(step, &config));

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
        res.crs_instance = "crs".into();
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
        {
            let mut tracker = Tracker::new(&mut res);
            execute(steps, &mut tracker, |s| dispatch_real(s, &config));
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
        res.crs_instance = "crs".into();
        res.output_pubsub.bq_subscription_id = "s1".into();
        res.output_pubsub.subscription_id_2 = "s2".into();
        res.output_pubsub.topic_id = "t".into();
        res.biq_query.dataset_id = "ds".into();
        res.biq_query.table_id = "table1".into();
        res.vector_artifact_url = "img".into();
        res.artifact_registry_repo = "repo".into();
        res.bucket_name = "bkt".into();

        let steps = plan(&res);
        assert_eq!(steps, vec![
            DeleteStep::CrsService("crs".into()),
            DeleteStep::PubsubBqSubscription("s1".into()),
            DeleteStep::PubsubSubscription2("s2".into()),
            DeleteStep::PubsubTopic("t".into()),
            DeleteStep::BqDataset("ds".into()),
            DeleteStep::ArtifactImage("img".into()),
            DeleteStep::ArtifactRepo("repo".into()),
            DeleteStep::GcsBucket("bkt".into()),
        ]);
    }
}
