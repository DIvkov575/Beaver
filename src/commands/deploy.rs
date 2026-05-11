use std::path::Path;
use std::process::exit;
use anyhow::{anyhow, Result};
use log::info;
use spinoff::{Color, Spinner, spinners};
use crate::lib::{bq::{
    self
}, config::Config, crs, dashboard, dataflow, detections_gen, gcs, pubsub, cloud_build, notifications, precheck, service_accounts};
use crate::lib::resources::{Resources, Tracker};
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, random_tag, validate_config_path};

pub fn deploy(path_arg: &str) -> Result<()> {
    info!("=======New Deployment======");
    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let config: Config = Config::from_path(&path);
    precheck::run(&config)?;
    let mut resources = Resources::empty(&config, &path);
    let mut tracker = Tracker::new(&mut resources);

    sigma::generate_detections(&path)?;
    detections_gen::generate_detections_file(&path)?;

    // Pub/Sub Service Agent needs bigquery.dataEditor for the BQ subscription
    // to actually flow. Idempotent — no-op if the binding already exists.
    service_accounts::grant_pubsub_to_bq(&config.project)?;

    bq::create(&mut tracker, &config)?;
    pubsub::create(&mut tracker, &config)?;
    gcs::create_bucket(&mut tracker, &config)?;

    // Vector runtime SA: minimum perms — read input sub, write output topic, write logs.
    let input_sub = Config::load_input_subscription(&path)?;
    let vector_sa_id = format!("beaver-vector-{}", random_tag(6));
    let vector_sa = service_accounts::create_sa(&vector_sa_id, "Beaver Vector", &config)?;
    tracker.record_vector_sa(vector_sa.email.clone(), true)?;
    service_accounts::grant_pubsub_subscription(&input_sub, &vector_sa.email,
        "roles/pubsub.subscriber", &config.project)?;
    service_accounts::grant_pubsub_topic(&tracker.resources().output_pubsub.topic_id,
        &vector_sa.email, "roles/pubsub.publisher", &config.project)?;
    service_accounts::grant_project(&config.project, &vector_sa.email,
        "roles/logging.logWriter")?;

    utilities::generate_vector_config(&path, tracker.resources(), &config)?;
    cloud_build::create_docker_image(&path, &mut tracker, &config)?;
    crs::create_vector(&mut tracker, &config)?;

    // Dataflow worker SA: minimum perms — read sub2, staging bucket, dataflow.worker, logs.
    let dataflow_sa_id = format!("beaver-dataflow-{}", random_tag(6));
    let dataflow_sa = service_accounts::create_sa(&dataflow_sa_id, "Beaver Dataflow", &config)?;
    tracker.record_dataflow_sa(dataflow_sa.email.clone(), true)?;
    service_accounts::grant_pubsub_subscription(
        &tracker.resources().output_pubsub.subscription_id_2,
        &dataflow_sa.email, "roles/pubsub.subscriber", &config.project)?;
    service_accounts::grant_bucket(&tracker.resources().bucket_name,
        &dataflow_sa.email, "roles/storage.objectAdmin")?;
    // Dataflow workers also need to write to GCP's managed staging bucket
    // (project-default for SDK distribution + runtime temp); the project-level
    // dataflow.worker role doesn't cover bucket-scoped writes there.
    let dataflow_staging = format!(
        "dataflow-staging-{}-{}",
        config.region,
        service_accounts::project_number(&config.project)?
    );
    service_accounts::grant_bucket(&dataflow_staging, &dataflow_sa.email, "roles/storage.objectAdmin")?;
    service_accounts::grant_project(&config.project, &dataflow_sa.email, "roles/dataflow.worker")?;
    service_accounts::grant_project(&config.project, &dataflow_sa.email, "roles/logging.logWriter")?;

    dataflow::create_template(&path, &mut tracker, &config)?;
    dataflow::create_pipeline(&mut tracker, &config)?;

    if let Some(notifications_cfg) = Config::load_notifications(&path)? {
        let name_to_id = notifications::create_channels(&mut tracker, &config, &notifications_cfg)?;
        notifications::create_alert_policies(&mut tracker, &config, &notifications_cfg, &name_to_id)?;
    }

    if let Some(dashboard_cfg) = Config::load_dashboard(&path)? {
        let metric_name = dashboard::create_log_metric(&mut tracker, &config)?;
        let id = dashboard::create_dashboard(&mut tracker, &config, &dashboard_cfg, &metric_name)?;
        let raw_id = id.rsplit('/').next().unwrap_or(&id);
        info!(
            "Dashboard: https://console.cloud.google.com/monitoring/dashboards/builder/{}?project={}",
            raw_id, config.project
        );
    }

    Ok(())
}
