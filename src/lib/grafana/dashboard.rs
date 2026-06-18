use serde_json::{json, Value};

/// Builds a complete Grafana dashboard JSON model for Beaver SIEM.
///
/// The dashboard has 4 sections:
/// 1. Posture Bar - stat panels (total alerts 24h, critical/high/med/low, active rules, pipeline health)
/// 2. Detection Activity - heatmap, top-N table, source bar chart
/// 3. Pipeline Health - collapsible row with ingestion rate, latency, DLQ depth
/// 4. Investigation - recent detections log table, ad-hoc query placeholder
pub struct GrafanaDashboardBuilder {
    pub project: String,
    pub dataset: String,
    pub table: String,
    pub title: String,
}

/// Severity color constants (dark theme).
const COLOR_CRITICAL: &str = "#FF4444";
const COLOR_HIGH: &str = "#FF8800";
const COLOR_MEDIUM: &str = "#FFCC00";
const COLOR_LOW: &str = "#4488FF";

impl GrafanaDashboardBuilder {
    /// Builds the complete Grafana dashboard JSON with BigQuery datasource variable.
    pub fn build(&self) -> Value {
        self.build_internal("${DS_BEAVER}", false)
    }

    /// Builds the same dashboard but targeting SQLite datasource for local preview.
    pub fn build_preview(title: &str) -> Value {
        let builder = GrafanaDashboardBuilder {
            project: String::new(),
            dataset: String::new(),
            table: "events".to_string(),
            title: title.to_string(),
        };
        builder.build_internal("sqlite", true)
    }

    fn datasource(&self, uid: &str) -> Value {
        json!({
            "type": "grafana-bigquery-datasource",
            "uid": uid
        })
    }

    fn fully_qualified_table(&self) -> String {
        if self.project.is_empty() {
            self.table.clone()
        } else {
            format!("{}.{}.{}", self.project, self.dataset, self.table)
        }
    }

