//! Typed widget system for Cloud Monitoring dashboard YAML generation.
//!
//! Each variant of [`Widget`] maps to a single Cloud Monitoring widget JSON/YAML
//! shape. Constructors accept `&str` for ergonomics; `to_yaml_value()` produces
//! a `serde_yaml::Value` ready for embedding in a mosaic layout tile.

use serde_yaml::{Mapping, Value};

// ─── Plot type ───────────────────────────────────────────────────────────────

/// The visual representation style for an XY chart data series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlotType {
    Line,
    StackedBar,
    StackedArea,
}

impl PlotType {
    fn as_str(&self) -> &'static str {
        match self {
            PlotType::Line => "LINE",
            PlotType::StackedBar => "STACKED_BAR",
            PlotType::StackedArea => "STACKED_AREA",
        }
    }
}

// ─── DataSet ─────────────────────────────────────────────────────────────────

/// A single time-series query + visual config inside a chart widget.
#[derive(Debug, Clone, PartialEq)]
pub struct DataSet {
    pub filter: String,
    pub alignment_period: String,
    pub aligner: String,
    pub reducer: Option<String>,
    pub group_by: Vec<String>,
    pub plot_type: PlotType,
}

impl DataSet {
    pub fn new(
        filter: &str,
        alignment_period: &str,
        aligner: &str,
        reducer: Option<&str>,
        group_by: Vec<&str>,
        plot_type: PlotType,
    ) -> Self {
        Self {
            filter: filter.to_owned(),
            alignment_period: alignment_period.to_owned(),
            aligner: aligner.to_owned(),
            reducer: reducer.map(|s| s.to_owned()),
            group_by: group_by.into_iter().map(|s| s.to_owned()).collect(),
            plot_type,
        }
    }

    /// Render the dataset as a `serde_yaml::Value` suitable for inclusion in
    /// a `dataSets` array.
    fn to_yaml_value(&self) -> Value {
        let mut aggregation = Mapping::new();
        aggregation.insert(
            Value::String("alignmentPeriod".into()),
            Value::String(self.alignment_period.clone()),
        );
        aggregation.insert(
            Value::String("perSeriesAligner".into()),
            Value::String(self.aligner.clone()),
        );
        if let Some(ref reducer) = self.reducer {
            aggregation.insert(
                Value::String("crossSeriesReducer".into()),
                Value::String(reducer.clone()),
            );
        }
        if !self.group_by.is_empty() {
            let fields: Vec<Value> = self
                .group_by
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect();
            aggregation.insert(
                Value::String("groupByFields".into()),
                Value::Sequence(fields),
            );
        }

        let mut ts_filter = Mapping::new();
        ts_filter.insert(
            Value::String("filter".into()),
            Value::String(self.filter.clone()),
        );
        ts_filter.insert(
            Value::String("aggregation".into()),
            Value::Mapping(aggregation),
        );

        let mut ts_query = Mapping::new();
        ts_query.insert(
            Value::String("timeSeriesFilter".into()),
            Value::Mapping(ts_filter),
        );

        let mut ds = Mapping::new();
        ds.insert(
            Value::String("timeSeriesQuery".into()),
            Value::Mapping(ts_query),
        );
        ds.insert(
            Value::String("plotType".into()),
            Value::String(self.plot_type.as_str().to_owned()),
        );

        Value::Mapping(ds)
    }
}

// ─── Threshold ───────────────────────────────────────────────────────────────

/// Colour of a scorecard threshold indicator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThresholdColor {
    Red,
    Yellow,
}

impl ThresholdColor {
    fn as_str(&self) -> &'static str {
        match self {
            ThresholdColor::Red => "RED",
            ThresholdColor::Yellow => "YELLOW",
        }
    }
}

/// Whether the threshold triggers above or below the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThresholdDirection {
    Above,
    Below,
}

impl ThresholdDirection {
    fn as_str(&self) -> &'static str {
        match self {
            ThresholdDirection::Above => "ABOVE",
            ThresholdDirection::Below => "BELOW",
        }
    }
}

/// Configuration for a scorecard threshold indicator.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdConfig {
    pub value: f64,
    pub color: ThresholdColor,
    pub direction: ThresholdDirection,
}

impl ThresholdConfig {
    pub fn new(value: f64, color: ThresholdColor, direction: ThresholdDirection) -> Self {
        Self {
            value,
            color,
            direction,
        }
    }

