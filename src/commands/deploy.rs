use std::path::Path;
use anyhow::Result;
use log::info;
use spinoff::{spinners, Color, Spinner};

use crate::lib::{bq::{
    self
}, config::Config, crs, dashboard, dataflow, detections_gen, gcs, pubsub, cloud_build, notifications, precheck, service_accounts};
use crate::lib::resources::{Resources, Tracker};
use crate::lib::sigma;
use crate::lib::utilities::{self, check_for_bq, check_for_gcloud, random_tag, validate_config_path};

/// Runs a step under a spinner. On `Ok`, the spinner reports success and
/// emits a tick line. On `Err`, the spinner reports failure and the error
/// propagates.
fn step<T, F: FnOnce() -> Result<T>>(label: &str, f: F) -> Result<T> {
    let mut sp = Spinner::new(spinners::Dots, label.to_string(), Color::Blue);
    match f() {
        Ok(v) => {
            sp.success(label);
            Ok(v)
        }
        Err(e) => {
            sp.fail(label);
            Err(e)
        }
    }
}

pub fn deploy(path_arg: &str) -> Result<()> {
    info!("=======New Deployment======");
    println!("\nBeaver deploy");
    println!("=============\n");

    let path = Path::new(path_arg);
    validate_config_path(&path)?;
    check_for_bq()?;
    check_for_gcloud()?;

    let config: Config = Config::from_path(&path);
    step("environment precheck", || precheck::run(&config))?;

    let mut resources = Resources::empty(&config, &path);
    let mut tracker = Tracker::new(&mut resources);

    step("compile sigma rules", || {
        sigma::generate_detections(&path)?;
        detections_gen::generate_detections_file(&path)
    })?;

    step("grant Pub/Sub→BQ delivery", || {
        service_accounts::grant_pubsub_to_bq(&config.project)
    })?;

    step("BigQuery dataset + table", || bq::create(&mut tracker, &config))?;
    step("Pub/Sub topic + subscriptions", || pubsub::create(&mut tracker, &config))?;
    step("GCS bucket", || gcs::create_bucket(&mut tracker, &config))?;

    let input_sub = Config::load_input_subscription(&path)?;
    let vector_sa = step("Vector service account + IAM", || {
        let vector_sa_id = format!("beaver-vector-{}", random_tag(6));
        let sa = service_accounts::create_sa(&vector_sa_id, "Beaver Vector", &config)?;
        tracker.record_vector_sa(sa.email.clone(), true)?;
        service_accounts::grant_pubsub_subscription(&input_sub, &sa.email,
            "roles/pubsub.subscriber", &config.project)?;
        service_accounts::grant_pubsub_topic(&tracker.resources().output_pubsub.topic_id,
            &sa.email, "roles/pubsub.publisher", &config.project)?;
        service_accounts::grant_project(&config.project, &sa.email, "roles/logging.logWriter")?;
        Ok::<_, anyhow::Error>(sa)
    })?;
    let _ = vector_sa;

    step("write vector.yaml", || utilities::generate_vector_config(&path, tracker.resources(), &config))?;
    step("build Vector docker image (Cloud Build)", || cloud_build::create_docker_image(&path, &mut tracker, &config))?;
    step("deploy Vector Cloud Run service", || crs::create_vector(&mut tracker, &config))?;

    step("Dataflow service account + IAM", || {
        let dataflow_sa_id = format!("beaver-dataflow-{}", random_tag(6));
        let sa = service_accounts::create_sa(&dataflow_sa_id, "Beaver Dataflow", &config)?;
        tracker.record_dataflow_sa(sa.email.clone(), true)?;
        service_accounts::grant_pubsub_subscription(
            &tracker.resources().output_pubsub.subscription_id_2,
            &sa.email, "roles/pubsub.subscriber", &config.project)?;
        service_accounts::grant_bucket(&tracker.resources().bucket_name,
            &sa.email, "roles/storage.objectAdmin")?;
        let dataflow_staging = format!(
            "dataflow-staging-{}-{}",
            config.region,
            service_accounts::project_number(&config.project)?
        );
        service_accounts::grant_bucket(&dataflow_staging, &sa.email, "roles/storage.objectAdmin")?;
        service_accounts::grant_project(&config.project, &sa.email, "roles/dataflow.worker")?;
        service_accounts::grant_project(&config.project, &sa.email, "roles/logging.logWriter")?;
        Ok::<_, anyhow::Error>(())
    })?;

    step("upload Dataflow template", || dataflow::create_template(&path, &mut tracker, &config))?;
    step("launch Dataflow streaming job", || dataflow::create_pipeline(&mut tracker, &config))?;
    let pipeline_name = tracker.resources().dataflow_pipeline_name.clone();
    step("wait for Dataflow workers to come up (up to 5 min)",
         || dataflow::wait_for_running(&pipeline_name, &config))?;

    let notif_count = if let Some(notifications_cfg) = Config::load_notifications(&path)? {
        step("notification channels + alert policies", || {
            let name_to_id = notifications::create_channels(&mut tracker, &config, &notifications_cfg)?;
            notifications::create_alert_policies(&mut tracker, &config, &notifications_cfg, &name_to_id)?;
            Ok::<_, anyhow::Error>(notifications_cfg.channels.len())
        })?
    } else { 0 };

    let mut dashboard_url: Option<String> = None;
    if let Some(dashboard_cfg) = Config::load_dashboard(&path)? {
        let url = step("SOC dashboard + log-based metric", || {
            let metric_name = dashboard::create_log_metric(&mut tracker, &config)?;
            let id = dashboard::create_dashboard(
                &mut tracker, &config, &dashboard_cfg, &metric_name, &input_sub,
            )?;
            let raw_id = id.rsplit('/').next().unwrap_or(&id).to_string();
            Ok::<_, anyhow::Error>(format!(
                "https://console.cloud.google.com/monitoring/dashboards/builder/{}?project={}",
                raw_id, config.project
            ))
        })?;
        dashboard_url = Some(url);
    }

    let res = tracker.resources();
    println!("\nDeployed:");
    println!("  BigQuery dataset    {}.{}", res.biq_query.dataset_id, res.biq_query.table_id);
    println!("  Pub/Sub output      {}", res.output_pubsub.topic_id);
    println!("  GCS bucket          {}", res.bucket_name);
    println!("  Vector (Cloud Run)  {}", res.crs_instance);
    println!("  Dataflow job        {}", res.dataflow_pipeline_name);
    println!("  Vector SA           {}", res.vector_sa_email);
    println!("  Dataflow SA         {}", res.dataflow_sa_email);
    if notif_count > 0 {
        println!("  Notification chans  {}", notif_count);
    }
    if let Some(url) = &dashboard_url {
        println!("\nDashboard: {}", url);
    }
    println!();
    Ok(())
}
