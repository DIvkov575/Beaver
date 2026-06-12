//! High-level panel constructors for the Beaver SIEM Cloud Monitoring dashboard.
//!
//! Each function builds a [`Row`] representing a logical section of the
//! dashboard (throughput, detections, health, etc.) using the typed widget and
//! layout APIs from sibling modules. [`build_dashboard_grid`] assembles all
//! rows into a complete [`Grid`] ready for rendering.

use crate::lib::dashboard::layout::{Grid, Row};
use crate::lib::dashboard::widgets::{DataSet, PlotType, Widget};

// ─── PanelContext ───────────────────────────────────────────────────────────

/// All GCP resource identifiers the panel constructors need to scope their
/// metric filters, log queries, and console links.
pub struct PanelContext {
    pub project: String,
    pub region: String,
    pub metric_name: String,
    pub vector_service: String,
    pub dataflow_job: String,
    pub input_subscription: String,
    pub output_topic: String,
    pub bq_subscription: String,
    pub dataflow_subscription: String,
    pub bq_dataset: String,
    pub bq_table: String,
    pub bucket: String,
    pub vector_sa: String,
    pub dataflow_sa: String,
    pub alerts_topic: String,
    pub dlq_topic: String,
}

// ─── Row builders ───────────────────────────────────────────────────────────

/// Full-width markdown text widget with deep-links to every Cloud Console
/// resource page for this deployment.
pub fn resource_links_row(ctx: &PanelContext) -> Row {
    let links = format!(
        "[Cloud Run](https://console.cloud.google.com/run/detail/{region}/{vector_service}/metrics?project={project}) · \
         [Dataflow](https://console.cloud.google.com/dataflow/jobs?project={project}) · \
         [BigQuery](https://console.cloud.google.com/bigquery?project={project}&d={bq_dataset}&t={bq_table}) · \
         [Output topic](https://console.cloud.google.com/cloudpubsub/topic/detail/{output_topic}?project={project}) · \
         [BQ sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{bq_subscription}?project={project}) · \
         [DF sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{dataflow_subscription}?project={project}) · \
         [Bucket](https://console.cloud.google.com/storage/browser/{bucket}?project={project}) · \
         [Image](https://console.cloud.google.com/artifacts/docker/{project}/{region}/beaver-images?project={project}) · \
         [IAM](https://console.cloud.google.com/iam-admin/serviceaccounts?project={project}) · \
         [Logs](https://console.cloud.google.com/logs/query?project={project})",
        project = ctx.project,
        region = ctx.region,
        vector_service = ctx.vector_service,
        bq_dataset = ctx.bq_dataset,
        bq_table = ctx.bq_table,
        output_topic = ctx.output_topic,
        bq_subscription = ctx.bq_subscription,
        dataflow_subscription = ctx.dataflow_subscription,
        bucket = ctx.bucket,
    );

    Row::new(2).full(Widget::text("Resources", &links))
}

/// Ingestion rate (input subscription push requests) and output rate (output
/// topic send_message_operation_count), side by side.
pub fn throughput_row(ctx: &PanelContext) -> Row {
    let ingestion = Widget::xy_chart(
        "Ingestion rate (push requests/min)",
        vec![DataSet {
            filter: format!(
                r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/push_request_count" AND resource.label.subscription_id="{}""#,
                ctx.input_subscription
            ),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        }],
    );

    let output = Widget::xy_chart(
        "Output rate (messages/min)",
        vec![DataSet {
            filter: format!(
                r#"resource.type="pubsub_topic" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count" AND resource.label.topic_id="{}""#,
                ctx.output_topic
            ),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        }],
    );

    Row::new(4).half(ingestion).half(output)
}