    fn to_yaml_value(&self) -> Value {
        let mut m = Mapping::new();
        m.insert(
            Value::String("value".into()),
            Value::Number(serde_yaml::Number::from(self.value)),
        );
        m.insert(
            Value::String("color".into()),
            Value::String(self.color.as_str().to_owned()),
        );
        m.insert(
            Value::String("direction".into()),
            Value::String(self.direction.as_str().to_owned()),
        );
        Value::Mapping(m)
    }
}

// ─── Widget enum ─────────────────────────────────────────────────────────────

/// A typed Cloud Monitoring dashboard widget.
#[derive(Debug, Clone, PartialEq)]
pub enum Widget {
    /// Markdown text block.
    Text {
        title: String,
        content: String,
    },
    /// Live log stream panel.
    LogsPanel {
        title: String,
        filter: String,
        project: String,
    },
    /// XY line/bar/area chart with one or more data series.
    XyChart {
        title: String,
        datasets: Vec<DataSet>,
    },
    /// Tabular time-series display (top-N style).
    TimeSeriesTable {
        title: String,
        datasets: Vec<DataSet>,
    },
    /// Shows the state of an alerting policy.
    AlertChart {
        title: String,
        policy_name: String,
    },
    /// Single-value scorecard with optional threshold colouring.
    Scorecard {
        title: String,
        dataset: DataSet,
        threshold: Option<ThresholdConfig>,
    },
}

impl Widget {
    // ── Constructors ──────────────────────────────────────────────────────

    pub fn text(title: &str, content: &str) -> Self {
        Widget::Text {
            title: title.to_owned(),
            content: content.to_owned(),
        }
    }

    pub fn logs_panel(title: &str, filter: &str, project: &str) -> Self {
        Widget::LogsPanel {
            title: title.to_owned(),
            filter: filter.to_owned(),
            project: project.to_owned(),
        }
    }

    pub fn xy_chart(title: &str, datasets: Vec<DataSet>) -> Self {
        Widget::XyChart {
            title: title.to_owned(),
            datasets,
        }
    }

    pub fn time_series_table(title: &str, datasets: Vec<DataSet>) -> Self {
        Widget::TimeSeriesTable {
            title: title.to_owned(),
            datasets,
        }
    }

    pub fn alert_chart(title: &str, policy_name: &str) -> Self {
        Widget::AlertChart {
            title: title.to_owned(),
            policy_name: policy_name.to_owned(),
        }
    }

    pub fn scorecard(title: &str, dataset: DataSet, threshold: Option<ThresholdConfig>) -> Self {
        Widget::Scorecard {
            title: title.to_owned(),
            dataset,
            threshold,
        }
    }

    // ── YAML serialization ───────────────────────────────────────────────