    fn time_filter(&self, preview: bool) -> &'static str {
        if preview {
            "timestamp >= datetime('now', '-24 hours')"
        } else {
            "$__timeFilter(timestamp)"
        }
    }

    fn build_internal(&self, ds_uid: &str, preview: bool) -> Value {
        let ds = self.datasource(ds_uid);
        let fqt = self.fully_qualified_table();
        let time_filter = self.time_filter(preview);

        let mut panels: Vec<Value> = Vec::new();
        let mut next_id: u32 = 1;
        let mut y_pos: u32 = 0;

        // =====================================================================
        // Section 1: Posture Bar
        // =====================================================================
        panels.push(self.row_panel(next_id, "Posture Bar", y_pos, false));
        next_id += 1;
        y_pos += 1;

        // Total Alerts (24h)
        panels.push(self.stat_panel(
            next_id,
            "Total Alerts (24h)",
            &format!(
                "SELECT COUNT(*) as value FROM `{}` WHERE {}",
                fqt, time_filter
            ),
            &ds,
            0,
            y_pos,
            4,
            4,
            None,
        ));
        next_id += 1;

        // Critical count
        panels.push(self.stat_panel(
            next_id,
            "Critical",
            &format!(
                "SELECT COUNT(*) as value FROM `{}` WHERE {} AND severity = 'critical'",
                fqt, time_filter
            ),
            &ds,
            4,
            y_pos,
            4,
            4,
            Some(COLOR_CRITICAL),
        ));
        next_id += 1;

        // High count
        panels.push(self.stat_panel(
            next_id,
            "High",
            &format!(
                "SELECT COUNT(*) as value FROM `{}` WHERE {} AND severity = 'high'",
                fqt, time_filter
            ),
            &ds,
            8,
            y_pos,
            4,
            4,
            Some(COLOR_HIGH),
        ));
        next_id += 1;

        // Medium count
        panels.push(self.stat_panel(
            next_id,
            "Medium",
            &format!(
                "SELECT COUNT(*) as value FROM `{}` WHERE {} AND severity = 'medium'",
                fqt, time_filter
            ),
            &ds,
            12,
            y_pos,
            4,
            4,
            Some(COLOR_MEDIUM),
        ));
        next_id += 1;

        // Low count
        panels.push(self.stat_panel(
            next_id,
            "Low",
            &format!(
                "SELECT COUNT(*) as value FROM `{}` WHERE {} AND severity = 'low'",
                fqt, time_filter
            ),
            &ds,
            16,
            y_pos,
            4,
            4,
            Some(COLOR_LOW),
        ));
        next_id += 1;

        // Active Rules
        panels.push(self.stat_panel(
            next_id,
            "Active Rules",
            &format!(
                "SELECT COUNT(DISTINCT rule_name) as value FROM `{}` WHERE {}",
                fqt, time_filter
            ),
            &ds,
            20,
            y_pos,
            2,
            4,
            None,
        ));
        next_id += 1;

        // Pipeline Health
        panels.push(self.stat_panel(
            next_id,
            "Pipeline Health",
            &format!(
                "SELECT CASE WHEN COUNT(*) > 0 THEN 'Healthy' ELSE 'No Data' END as value FROM `{}` WHERE {}",
                fqt, time_filter
            ),
            &ds,
            22,
            y_pos,
            2,
            4,
            None,
        ));
        next_id += 1;
        y_pos += 4;

        // =====================================================================
        // Section 2: Detection Activity
        // =====================================================================
        panels.push(self.row_panel(next_id, "Detection Activity", y_pos, false));
        next_id += 1;
        y_pos += 1;

        // Heatmap: alerts by severity over time
        panels.push(self.heatmap_panel(
            next_id,
            "Alerts by Severity Over Time",
            &format!(
                "SELECT timestamp, severity, COUNT(*) as count FROM `{}` WHERE {} GROUP BY timestamp, severity ORDER BY timestamp",
                fqt, time_filter
            ),
            &ds,
            0,
            y_pos,
            12,
            8,
        ));
        next_id += 1;

        // Table: top-N firing rules
        panels.push(self.table_panel(
            next_id,
            "Top Firing Rules",
            &format!(
                "SELECT rule_name, severity, COUNT(*) as fire_count FROM `{}` WHERE {} GROUP BY rule_name, severity ORDER BY fire_count DESC LIMIT 20",
                fqt, time_filter
            ),
            &ds,
            12,
            y_pos,
            12,
            4,
        ));
        next_id += 1;

        // Bar chart: top sources
        panels.push(self.barchart_panel(
            next_id,
            "Top Sources",
            &format!(
                "SELECT source_ip, COUNT(*) as count FROM `{}` WHERE {} GROUP BY source_ip ORDER BY count DESC LIMIT 10",
                fqt, time_filter
            ),
            &ds,
            12,
            y_pos + 4,
            12,
            4,
        ));
        next_id += 1;
        y_pos += 8;

        // =====================================================================
        // Section 3: Pipeline Health (collapsible)
        // =====================================================================
        panels.push(self.row_panel(next_id, "Pipeline Health", y_pos, true));
        next_id += 1;
        y_pos += 1;

        // Ingestion Rate
        panels.push(self.timeseries_panel(
            next_id,
            "Ingestion Rate",
            &format!(
                "SELECT timestamp, COUNT(*) as events_per_minute FROM `{}` WHERE {} GROUP BY timestamp ORDER BY timestamp",
                fqt, time_filter
            ),
            &ds,
            0,
            y_pos,
            8,
            6,
        ));
        next_id += 1;

        // Processing Latency
        panels.push(self.timeseries_panel(
            next_id,
            "Processing Latency",
            &format!(
                "SELECT timestamp, AVG(processing_latency_ms) as avg_latency_ms FROM `{}` WHERE {} GROUP BY timestamp ORDER BY timestamp",
                fqt, time_filter
            ),
            &ds,
            8,
            y_pos,
            8,
            6,
        ));
        next_id += 1;

        // DLQ Depth
        panels.push(self.timeseries_panel(
            next_id,
            "DLQ Depth",
            &format!(
                "SELECT timestamp, COUNT(*) as dlq_messages FROM `{}_dlq` WHERE {} GROUP BY timestamp ORDER BY timestamp",
                fqt, time_filter
            ),
            &ds,
            16,
            y_pos,
            8,
            6,
        ));
        next_id += 1;
        y_pos += 6;

        // =====================================================================
        // Section 4: Investigation
        // =====================================================================
        panels.push(self.row_panel(next_id, "Investigation", y_pos, false));
        next_id += 1;
        y_pos += 1;

        // Recent detections log table
        panels.push(self.table_panel(
            next_id,
            "Recent Detections",
            &format!(
                "SELECT timestamp, rule_name, severity, source_ip, destination_ip, raw_event FROM `{}` WHERE {} ORDER BY timestamp DESC LIMIT 100",
                fqt, time_filter
            ),
            &ds,
            0,
            y_pos,
            24,
            8,
        ));
        next_id += 1;
        y_pos += 8;

        // Ad-hoc query placeholder
        panels.push(self.text_panel(
            next_id,
            "Ad-hoc Query",
            &format!(
                "Use this panel as a starting point for custom BigQuery investigations.\n\n```sql\nSELECT *\nFROM `{}`\nWHERE $__timeFilter(timestamp)\n  AND rule_name = '$rule_name'\nORDER BY timestamp DESC\nLIMIT 500\n```",
                fqt
            ),
            0,
            y_pos,
            24,
            4,
        ));
        let _ = next_id; // suppress unused warning

        // Build templating variables
        let templating = self.build_templating(&ds, &fqt, time_filter);

        json!({
            "dashboard": {
                "title": self.title,
                "uid": "beaver-siem-dashboard",
                "schemaVersion": 39,
                "version": 1,
                "timezone": "browser",
                "editable": true,
                "style": "dark",
                "panels": panels,
                "templating": templating,
                "time": {
                    "from": "now-24h",
                    "to": "now"
                },
                "refresh": "1m",
                "annotations": {
                    "list": []
                }
            }
        })
    }

    fn build_templating(&self, ds: &Value, fqt: &str, time_filter: &str) -> Value {
        json!({
            "list": [
                {
                    "name": "severity",
                    "type": "query",
                    "datasource": ds,
                    "query": format!(
                        "SELECT DISTINCT severity FROM `{}` WHERE {} ORDER BY severity",
                        fqt, time_filter
                    ),
                    "multi": true,
                    "includeAll": true,
                    "current": {
                        "text": "All",
                        "value": "$__all"
                    }
                },
                {
                    "name": "rule_name",
                    "type": "query",
                    "datasource": ds,
                    "query": format!(
                        "SELECT DISTINCT rule_name FROM `{}` WHERE {} ORDER BY rule_name",
                        fqt, time_filter
                    ),
                    "multi": true,
                    "includeAll": true,
                    "current": {
                        "text": "All",
                        "value": "$__all"
                    }
                }
            ]
        })
    }

    fn row_panel(&self, id: u32, title: &str, y: u32, collapsed: bool) -> Value {
        json!({
            "id": id,
            "type": "row",
            "title": title,
            "collapsed": collapsed,
            "gridPos": { "h": 1, "w": 24, "x": 0, "y": y },
            "panels": []
        })
    }

    fn stat_panel(
        &self,
        id: u32,
        title: &str,
        query: &str,
        ds: &Value,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        color: Option<&str>,
    ) -> Value {
        let mut panel = json!({
            "id": id,
            "type": "stat",
            "title": title,
            "datasource": ds,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "targets": [
                {
                    "rawSql": query,
                    "format": "table"
                }
            ],
            "options": {
                "reduceOptions": {
                    "calcs": ["lastNotNull"],
                    "fields": "",
                    "values": false
                },
                "textMode": "auto",
                "colorMode": "value",
                "graphMode": "none"
            },
            "fieldConfig": {
                "defaults": {
                    "thresholds": {
                        "mode": "absolute",
                        "steps": [
                            { "color": "green", "value": null }
                        ]
                    }
                },
                "overrides": []
            }
        });

        if let Some(c) = color {
            panel["fieldConfig"]["defaults"]["color"] = json!({
                "mode": "fixed",
                "fixedColor": c
            });
        }

        panel
    }

    fn heatmap_panel(
        &self,
        id: u32,
        title: &str,
        query: &str,
        ds: &Value,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Value {
        json!({
            "id": id,
            "type": "heatmap",
            "title": title,
            "datasource": ds,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "targets": [
                {
                    "rawSql": query,
                    "format": "table"
                }
            ],
            "options": {
                "color": {
                    "scheme": "Oranges",
                    "mode": "scheme"
                }
            },
            "fieldConfig": {
                "defaults": {},
                "overrides": [
                    {
                        "matcher": { "id": "byName", "options": "critical" },
                        "properties": [{ "id": "color", "value": { "fixedColor": COLOR_CRITICAL, "mode": "fixed" } }]
                    },
                    {
                        "matcher": { "id": "byName", "options": "high" },
                        "properties": [{ "id": "color", "value": { "fixedColor": COLOR_HIGH, "mode": "fixed" } }]
                    },
                    {
                        "matcher": { "id": "byName", "options": "medium" },
                        "properties": [{ "id": "color", "value": { "fixedColor": COLOR_MEDIUM, "mode": "fixed" } }]
                    },
                    {
                        "matcher": { "id": "byName", "options": "low" },
                        "properties": [{ "id": "color", "value": { "fixedColor": COLOR_LOW, "mode": "fixed" } }]
                    }
                ]
            }
        })
    }

    fn table_panel(
        &self,
        id: u32,
        title: &str,
        query: &str,
        ds: &Value,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Value {
        json!({
            "id": id,
            "type": "table",
            "title": title,
            "datasource": ds,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "targets": [
                {
                    "rawSql": query,
                    "format": "table"
                }
            ],
            "options": {
                "showHeader": true,
                "sortBy": []
            },
            "fieldConfig": {
                "defaults": {},
                "overrides": [
                    {
                        "matcher": { "id": "byName", "options": "severity" },
                        "properties": [{
                            "id": "custom.displayMode",
                            "value": "color-background"
                        }, {
                            "id": "mappings",
                            "value": [
                                { "type": "value", "options": { "critical": { "color": COLOR_CRITICAL } } },
                                { "type": "value", "options": { "high": { "color": COLOR_HIGH } } },
                                { "type": "value", "options": { "medium": { "color": COLOR_MEDIUM } } },
                                { "type": "value", "options": { "low": { "color": COLOR_LOW } } }
                            ]
                        }]
                    }
                ]
            }
        })
    }

    fn barchart_panel(
        &self,
        id: u32,
        title: &str,
        query: &str,
        ds: &Value,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Value {
        json!({
            "id": id,
            "type": "barchart",
            "title": title,
            "datasource": ds,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "targets": [
                {
                    "rawSql": query,
                    "format": "table"
                }
            ],
            "options": {
                "orientation": "horizontal",
                "xField": "source_ip",
                "barWidth": 0.8
            },
            "fieldConfig": {
                "defaults": {
                    "color": {
                        "mode": "palette-classic"
                    }
                },
                "overrides": []
            }
        })
    }

    fn timeseries_panel(
        &self,
        id: u32,
        title: &str,
        query: &str,
        ds: &Value,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Value {
        json!({
            "id": id,
            "type": "timeseries",
            "title": title,
            "datasource": ds,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "targets": [
                {
                    "rawSql": query,
                    "format": "time_series"
                }
            ],
            "options": {
                "legend": {
                    "displayMode": "list",
                    "placement": "bottom"
                },
                "tooltip": {
                    "mode": "single"
                }
            },
            "fieldConfig": {
                "defaults": {
                    "custom": {
                        "lineWidth": 2,
                        "fillOpacity": 10,
                        "spanNulls": false
                    }
                },
                "overrides": []
            }
        })
    }

    fn text_panel(
        &self,
        id: u32,
        title: &str,
        content: &str,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Value {
        json!({
            "id": id,
            "type": "text",
            "title": title,
            "gridPos": { "h": h, "w": w, "x": x, "y": y },
            "options": {
                "mode": "markdown",
                "content": content
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_builder() -> GrafanaDashboardBuilder {
        GrafanaDashboardBuilder {
            project: "my-project".to_string(),
            dataset: "beaver_siem".to_string(),
            table: "events".to_string(),
            title: "Beaver SIEM Dashboard".to_string(),
        }
    }

    #[test]
    fn test_build_produces_valid_json_with_expected_panel_count() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        // 4 row panels + 7 stat panels + 1 heatmap + 1 table + 1 barchart
        // + 3 timeseries + 1 table + 1 text = 19 panels total
        assert_eq!(panels.len(), 19);
    }

    #[test]
    fn test_all_four_sections_present() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        let row_titles: Vec<&str> = panels
            .iter()
            .filter(|p| p["type"] == "row")
            .map(|p| p["title"].as_str().unwrap())
            .collect();

        assert!(
            row_titles.contains(&"Posture Bar"),
            "Missing Posture Bar section"
        );
        assert!(
            row_titles.contains(&"Detection Activity"),
            "Missing Detection Activity section"
        );
        assert!(
            row_titles.contains(&"Pipeline Health"),
            "Missing Pipeline Health section"
        );
        assert!(
            row_titles.contains(&"Investigation"),
            "Missing Investigation section"
        );
    }

    #[test]
    fn test_datasource_references_correct() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        for panel in panels {
            if let Some(ds) = panel.get("datasource") {
                assert_eq!(
                    ds["uid"].as_str().unwrap(),
                    "${DS_BEAVER}",
                    "Panel '{}' has wrong datasource uid",
                    panel["title"].as_str().unwrap_or("unknown")
                );
                assert_eq!(
                    ds["type"].as_str().unwrap(),
                    "grafana-bigquery-datasource"
                );
            }
        }
    }

    #[test]
    fn test_preview_datasource_is_sqlite() {
        let dashboard = GrafanaDashboardBuilder::build_preview("Preview Dashboard");

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        for panel in panels {
            if let Some(ds) = panel.get("datasource") {
                assert_eq!(
                    ds["uid"].as_str().unwrap(),
                    "sqlite",
                    "Preview panel '{}' should use sqlite datasource",
                    panel["title"].as_str().unwrap_or("unknown")
                );
            }
        }
    }

    #[test]
    fn test_grid_positions_do_not_overlap() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();

        // Collect all panel rectangles (excluding row panels which are just dividers)
        let rects: Vec<(u32, u32, u32, u32, &str)> = panels
            .iter()
            .filter(|p| p["type"] != "row")
            .map(|p| {
                let gp = &p["gridPos"];
                let x = gp["x"].as_u64().unwrap() as u32;
                let y = gp["y"].as_u64().unwrap() as u32;
                let w = gp["w"].as_u64().unwrap() as u32;
                let h = gp["h"].as_u64().unwrap() as u32;
                (x, y, w, h, p["title"].as_str().unwrap_or("unknown"))
            })
            .collect();

        // Check no two panels on the same row(s) overlap
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (x1, y1, w1, h1, title1) = rects[i];
                let (x2, y2, w2, h2, title2) = rects[j];

                // Two rectangles overlap if they overlap on both axes
                let x_overlap = x1 < x2 + w2 && x2 < x1 + w1;
                let y_overlap = y1 < y2 + h2 && y2 < y1 + h1;

                assert!(
                    !(x_overlap && y_overlap),
                    "Panels '{}' (x:{},y:{},w:{},h:{}) and '{}' (x:{},y:{},w:{},h:{}) overlap",
                    title1, x1, y1, w1, h1, title2, x2, y2, w2, h2
                );
            }
        }
    }

    #[test]
    fn test_dashboard_metadata() {
        let builder = make_builder();
        let dashboard = builder.build();

        let db = &dashboard["dashboard"];
        assert_eq!(db["title"].as_str().unwrap(), "Beaver SIEM Dashboard");
        assert_eq!(db["uid"].as_str().unwrap(), "beaver-siem-dashboard");
        assert_eq!(db["timezone"].as_str().unwrap(), "browser");
        assert_eq!(db["style"].as_str().unwrap(), "dark");
        assert!(db["templating"]["list"].as_array().unwrap().len() >= 2);
    }

    #[test]
    fn test_pipeline_health_section_is_collapsed() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        let health_row = panels
            .iter()
            .find(|p| p["type"] == "row" && p["title"] == "Pipeline Health")
            .expect("Pipeline Health row not found");

        assert_eq!(health_row["collapsed"].as_bool().unwrap(), true);
    }

    #[test]
    fn test_severity_colors_in_overrides() {
        let builder = make_builder();
        let dashboard = builder.build();

        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();

        // Find the Critical stat panel and verify its color
        let critical_panel = panels
            .iter()
            .find(|p| p["title"] == "Critical")
            .expect("Critical panel not found");

        let fixed_color = critical_panel["fieldConfig"]["defaults"]["color"]["fixedColor"]
            .as_str()
            .unwrap();
        assert_eq!(fixed_color, "#FF4444");
    }
}