/// Detection analytics: top firing rules table, detection rate by rule chart,
/// and alerts/min from the alerts topic.
pub fn detection_row(ctx: &PanelContext) -> Row {
    let metric_type = format!("logging.googleapis.com/user/{}", ctx.metric_name);

    let top_rules = Widget::time_series_table(
        "Top firing rules (1h)",
        vec![DataSet {
            filter: format!(r#"metric.type="{}""#, metric_type),
            alignment_period: "3600s".into(),
            aligner: "ALIGN_SUM".into(),
            reducer: Some("REDUCE_SUM".into()),
            group_by: vec!["metric.label.rule_name".into()],
            plot_type: PlotType::Line,
        }],
    );

    let rate_chart = Widget::xy_chart(
        "Detection rate by rule",
        vec![DataSet {
            filter: format!(r#"metric.type="{}""#, metric_type),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: Some("REDUCE_SUM".into()),
            group_by: vec!["metric.label.rule_name".into()],
            plot_type: PlotType::StackedArea,
        }],
    );

    let alerts_min = Widget::xy_chart(
        "Alerts / min",
        vec![DataSet {
            filter: format!(
                r#"resource.type="pubsub_topic" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count" AND resource.label.topic_id="{}""#,
                ctx.alerts_topic
            ),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        }],
    );

    Row::new(4).span(4, top_rules).span(4, rate_chart).span(4, alerts_min)
}

/// Subscription backlogs (BQ + Dataflow subscriptions) and DLQ messages/min.
pub fn pipeline_health_row(ctx: &PanelContext) -> Row {
    let backlogs = Widget::xy_chart(
        "Subscription backlogs (undelivered messages)",
        vec![
            DataSet {
                filter: format!(
                    r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/num_undelivered_messages" AND (resource.label.subscription_id="{}" OR resource.label.subscription_id="{}")"#,
                    ctx.bq_subscription, ctx.dataflow_subscription
                ),
                alignment_period: "60s".into(),
                aligner: "ALIGN_MAX".into(),
                reducer: Some("REDUCE_SUM".into()),
                group_by: vec!["resource.label.subscription_id".into()],
                plot_type: PlotType::Line,
            },
        ],
    );

    let dlq = Widget::xy_chart(
        "DLQ messages / min",
        vec![DataSet {
            filter: format!(
                r#"resource.type="pubsub_topic" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count" AND resource.label.topic_id="{}""#,
                ctx.dlq_topic
            ),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        }],
    );

    Row::new(4).half(backlogs).half(dlq)
}

/// Live detection event feed (BEAVER_SIEM_MATCH log entries).
pub fn logs_row(ctx: &PanelContext) -> Row {
    Row::new(4).full(Widget::logs_panel(
        "Detection events (live feed)",
        r#"resource.type="dataflow_step" AND jsonPayload.message:"BEAVER_SIEM_MATCH""#,
        &ctx.project,
    ))
}

/// Dataflow worker warnings and errors.
pub fn worker_logs_row(ctx: &PanelContext) -> Row {
    let filter = format!(
        r#"resource.type="dataflow_step" AND resource.labels.job_name="{}" AND severity>=WARNING"#,
        ctx.dataflow_job
    );
    Row::new(4).full(Widget::logs_panel(
        "Dataflow worker logs (warnings + errors)",
        &filter,
        &ctx.project,
    ))
}

/// Uniform grid of component-health `alertChart` widgets, 4 per row.
/// Each entry is a (label, policy_name) pair.
pub fn health_grid_row(component_health: &[(String, String)]) -> Row {
    let widgets: Vec<Widget> = component_health
        .iter()
        .map(|(label, policy_name)| Widget::alert_chart(label, policy_name))
        .collect();
    Row::new(3).grid(widgets, 4)
}

/// Notification delivery logs panel. Returns `None` if no alert policies exist.
pub fn alerting_row(ctx: &PanelContext, alert_policies: &[String]) -> Option<Row> {
    if alert_policies.is_empty() {
        return None;
    }
    Some(Row::new(4).full(Widget::logs_panel(
        "Notification delivery (recent)",
        r#"resource.type="alerting_policy""#,
        &ctx.project,
    )))
}

/// Grid of `alertChart` widgets for notification-route alert policies, 4 per
/// row. Returns `None` if the slice is empty.
pub fn alert_policies_grid_row(alert_policies: &[String]) -> Option<Row> {
    if alert_policies.is_empty() {
        return None;
    }
    let widgets: Vec<Widget> = alert_policies
        .iter()
        .map(|policy| {
            let short_id = policy.rsplit('/').next().unwrap_or(policy);
            Widget::alert_chart(&format!("Alert: {}", short_id), policy)
        })
        .collect();
    Some(Row::new(3).grid(widgets, 4))
}

// ─── Full grid assembly ─────────────────────────────────────────────────────

/// Assemble all panel rows into a complete dashboard grid.
///
/// Order: resource links, health grid, throughput, detection, pipeline health,
/// logs, worker logs, (optional) alerting, (optional) alert policies grid.
pub fn build_dashboard_grid(
    ctx: &PanelContext,
    component_health: &[(String, String)],
    alert_policies: &[String],
) -> Grid {
    let mut grid = Grid::new();

    grid.add_row(resource_links_row(ctx));
    grid.add_row(health_grid_row(component_health));
    grid.add_row(throughput_row(ctx));
    grid.add_row(detection_row(ctx));
    grid.add_row(pipeline_health_row(ctx));
    grid.add_row(logs_row(ctx));
    grid.add_row(worker_logs_row(ctx));

    if let Some(row) = alerting_row(ctx, alert_policies) {
        grid.add_row(row);
    }
    if let Some(row) = alert_policies_grid_row(alert_policies) {
        grid.add_row(row);
    }

    grid
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::dashboard::render::render_dashboard_yaml;

    fn test_ctx() -> PanelContext {
        PanelContext {
            project: "test-project".into(),
            region: "us-central1".into(),
            metric_name: "beaver_detection_count_abc123".into(),
            vector_service: "beaver-vector-xyz".into(),
            dataflow_job: "beaver-detections-xyz".into(),
            input_subscription: "beaver-input-sub".into(),
            output_topic: "beaver-output-topic".into(),
            bq_subscription: "beaver-bq-sub".into(),
            dataflow_subscription: "beaver-df-sub".into(),
            bq_dataset: "beaver_dataset".into(),
            bq_table: "events".into(),
            bucket: "beaver-bucket-xyz".into(),
            vector_sa: "vector-sa@test.iam.gserviceaccount.com".into(),
            dataflow_sa: "df-sa@test.iam.gserviceaccount.com".into(),
            alerts_topic: "beaver-alerts".into(),
            dlq_topic: "beaver-dlq".into(),
        }
    }

    #[test]
    fn resource_links_row_produces_one_tile() {
        let ctx = test_ctx();
        let row = resource_links_row(&ctx);
        let mut grid = Grid::new();
        grid.add_row(row);
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].width, 12);
        assert_eq!(tiles[0].height, 2);
    }

    #[test]
    fn throughput_row_produces_two_tiles() {
        let ctx = test_ctx();
        let row = throughput_row(&ctx);
        let mut grid = Grid::new();
        grid.add_row(row);
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].width, 6);
        assert_eq!(tiles[1].width, 6);
    }

    #[test]
    fn detection_row_produces_three_tiles() {
        let ctx = test_ctx();
        let row = detection_row(&ctx);
        let mut grid = Grid::new();
        grid.add_row(row);
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].width, 4);
        assert_eq!(tiles[1].width, 4);
        assert_eq!(tiles[2].width, 4);
    }

    #[test]
    fn full_dashboard_grid_is_valid_yaml() {
        let ctx = test_ctx();
        let component_health = vec![
            ("Cloud Run".to_string(), "projects/p/alertPolicies/h-cr".to_string()),
            ("Dataflow".to_string(), "projects/p/alertPolicies/h-df".to_string()),
            ("BigQuery".to_string(), "projects/p/alertPolicies/h-bq".to_string()),
            ("GCS".to_string(), "projects/p/alertPolicies/h-gcs".to_string()),
            ("Log metric".to_string(), "projects/p/alertPolicies/h-lm".to_string()),
        ];
        let alert_policies = vec![
            "projects/p/alertPolicies/notif-1".to_string(),
            "projects/p/alertPolicies/notif-2".to_string(),
        ];

        let grid = build_dashboard_grid(&ctx, &component_health, &alert_policies);
        let yaml_str = render_dashboard_yaml("Beaver SIEM Dashboard", &grid);

        // Must parse as valid YAML
        let parsed: serde_yaml::Value =
            serde_yaml::from_str(&yaml_str).expect("rendered dashboard must be valid YAML");

        // Verify top-level structure
        let root = parsed.as_mapping().unwrap();
        assert_eq!(
            root.get(&serde_yaml::Value::from("displayName")),
            Some(&serde_yaml::Value::from("Beaver SIEM Dashboard"))
        );

        let mosaic = root
            .get(&serde_yaml::Value::from("mosaicLayout"))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert_eq!(
            mosaic.get(&serde_yaml::Value::from("columns")),
            Some(&serde_yaml::Value::from(12u64))
        );

        let tiles = mosaic
            .get(&serde_yaml::Value::from("tiles"))
            .unwrap()
            .as_sequence()
            .unwrap();

        // Should have many tiles (resource links + 5 health + throughput x2 +
        // detection x3 + pipeline_health x2 + logs + worker_logs + alerting +
        // 2 alert policy charts)
        assert!(tiles.len() >= 15, "expected at least 15 tiles, got {}", tiles.len());
    }

    #[test]
    fn alerting_row_returns_none_when_empty() {
        let ctx = test_ctx();
        assert!(alerting_row(&ctx, &[]).is_none());
    }

    #[test]
    fn alert_policies_grid_row_returns_none_when_empty() {
        assert!(alert_policies_grid_row(&[]).is_none());
    }
}