    /// Serialize this widget to a `serde_yaml::Value` matching the Cloud
    /// Monitoring dashboard widget schema.
    pub fn to_yaml_value(&self) -> Value {
        let mut root = Mapping::new();

        match self {
            Widget::Text { title, content } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));
                let mut text = Mapping::new();
                text.insert(
                    Value::String("content".into()),
                    Value::String(content.clone()),
                );
                text.insert(
                    Value::String("format".into()),
                    Value::String("MARKDOWN".into()),
                );
                root.insert(Value::String("text".into()), Value::Mapping(text));
            }
            Widget::LogsPanel {
                title,
                filter,
                project,
            } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));
                let mut panel = Mapping::new();
                panel.insert(
                    Value::String("filter".into()),
                    Value::String(filter.clone()),
                );
                panel.insert(
                    Value::String("resourceNames".into()),
                    Value::Sequence(vec![Value::String(format!("projects/{}", project))]),
                );
                root.insert(Value::String("logsPanel".into()), Value::Mapping(panel));
            }
            Widget::XyChart { title, datasets } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));
                let ds_values: Vec<Value> =
                    datasets.iter().map(|ds| ds.to_yaml_value()).collect();
                let mut chart = Mapping::new();
                chart.insert(
                    Value::String("dataSets".into()),
                    Value::Sequence(ds_values),
                );
                root.insert(Value::String("xyChart".into()), Value::Mapping(chart));
            }
            Widget::TimeSeriesTable { title, datasets } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));
                let ds_values: Vec<Value> =
                    datasets.iter().map(|ds| ds.to_yaml_value()).collect();
                let mut table = Mapping::new();
                table.insert(
                    Value::String("dataSets".into()),
                    Value::Sequence(ds_values),
                );
                root.insert(
                    Value::String("timeSeriesTable".into()),
                    Value::Mapping(table),
                );
            }
            Widget::AlertChart { title, policy_name } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));
                let mut chart = Mapping::new();
                chart.insert(
                    Value::String("name".into()),
                    Value::String(policy_name.clone()),
                );
                root.insert(Value::String("alertChart".into()), Value::Mapping(chart));
            }
            Widget::Scorecard {
                title,
                dataset,
                threshold,
            } => {
                root.insert(Value::String("title".into()), Value::String(title.clone()));

                let mut scorecard = Mapping::new();

                // Build timeSeriesQuery from the dataset (reuse internal structure)
                let ds_val = dataset.to_yaml_value();
                if let Value::Mapping(ref ds_map) = ds_val {
                    if let Some(tsq) = ds_map.get(Value::String("timeSeriesQuery".into())) {
                        scorecard.insert(
                            Value::String("timeSeriesQuery".into()),
                            tsq.clone(),
                        );
                    }
                }

                if let Some(ref thr) = threshold {
                    scorecard.insert(
                        Value::String("thresholds".into()),
                        Value::Sequence(vec![thr.to_yaml_value()]),
                    );
                }

                root.insert(
                    Value::String("scorecard".into()),
                    Value::Mapping(scorecard),
                );
            }
        }

        Value::Mapping(root)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: look up a key in a YAML Mapping value.
    fn get<'a>(val: &'a Value, key: &str) -> &'a Value {
        val.as_mapping()
            .and_then(|m| m.get(&Value::String(key.into())))
            .unwrap_or(&Value::Null)
    }

    fn sample_dataset() -> DataSet {
        DataSet::new(
            "metric.type=\"compute.googleapis.com/instance/cpu/utilization\"",
            "60s",
            "ALIGN_MEAN",
            Some("REDUCE_SUM"),
            vec!["metric.label.instance_name"],
            PlotType::Line,
        )
    }

    #[test]
    fn text_widget_renders_markdown() {
        let w = Widget::text("Status", "All systems **operational**");
        let v = w.to_yaml_value();

        assert_eq!(get(&v, "title"), &Value::String("Status".into()));
        let text = get(&v, "text");
        assert_eq!(
            get(text, "content"),
            &Value::String("All systems **operational**".into())
        );
        assert_eq!(get(text, "format"), &Value::String("MARKDOWN".into()));
    }

    #[test]
    fn logs_panel_renders_filter_and_resource() {
        let w = Widget::logs_panel("Live Feed", "severity>=WARNING", "my-project");
        let v = w.to_yaml_value();

        assert_eq!(get(&v, "title"), &Value::String("Live Feed".into()));
        let panel = get(&v, "logsPanel");
        assert_eq!(
            get(panel, "filter"),
            &Value::String("severity>=WARNING".into())
        );
        let resources = get(panel, "resourceNames");
        let seq = resources.as_sequence().expect("resourceNames is a sequence");
        assert_eq!(seq.len(), 1);
        assert_eq!(seq[0], Value::String("projects/my-project".into()));
    }

    #[test]
    fn xy_chart_renders_datasets_with_aggregation() {
        let ds = sample_dataset();
        let w = Widget::xy_chart("CPU Usage", vec![ds]);
        let v = w.to_yaml_value();

        assert_eq!(get(&v, "title"), &Value::String("CPU Usage".into()));
        let chart = get(&v, "xyChart");
        let datasets = get(chart, "dataSets")
            .as_sequence()
            .expect("dataSets is a sequence");
        assert_eq!(datasets.len(), 1);

        let ds0 = &datasets[0];
        let tsq = get(ds0, "timeSeriesQuery");
        let tsf = get(tsq, "timeSeriesFilter");
        assert_eq!(
            get(tsf, "filter"),
            &Value::String(
                "metric.type=\"compute.googleapis.com/instance/cpu/utilization\"".into()
            )
        );
        let agg = get(tsf, "aggregation");
        assert_eq!(
            get(agg, "alignmentPeriod"),
            &Value::String("60s".into())
        );
        assert_eq!(
            get(agg, "perSeriesAligner"),
            &Value::String("ALIGN_MEAN".into())
        );
        assert_eq!(
            get(agg, "crossSeriesReducer"),
            &Value::String("REDUCE_SUM".into())
        );
        let group = get(agg, "groupByFields")
            .as_sequence()
            .expect("groupByFields is a sequence");
        assert_eq!(group, &[Value::String("metric.label.instance_name".into())]);

        assert_eq!(get(ds0, "plotType"), &Value::String("LINE".into()));
    }

    #[test]
    fn time_series_table_uses_correct_key() {
        let ds = DataSet::new(
            "metric.type=\"custom/foo\"",
            "3600s",
            "ALIGN_SUM",
            None,
            vec![],
            PlotType::StackedBar,
        );
        let w = Widget::time_series_table("Top Items", vec![ds]);
        let v = w.to_yaml_value();

        assert_eq!(get(&v, "title"), &Value::String("Top Items".into()));
        // Key must be timeSeriesTable, not xyChart.
        assert!(get(&v, "timeSeriesTable").is_mapping());
        assert!(get(&v, "xyChart").is_null());

        let table = get(&v, "timeSeriesTable");
        let datasets = get(table, "dataSets")
            .as_sequence()
            .expect("dataSets");
        assert_eq!(datasets.len(), 1);
        // No reducer and no groupBy — those keys should be absent.
        let agg = get(
            get(get(&datasets[0], "timeSeriesQuery"), "timeSeriesFilter"),
            "aggregation",
        );
        assert!(get(agg, "crossSeriesReducer").is_null());
        assert!(get(agg, "groupByFields").is_null());
        // Plot type for table datasets.
        assert_eq!(
            get(&datasets[0], "plotType"),
            &Value::String("STACKED_BAR".into())
        );
    }

    #[test]
    fn alert_chart_renders_policy_name() {
        let w = Widget::alert_chart(
            "Pipeline Health",
            "projects/p/alertPolicies/abc123",
        );
        let v = w.to_yaml_value();

        assert_eq!(
            get(&v, "title"),
            &Value::String("Pipeline Health".into())
        );
        let chart = get(&v, "alertChart");
        assert_eq!(
            get(chart, "name"),
            &Value::String("projects/p/alertPolicies/abc123".into())
        );
    }

    #[test]
    fn scorecard_renders_query_and_threshold() {
        let ds = DataSet::new(
            "metric.type=\"custom/latency\"",
            "300s",
            "ALIGN_PERCENTILE_99",
            None,
            vec![],
            PlotType::Line,
        );
        let thr = ThresholdConfig::new(500.0, ThresholdColor::Red, ThresholdDirection::Above);
        let w = Widget::scorecard("P99 Latency", ds, Some(thr));
        let v = w.to_yaml_value();

        assert_eq!(
            get(&v, "title"),
            &Value::String("P99 Latency".into())
        );
        let sc = get(&v, "scorecard");
        // timeSeriesQuery should be present inside scorecard.
        let tsq = get(sc, "timeSeriesQuery");
        assert!(tsq.is_mapping());
        let tsf = get(tsq, "timeSeriesFilter");
        assert_eq!(
            get(tsf, "filter"),
            &Value::String("metric.type=\"custom/latency\"".into())
        );
        // Thresholds array.
        let thresholds = get(sc, "thresholds")
            .as_sequence()
            .expect("thresholds is a sequence");
        assert_eq!(thresholds.len(), 1);
        let t0 = &thresholds[0];
        assert_eq!(get(t0, "color"), &Value::String("RED".into()));
        assert_eq!(get(t0, "direction"), &Value::String("ABOVE".into()));
    }

    #[test]
    fn scorecard_without_threshold_omits_thresholds_key() {
        let ds = DataSet::new(
            "metric.type=\"custom/count\"",
            "60s",
            "ALIGN_SUM",
            None,
            vec![],
            PlotType::Line,
        );
        let w = Widget::scorecard("Event Count", ds, None);
        let v = w.to_yaml_value();

        let sc = get(&v, "scorecard");
        assert!(sc.is_mapping());
        // No thresholds key when None.
        assert!(get(sc, "thresholds").is_null());
    }

    #[test]
    fn stacked_area_plot_type_string() {
        let ds = DataSet::new(
            "metric.type=\"custom/bytes\"",
            "60s",
            "ALIGN_RATE",
            None,
            vec![],
            PlotType::StackedArea,
        );
        let w = Widget::xy_chart("Throughput", vec![ds]);
        let v = w.to_yaml_value();
        let chart = get(&v, "xyChart");
        let datasets = get(chart, "dataSets").as_sequence().unwrap();
        assert_eq!(
            get(&datasets[0], "plotType"),
            &Value::String("STACKED_AREA".into())
        );
    }
}
