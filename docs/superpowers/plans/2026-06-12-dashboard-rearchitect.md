# Dashboard Re-Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-architect the SOC dashboard module from a single monolithic YAML string-builder into a composable widget system with richer visualization panels (pipeline throughput, latency percentiles, error budget, correlation state) and cleaner code structure.

**Architecture:** Split `dashboard.rs` into a module directory (`src/lib/dashboard/`) with: `mod.rs` (public API + config), `widgets.rs` (individual widget builders), `layout.rs` (grid layout engine), `render.rs` (YAML serialization), and `health.rs` (component health policies). Each widget is a struct that knows its own size and renders to YAML independently. The layout engine arranges widgets into rows with configurable column spans.

**Tech Stack:** Rust, serde_yaml for structured serialization (replacing hand-rolled format strings), GCP Cloud Monitoring dashboard YAML spec.

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/lib/dashboard/mod.rs` | Public API: `create_dashboard`, `delete_dashboard`, `create_log_metric`, `delete_log_metric`, `DashboardConfig`. Re-exports. |
| `src/lib/dashboard/widgets.rs` | Widget enum + builders for each panel type (text, logs, xy_chart, table, alert_chart, scorecard) |
| `src/lib/dashboard/layout.rs` | `Row` and `Grid` types that arrange widgets into a 12-column mosaic |
| `src/lib/dashboard/render.rs` | Converts a `Grid` of widgets into the final YAML string |
| `src/lib/dashboard/health.rs` | Component health policy creation (extracted from current `create_component_health_policies`) |
| `src/lib/dashboard/panels.rs` | High-level panel constructors that combine widgets into semantic groups (throughput row, detection row, health row, etc.) |

---

### Task 1: Create dashboard module directory and migrate existing tests

**Files:**
- Create: `src/lib/dashboard/mod.rs`
- Create: `src/lib/dashboard/widgets.rs`
- Create: `src/lib/dashboard/layout.rs`
- Create: `src/lib/dashboard/render.rs`
- Create: `src/lib/dashboard/health.rs`
- Create: `src/lib/dashboard/panels.rs`
- Delete: `src/lib/dashboard.rs`
- Modify: `src/lib/mod.rs`

- [ ] **Step 1: Create the module directory structure with stub files**

Create `src/lib/dashboard/` directory. Create `mod.rs` that re-exports the existing public API surface exactly as-is (so callers don't break). Move the entire content of `dashboard.rs` into `mod.rs` initially.

```rust
// src/lib/dashboard/mod.rs — initially identical to old dashboard.rs
// We'll extract pieces in subsequent tasks.

mod widgets;
mod layout;
mod render;
mod health;
mod panels;

// ... existing dashboard.rs content ...
```

The other files start as empty stubs:
```rust
// src/lib/dashboard/widgets.rs
// src/lib/dashboard/layout.rs
// src/lib/dashboard/render.rs
// src/lib/dashboard/health.rs
// src/lib/dashboard/panels.rs
```

- [ ] **Step 2: Update `src/lib/mod.rs` to use the directory module**

Change `pub mod dashboard;` — no change needed since Rust resolves `dashboard/mod.rs` automatically when the file `dashboard.rs` is removed and `dashboard/` exists.

- [ ] **Step 3: Run tests to verify nothing broke**

Run: `cargo test`
Expected: All 49 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/lib/dashboard/ src/lib/mod.rs
git rm src/lib/dashboard.rs
git commit -m "refactor(dashboard): convert to module directory (no behavior change)"
```

---

### Task 2: Implement the Widget type system

**Files:**
- Modify: `src/lib/dashboard/widgets.rs`

- [ ] **Step 1: Write tests for widget rendering**

```rust
// src/lib/dashboard/widgets.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_widget_renders_markdown() {
        let w = Widget::text("Resources", "Hello **world**");
        let yaml = w.to_yaml_value();
        let title = yaml["title"].as_str().unwrap();
        assert_eq!(title, "Resources");
        assert_eq!(yaml["text"]["format"].as_str().unwrap(), "MARKDOWN");
        assert!(yaml["text"]["content"].as_str().unwrap().contains("Hello **world**"));
    }

    #[test]
    fn logs_panel_widget_renders_filter_and_project() {
        let w = Widget::logs_panel("Live feed", "severity>=WARNING", "my-proj");
        let yaml = w.to_yaml_value();
        assert_eq!(yaml["title"].as_str().unwrap(), "Live feed");
        assert!(yaml["logsPanel"]["filter"].as_str().unwrap().contains("severity>=WARNING"));
        assert!(yaml["logsPanel"]["resourceNames"][0].as_str().unwrap().contains("my-proj"));
    }

    #[test]
    fn xy_chart_widget_renders_datasets() {
        let ds = DataSet {
            filter: "metric.type=\"custom/foo\"".into(),
            alignment_period: "60s".into(),
            aligner: "ALIGN_RATE".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        };
        let w = Widget::xy_chart("Rate", vec![ds]);
        let yaml = w.to_yaml_value();
        assert_eq!(yaml["title"].as_str().unwrap(), "Rate");
        assert!(yaml["xyChart"]["dataSets"][0]["timeSeriesQuery"]["timeSeriesFilter"]["filter"]
            .as_str().unwrap().contains("custom/foo"));
    }

    #[test]
    fn time_series_table_widget_renders() {
        let ds = DataSet {
            filter: "metric.type=\"custom/bar\"".into(),
            alignment_period: "3600s".into(),
            aligner: "ALIGN_SUM".into(),
            reducer: Some("REDUCE_SUM".into()),
            group_by: vec!["metric.label.rule_name".into()],
            plot_type: PlotType::Line,
        };
        let w = Widget::time_series_table("Top rules", vec![ds]);
        let yaml = w.to_yaml_value();
        assert_eq!(yaml["title"].as_str().unwrap(), "Top rules");
        assert!(yaml["timeSeriesTable"].is_mapping());
    }

    #[test]
    fn alert_chart_widget_renders_policy_name() {
        let w = Widget::alert_chart("Cloud Run", "projects/p/alertPolicies/abc");
        let yaml = w.to_yaml_value();
        assert_eq!(yaml["title"].as_str().unwrap(), "Cloud Run");
        assert_eq!(yaml["alertChart"]["name"].as_str().unwrap(), "projects/p/alertPolicies/abc");
    }

    #[test]
    fn scorecard_widget_renders_threshold() {
        let ds = DataSet {
            filter: "metric.type=\"custom/latency\"".into(),
            alignment_period: "60s".into(),
            aligner: "ALIGN_PERCENTILE_99".into(),
            reducer: None,
            group_by: vec![],
            plot_type: PlotType::Line,
        };
        let w = Widget::scorecard("P99 Latency", ds, Some(ThresholdConfig {
            value: 5000.0,
            color: ThresholdColor::Red,
            direction: ThresholdDirection::Above,
        }));
        let yaml = w.to_yaml_value();
        assert_eq!(yaml["title"].as_str().unwrap(), "P99 Latency");
        assert!(yaml["scorecard"].is_mapping());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dashboard::widgets`
Expected: FAIL — types don't exist yet.

- [ ] **Step 3: Implement the Widget type system**

```rust
// src/lib/dashboard/widgets.rs
use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone)]
pub enum PlotType {
    Line,
    StackedBar,
    StackedArea,
}

impl PlotType {
    fn as_str(&self) -> &str {
        match self {
            PlotType::Line => "LINE",
            PlotType::StackedBar => "STACKED_BAR",
            PlotType::StackedArea => "STACKED_AREA",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataSet {
    pub filter: String,
    pub alignment_period: String,
    pub aligner: String,
    pub reducer: Option<String>,
    pub group_by: Vec<String>,
    pub plot_type: PlotType,
}

#[derive(Debug, Clone)]
pub enum ThresholdColor {
    Red,
    Yellow,
}

impl ThresholdColor {
    fn as_str(&self) -> &str {
        match self {
            ThresholdColor::Red => "RED",
            ThresholdColor::Yellow => "YELLOW",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThresholdDirection {
    Above,
    Below,
}

impl ThresholdDirection {
    fn as_str(&self) -> &str {
        match self {
            ThresholdDirection::Above => "ABOVE",
            ThresholdDirection::Below => "BELOW",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThresholdConfig {
    pub value: f64,
    pub color: ThresholdColor,
    pub direction: ThresholdDirection,
}

#[derive(Debug, Clone)]
pub enum Widget {
    Text { title: String, content: String },
    LogsPanel { title: String, filter: String, project: String },
    XyChart { title: String, datasets: Vec<DataSet> },
    TimeSeriesTable { title: String, datasets: Vec<DataSet> },
    AlertChart { title: String, policy_name: String },
    Scorecard { title: String, dataset: DataSet, threshold: Option<ThresholdConfig> },
}

impl Widget {
    pub fn text(title: &str, content: &str) -> Self {
        Widget::Text { title: title.into(), content: content.into() }
    }

    pub fn logs_panel(title: &str, filter: &str, project: &str) -> Self {
        Widget::LogsPanel { title: title.into(), filter: filter.into(), project: project.into() }
    }

    pub fn xy_chart(title: &str, datasets: Vec<DataSet>) -> Self {
        Widget::XyChart { title: title.into(), datasets }
    }

    pub fn time_series_table(title: &str, datasets: Vec<DataSet>) -> Self {
        Widget::TimeSeriesTable { title: title.into(), datasets }
    }

    pub fn alert_chart(title: &str, policy_name: &str) -> Self {
        Widget::AlertChart { title: title.into(), policy_name: policy_name.into() }
    }

    pub fn scorecard(title: &str, dataset: DataSet, threshold: Option<ThresholdConfig>) -> Self {
        Widget::Scorecard { title: title.into(), dataset, threshold }
    }

    pub fn to_yaml_value(&self) -> Value {
        let mut map = Mapping::new();
        match self {
            Widget::Text { title, content } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut text_map = Mapping::new();
                text_map.insert(Value::from("content"), Value::from(content.as_str()));
                text_map.insert(Value::from("format"), Value::from("MARKDOWN"));
                map.insert(Value::from("text"), Value::Mapping(text_map));
            }
            Widget::LogsPanel { title, filter, project } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut panel = Mapping::new();
                panel.insert(Value::from("filter"), Value::from(filter.as_str()));
                let resources = Value::Sequence(vec![
                    Value::from(format!("projects/{}", project)),
                ]);
                panel.insert(Value::from("resourceNames"), resources);
                map.insert(Value::from("logsPanel"), Value::Mapping(panel));
            }
            Widget::XyChart { title, datasets } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut chart = Mapping::new();
                let ds_list: Vec<Value> = datasets.iter().map(|ds| Self::dataset_to_value(ds)).collect();
                chart.insert(Value::from("dataSets"), Value::Sequence(ds_list));
                map.insert(Value::from("xyChart"), Value::Mapping(chart));
            }
            Widget::TimeSeriesTable { title, datasets } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut table = Mapping::new();
                let ds_list: Vec<Value> = datasets.iter().map(|ds| Self::dataset_to_value(ds)).collect();
                table.insert(Value::from("dataSets"), Value::Sequence(ds_list));
                map.insert(Value::from("timeSeriesTable"), Value::Mapping(table));
            }
            Widget::AlertChart { title, policy_name } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut alert = Mapping::new();
                alert.insert(Value::from("name"), Value::from(policy_name.as_str()));
                map.insert(Value::from("alertChart"), Value::Mapping(alert));
            }
            Widget::Scorecard { title, dataset, threshold } => {
                map.insert(Value::from("title"), Value::from(title.as_str()));
                let mut sc = Mapping::new();
                let mut tsq = Mapping::new();
                let mut tsf = Mapping::new();
                tsf.insert(Value::from("filter"), Value::from(dataset.filter.as_str()));
                let mut agg = Mapping::new();
                agg.insert(Value::from("alignmentPeriod"), Value::from(dataset.alignment_period.as_str()));
                agg.insert(Value::from("perSeriesAligner"), Value::from(dataset.aligner.as_str()));
                tsf.insert(Value::from("aggregation"), Value::Mapping(agg));
                tsq.insert(Value::from("timeSeriesFilter"), Value::Mapping(tsf));
                sc.insert(Value::from("timeSeriesQuery"), Value::Mapping(tsq));
                if let Some(t) = threshold {
                    let mut thresholds = Vec::new();
                    let mut th = Mapping::new();
                    th.insert(Value::from("value"), Value::from(t.value));
                    th.insert(Value::from("color"), Value::from(t.color.as_str()));
                    th.insert(Value::from("direction"), Value::from(t.direction.as_str()));
                    thresholds.push(Value::Mapping(th));
                    sc.insert(Value::from("thresholds"), Value::Sequence(thresholds));
                }
                map.insert(Value::from("scorecard"), Value::Mapping(sc));
            }
        }
        Value::Mapping(map)
    }

    fn dataset_to_value(ds: &DataSet) -> Value {
        let mut dataset_map = Mapping::new();
        let mut tsq = Mapping::new();
        let mut tsf = Mapping::new();
        tsf.insert(Value::from("filter"), Value::from(ds.filter.as_str()));
        let mut agg = Mapping::new();
        agg.insert(Value::from("alignmentPeriod"), Value::from(ds.alignment_period.as_str()));
        agg.insert(Value::from("perSeriesAligner"), Value::from(ds.aligner.as_str()));
        if let Some(ref reducer) = ds.reducer {
            agg.insert(Value::from("crossSeriesReducer"), Value::from(reducer.as_str()));
        }
        if !ds.group_by.is_empty() {
            let groups: Vec<Value> = ds.group_by.iter().map(|g| Value::from(g.as_str())).collect();
            agg.insert(Value::from("groupByFields"), Value::Sequence(groups));
        }
        tsf.insert(Value::from("aggregation"), Value::Mapping(agg));
        tsq.insert(Value::from("timeSeriesFilter"), Value::Mapping(tsf));
        dataset_map.insert(Value::from("timeSeriesQuery"), Value::Mapping(tsq));
        dataset_map.insert(Value::from("plotType"), Value::from(ds.plot_type.as_str()));
        Value::Mapping(dataset_map)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dashboard::widgets`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/widgets.rs
git commit -m "feat(dashboard): add typed Widget system with YAML serialization"
```

---

### Task 3: Implement the layout engine

**Files:**
- Modify: `src/lib/dashboard/layout.rs`

- [ ] **Step 1: Write tests for the layout engine**

```rust
// src/lib/dashboard/layout.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::dashboard::widgets::Widget;

    #[test]
    fn single_full_width_tile() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(4).full(Widget::text("Title", "hi")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].x, 0);
        assert_eq!(tiles[0].y, 0);
        assert_eq!(tiles[0].width, 12);
        assert_eq!(tiles[0].height, 4);
    }

    #[test]
    fn two_half_width_tiles_same_row() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(4)
            .half(Widget::text("Left", "l"))
            .half(Widget::text("Right", "r")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].x, 0);
        assert_eq!(tiles[0].width, 6);
        assert_eq!(tiles[1].x, 6);
        assert_eq!(tiles[1].width, 6);
        assert_eq!(tiles[0].y, tiles[1].y);
    }

    #[test]
    fn rows_stack_vertically() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(3).full(Widget::text("R1", "a")));
        grid.add_row(Row::new(4).full(Widget::text("R2", "b")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles[0].y, 0);
        assert_eq!(tiles[0].height, 3);
        assert_eq!(tiles[1].y, 3);
        assert_eq!(tiles[1].height, 4);
    }

    #[test]
    fn grid_of_alert_charts() {
        let widgets: Vec<Widget> = (0..5)
            .map(|i| Widget::alert_chart(&format!("W{}", i), &format!("policy/{}", i)))
            .collect();
        let mut grid = Grid::new();
        grid.add_row(Row::new(3).grid(widgets, 4));
        let tiles = grid.to_tiles();
        // 5 widgets at 4-per-row = 2 internal rows
        assert_eq!(tiles.len(), 5);
        // First 4 in row 0, last 1 in row 1
        assert_eq!(tiles[0].y, 0);
        assert_eq!(tiles[3].y, 0);
        assert_eq!(tiles[4].y, 3); // second sub-row
        assert_eq!(tiles[0].width, 3); // 12/4
    }

    #[test]
    fn empty_grid_produces_no_tiles() {
        let grid = Grid::new();
        assert!(grid.to_tiles().is_empty());
    }

    #[test]
    fn third_width_tiles() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(3)
            .span(4, Widget::text("A", ""))
            .span(4, Widget::text("B", ""))
            .span(4, Widget::text("C", "")));
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].width, 4);
        assert_eq!(tiles[1].x, 4);
        assert_eq!(tiles[2].x, 8);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dashboard::layout`
Expected: FAIL — types don't exist.

- [ ] **Step 3: Implement the layout engine**

```rust
// src/lib/dashboard/layout.rs
use crate::lib::dashboard::widgets::Widget;

#[derive(Debug, Clone)]
pub struct Tile {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub widget: Widget,
}

#[derive(Debug, Clone)]
enum RowEntry {
    Span { cols: usize, widget: Widget },
    Grid { widgets: Vec<Widget>, cols_per_row: usize },
}

#[derive(Debug, Clone)]
pub struct Row {
    height: usize,
    entries: Vec<RowEntry>,
}

impl Row {
    pub fn new(height: usize) -> Self {
        Row { height, entries: Vec::new() }
    }

    pub fn full(mut self, widget: Widget) -> Self {
        self.entries.push(RowEntry::Span { cols: 12, widget });
        self
    }

    pub fn half(mut self, widget: Widget) -> Self {
        self.entries.push(RowEntry::Span { cols: 6, widget });
        self
    }

    pub fn span(mut self, cols: usize, widget: Widget) -> Self {
        self.entries.push(RowEntry::Span { cols, widget });
        self
    }

    pub fn grid(mut self, widgets: Vec<Widget>, cols_per_row: usize) -> Self {
        self.entries.push(RowEntry::Grid { widgets, cols_per_row });
        self
    }
}

#[derive(Debug, Clone)]
pub struct Grid {
    rows: Vec<Row>,
}

impl Grid {
    pub fn new() -> Self {
        Grid { rows: Vec::new() }
    }

    pub fn add_row(&mut self, row: Row) {
        self.rows.push(row);
    }

    pub fn to_tiles(&self) -> Vec<Tile> {
        let mut tiles = Vec::new();
        let mut y_offset: usize = 0;

        for row in &self.rows {
            let mut row_max_height: usize = row.height;
            let mut x_offset: usize = 0;

            for entry in &row.entries {
                match entry {
                    RowEntry::Span { cols, widget } => {
                        tiles.push(Tile {
                            x: x_offset,
                            y: y_offset,
                            width: *cols,
                            height: row.height,
                            widget: widget.clone(),
                        });
                        x_offset += cols;
                    }
                    RowEntry::Grid { widgets, cols_per_row } => {
                        let cell_w = 12 / cols_per_row;
                        let num_sub_rows = widgets.len().div_ceil(*cols_per_row);
                        for (i, widget) in widgets.iter().enumerate() {
                            let gx = (i % cols_per_row) * cell_w;
                            let gy = y_offset + (i / cols_per_row) * row.height;
                            tiles.push(Tile {
                                x: gx,
                                y: gy,
                                width: cell_w,
                                height: row.height,
                                widget: widget.clone(),
                            });
                        }
                        row_max_height = num_sub_rows * row.height;
                    }
                }
            }
            y_offset += row_max_height;
        }
        tiles
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dashboard::layout`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/layout.rs
git commit -m "feat(dashboard): add grid layout engine for widget placement"
```

---

### Task 4: Implement the YAML renderer

**Files:**
- Modify: `src/lib/dashboard/render.rs`

- [ ] **Step 1: Write tests for the renderer**

```rust
// src/lib/dashboard/render.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::dashboard::widgets::Widget;
    use crate::lib::dashboard::layout::{Grid, Row};

    #[test]
    fn render_produces_valid_yaml_with_display_name() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(3).full(Widget::text("Hello", "World")));
        let yaml = render_dashboard_yaml("My Dashboard", &grid);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
        assert_eq!(parsed["displayName"].as_str().unwrap(), "My Dashboard");
        assert_eq!(parsed["mosaicLayout"]["columns"].as_u64().unwrap(), 12);
        let tiles = parsed["mosaicLayout"]["tiles"].as_sequence().unwrap();
        assert_eq!(tiles.len(), 1);
    }

    #[test]
    fn tiles_have_correct_position_fields() {
        let mut grid = Grid::new();
        grid.add_row(Row::new(4)
            .half(Widget::text("A", ""))
            .half(Widget::text("B", "")));
        let yaml = render_dashboard_yaml("Test", &grid);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        let tiles = parsed["mosaicLayout"]["tiles"].as_sequence().unwrap();
        assert_eq!(tiles[0]["xPos"].as_u64().unwrap(), 0);
        assert_eq!(tiles[0]["width"].as_u64().unwrap(), 6);
        assert_eq!(tiles[1]["xPos"].as_u64().unwrap(), 6);
    }

    #[test]
    fn empty_grid_renders_minimal_dashboard() {
        let grid = Grid::new();
        let yaml = render_dashboard_yaml("Empty", &grid);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed["displayName"].as_str().unwrap(), "Empty");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dashboard::render`
Expected: FAIL.

- [ ] **Step 3: Implement the renderer**

```rust
// src/lib/dashboard/render.rs
use serde_yaml::{Mapping, Value};

use crate::lib::dashboard::layout::{Grid, Tile};

pub fn render_dashboard_yaml(display_name: &str, grid: &Grid) -> String {
    let tiles = grid.to_tiles();

    let mut root = Mapping::new();
    root.insert(Value::from("displayName"), Value::from(display_name));

    let mut mosaic = Mapping::new();
    mosaic.insert(Value::from("columns"), Value::from(12u64));

    let tile_values: Vec<Value> = tiles.iter().map(|t| tile_to_value(t)).collect();
    mosaic.insert(Value::from("tiles"), Value::Sequence(tile_values));

    root.insert(Value::from("mosaicLayout"), Value::Mapping(mosaic));

    serde_yaml::to_string(&Value::Mapping(root)).unwrap_or_default()
}

fn tile_to_value(tile: &Tile) -> Value {
    let mut map = Mapping::new();
    if tile.x > 0 {
        map.insert(Value::from("xPos"), Value::from(tile.x as u64));
    }
    if tile.y > 0 {
        map.insert(Value::from("yPos"), Value::from(tile.y as u64));
    }
    map.insert(Value::from("width"), Value::from(tile.width as u64));
    map.insert(Value::from("height"), Value::from(tile.height as u64));
    map.insert(Value::from("widget"), tile.widget.to_yaml_value());
    Value::Mapping(map)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dashboard::render`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/render.rs
git commit -m "feat(dashboard): add YAML renderer from Grid to Cloud Monitoring spec"
```

---

### Task 5: Implement the panels module with new dashboard content

**Files:**
- Modify: `src/lib/dashboard/panels.rs`

- [ ] **Step 1: Write tests for panel builders**

```rust
// src/lib/dashboard/panels.rs
#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> PanelContext {
        PanelContext {
            project: "test-proj".into(),
            region: "us-east1".into(),
            metric_name: "beaver_detection_count_abc".into(),
            vector_service: "beaver-vector-xyz".into(),
            dataflow_job: "beaver-detections-xyz".into(),
            input_subscription: "input-sub".into(),
            output_topic: "beaver_outtopic".into(),
            bq_subscription: "beaver_bqsub".into(),
            dataflow_subscription: "beaver_dfsub".into(),
            bq_dataset: "beaver_dataset".into(),
            bq_table: "events".into(),
            bucket: "beaver_bkt".into(),
            vector_sa: "vector-sa@test.iam".into(),
            dataflow_sa: "df-sa@test.iam".into(),
            alerts_topic: "beaver-alerts".into(),
            dlq_topic: "beaver-dlq".into(),
        }
    }

    #[test]
    fn resource_links_row_contains_all_resources() {
        let ctx = test_ctx();
        let row = resource_links_row(&ctx);
        let grid = {
            let mut g = crate::lib::dashboard::layout::Grid::new();
            g.add_row(row);
            g
        };
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 1);
    }

    #[test]
    fn throughput_row_produces_two_panels() {
        let ctx = test_ctx();
        let row = throughput_row(&ctx);
        let grid = {
            let mut g = crate::lib::dashboard::layout::Grid::new();
            g.add_row(row);
            g
        };
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 2);
    }

    #[test]
    fn detection_row_produces_three_panels() {
        let ctx = test_ctx();
        let row = detection_row(&ctx);
        let grid = {
            let mut g = crate::lib::dashboard::layout::Grid::new();
            g.add_row(row);
            g
        };
        let tiles = grid.to_tiles();
        assert_eq!(tiles.len(), 3);
    }

    #[test]
    fn full_dashboard_grid_is_valid_yaml() {
        let ctx = test_ctx();
        let health = vec![
            ("Cloud Run".into(), "projects/p/alertPolicies/cr".into()),
            ("Dataflow".into(), "projects/p/alertPolicies/df".into()),
        ];
        let grid = build_dashboard_grid(&ctx, &health, &[]);
        let yaml = crate::lib::dashboard::render::render_dashboard_yaml("Test", &grid);
        let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
        assert!(yaml.contains("Test"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test dashboard::panels`
Expected: FAIL.

- [ ] **Step 3: Implement the panels module**

```rust
// src/lib/dashboard/panels.rs
use crate::lib::dashboard::widgets::{DataSet, PlotType, Widget};
use crate::lib::dashboard::layout::{Grid, Row};

#[derive(Debug, Clone)]
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

pub fn resource_links_row(ctx: &PanelContext) -> Row {
    let links = format!(
        "[Cloud Run](https://console.cloud.google.com/run/detail/{region}/{vector_service}/metrics?project={project}) · \
         [Dataflow](https://console.cloud.google.com/dataflow/jobs?project={project}) · \
         [BigQuery](https://console.cloud.google.com/bigquery?project={project}&d={bq_dataset}&t={bq_table}&page=table) · \
         [Output topic](https://console.cloud.google.com/cloudpubsub/topic/detail/{output_topic}?project={project}) · \
         [BQ sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{bq_subscription}?project={project}) · \
         [DF sub](https://console.cloud.google.com/cloudpubsub/subscription/detail/{dataflow_subscription}?project={project}) · \
         [Bucket](https://console.cloud.google.com/storage/browser/{bucket}?project={project}) · \
         [Logs](https://console.cloud.google.com/logs/query?project={project})",
        region = ctx.region,
        vector_service = ctx.vector_service,
        project = ctx.project,
        bq_dataset = ctx.bq_dataset,
        bq_table = ctx.bq_table,
        output_topic = ctx.output_topic,
        bq_subscription = ctx.bq_subscription,
        dataflow_subscription = ctx.dataflow_subscription,
        bucket = ctx.bucket,
    );
    Row::new(2).full(Widget::text("Resources", &links))
}

pub fn throughput_row(ctx: &PanelContext) -> Row {
    let input_ds = DataSet {
        filter: format!(
            r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/push_request_count" AND resource.label.subscription_id="{}""#,
            ctx.input_subscription
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_RATE".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    let output_ds = DataSet {
        filter: format!(
            r#"resource.type="pubsub_topic" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count" AND resource.label.topic_id="{}""#,
            ctx.output_topic
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_RATE".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    Row::new(4)
        .half(Widget::xy_chart("Ingestion rate (msgs/sec)", vec![input_ds]))
        .half(Widget::xy_chart("Output rate (msgs/sec)", vec![output_ds]))
}

pub fn detection_row(ctx: &PanelContext) -> Row {
    let metric_type = format!("logging.googleapis.com/user/{}", ctx.metric_name);
    let table_ds = DataSet {
        filter: format!(r#"metric.type="{}""#, metric_type),
        alignment_period: "3600s".into(),
        aligner: "ALIGN_SUM".into(),
        reducer: Some("REDUCE_SUM".into()),
        group_by: vec!["metric.label.rule_name".into()],
        plot_type: PlotType::Line,
    };
    let rate_ds = DataSet {
        filter: format!(r#"metric.type="{}""#, metric_type),
        alignment_period: "60s".into(),
        aligner: "ALIGN_RATE".into(),
        reducer: Some("REDUCE_SUM".into()),
        group_by: vec!["metric.label.rule_name".into()],
        plot_type: PlotType::StackedArea,
    };
    let alerts_ds = DataSet {
        filter: format!(
            r#"resource.type="pubsub_topic" AND resource.labels.topic_id="{}" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count""#,
            ctx.alerts_topic
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_RATE".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    Row::new(4)
        .span(4, Widget::time_series_table("Top firing rules (1h)", vec![table_ds]))
        .span(4, Widget::xy_chart("Detection rate by rule", vec![rate_ds]))
        .span(4, Widget::xy_chart("Alerts / min (sigma_beam)", vec![alerts_ds]))
}

pub fn pipeline_health_row(ctx: &PanelContext) -> Row {
    let backlog_ds = DataSet {
        filter: format!(
            r#"resource.type="pubsub_subscription" AND metric.type="pubsub.googleapis.com/subscription/num_undelivered_messages" AND resource.label.subscription_id="{}" OR resource.label.subscription_id="{}""#,
            ctx.bq_subscription, ctx.dataflow_subscription
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_MEAN".into(),
        reducer: None,
        group_by: vec!["resource.label.subscription_id".into()],
        plot_type: PlotType::Line,
    };
    let dlq_ds = DataSet {
        filter: format!(
            r#"resource.type="pubsub_topic" AND resource.labels.topic_id="{}" AND metric.type="pubsub.googleapis.com/topic/send_message_operation_count""#,
            ctx.dlq_topic
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_RATE".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    Row::new(4)
        .half(Widget::xy_chart("Subscription backlogs", vec![backlog_ds]))
        .half(Widget::xy_chart("DLQ messages / min", vec![dlq_ds]))
}

pub fn logs_row(ctx: &PanelContext) -> Row {
    Row::new(4).full(Widget::logs_panel(
        "Detection events (live feed)",
        r#"resource.type="dataflow_step" AND jsonPayload.message:"BEAVER_SIEM_MATCH""#,
        &ctx.project,
    ))
}

pub fn worker_logs_row(ctx: &PanelContext) -> Row {
    Row::new(4).full(Widget::logs_panel(
        "Dataflow worker logs (warnings + errors)",
        &format!(
            r#"resource.type="dataflow_step" AND resource.labels.job_name="{}" AND severity>=WARNING"#,
            ctx.dataflow_job
        ),
        &ctx.project,
    ))
}

pub fn health_grid_row(component_health: &[(String, String)]) -> Row {
    let widgets: Vec<Widget> = component_health
        .iter()
        .map(|(label, policy)| Widget::alert_chart(label, policy))
        .collect();
    Row::new(3).grid(widgets, 4)
}

pub fn alerting_row(ctx: &PanelContext, alert_policies: &[String]) -> Option<Row> {
    if alert_policies.is_empty() {
        return None;
    }
    // Not a simple Row — we need two things: a logs panel + alert chart grid.
    // We'll return a Row with the notification delivery log panel.
    // The alert charts are handled separately.
    Some(Row::new(4).full(Widget::logs_panel(
        "Notification delivery (recent)",
        r#"resource.type="alerting_policy""#,
        &ctx.project,
    )))
}

pub fn alert_policies_grid_row(alert_policies: &[String]) -> Option<Row> {
    if alert_policies.is_empty() {
        return None;
    }
    let widgets: Vec<Widget> = alert_policies
        .iter()
        .map(|policy| {
            let short = policy.rsplit('/').next().unwrap_or(policy);
            Widget::alert_chart(&format!("Alert: {}", short), policy)
        })
        .collect();
    Some(Row::new(3).grid(widgets, 4))
}

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test dashboard::panels`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/panels.rs
git commit -m "feat(dashboard): add panel builders with throughput, detection, and health rows"
```

---

### Task 6: Extract health policies into health.rs

**Files:**
- Modify: `src/lib/dashboard/health.rs`
- Modify: `src/lib/dashboard/mod.rs`

- [ ] **Step 1: Move `create_component_health_policies`, `create_health_policy`, `noop_threshold`, `threshold_condition` from mod.rs into health.rs**

Move the functions verbatim. Update visibility to `pub(super)` where called only from mod.rs.

```rust
// src/lib/dashboard/health.rs
use std::process::Command;
use anyhow::{anyhow, Result};
use log::info;

use crate::lib::config::Config;
use crate::lib::resources::Tracker;

#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub label: String,
    pub policy_name: String,
}

pub fn create_component_health_policies(
    tracker: &mut Tracker,
    config: &Config,
    metric_name: &str,
) -> Result<Vec<ComponentHealth>> {
    // ... exact same implementation as current create_component_health_policies ...
    // (copy from mod.rs)
}

pub fn noop_threshold(filter: &str, value: f64) -> String {
    threshold_condition("no-op (deploy-time check)", filter, value, "ALIGN_MAX")
}

pub fn threshold_condition(label: &str, filter: &str, value: f64, aligner: &str) -> String {
    format!(
r#"  - displayName: "{label}"
    conditionThreshold:
      filter: '{filter}'
      comparison: COMPARISON_GT
      thresholdValue: {value}
      duration: 60s
      aggregations:
        - alignmentPeriod: 60s
          perSeriesAligner: {aligner}
"#)
}

pub fn create_health_policy(
    tracker: &mut Tracker,
    config: &Config,
    display: &str,
    conditions_yaml: &str,
) -> Result<String> {
    // ... exact same implementation as current create_health_policy ...
    // (copy from mod.rs)
}
```

- [ ] **Step 2: Update mod.rs to import from health.rs instead of defining inline**

Remove the moved functions from mod.rs. Add `use health::*;` or qualified paths.

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/lib/dashboard/health.rs src/lib/dashboard/mod.rs
git commit -m "refactor(dashboard): extract health policy logic to health.rs"
```

---

### Task 7: Wire up the new rendering pipeline in mod.rs

**Files:**
- Modify: `src/lib/dashboard/mod.rs`

- [ ] **Step 1: Replace `render_dashboard` with new pipeline**

Update `create_dashboard` to use the new `panels::build_dashboard_grid` + `render::render_dashboard_yaml` instead of the old `render_dashboard` function. The `DashboardContext` struct is replaced by `PanelContext`.

```rust
// In create_dashboard, replace:
//   let yaml = render_dashboard(&ctx);
// With:
    let panel_ctx = panels::PanelContext {
        project: config.project.clone(),
        region: config.region.clone(),
        metric_name: metric_name.to_string(),
        vector_service: res.crs_instance.clone(),
        dataflow_job: res.dataflow_pipeline_name.clone(),
        input_subscription: input_subscription.to_string(),
        output_topic: res.output_pubsub.topic_id.clone(),
        bq_subscription: res.output_pubsub.bq_subscription_id.clone(),
        dataflow_subscription: res.output_pubsub.subscription_id_2.clone(),
        bq_dataset: res.biq_query.dataset_id.clone(),
        bq_table: res.biq_query.table_id.clone(),
        bucket: res.bucket_name.clone(),
        vector_sa: res.vector_sa_email.clone(),
        dataflow_sa: res.dataflow_sa_email.clone(),
        alerts_topic: res.alerts_topic_id.clone(),
        dlq_topic: res.dlq_topic_id.clone(),
    };
    let health_pairs: Vec<(String, String)> = component_health
        .iter()
        .map(|c| (c.label.clone(), c.policy_name.clone()))
        .collect();
    let grid = panels::build_dashboard_grid(&panel_ctx, &health_pairs, &res.alert_policies);
    let yaml = render::render_dashboard_yaml(&display, &grid);
```

- [ ] **Step 2: Remove old `render_dashboard` function and `DashboardContext` struct**

Delete the `DashboardContext` struct and old `render_dashboard` function from mod.rs.

- [ ] **Step 3: Update existing tests to use new API**

The tests that used `DashboardContext` + `render_dashboard` should now construct a `PanelContext` + call `panels::build_dashboard_grid` + `render::render_dashboard_yaml`.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests pass (old + new).

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/
git commit -m "feat(dashboard): wire new widget/layout/render pipeline into create_dashboard"
```

---

### Task 8: Add new visualization panels — Dataflow latency and error budget

**Files:**
- Modify: `src/lib/dashboard/panels.rs`

- [ ] **Step 1: Write test for the new latency row**

```rust
#[test]
fn latency_row_produces_two_panels() {
    let ctx = test_ctx();
    let row = dataflow_latency_row(&ctx);
    let grid = {
        let mut g = crate::lib::dashboard::layout::Grid::new();
        g.add_row(row);
        g
    };
    let tiles = grid.to_tiles();
    assert_eq!(tiles.len(), 2);
}
```

- [ ] **Step 2: Implement `dataflow_latency_row`**

```rust
pub fn dataflow_latency_row(ctx: &PanelContext) -> Row {
    let system_lag_ds = DataSet {
        filter: format!(
            r#"resource.type="dataflow_job" AND metric.type="dataflow.googleapis.com/job/system_lag" AND resource.label.job_name="{}""#,
            ctx.dataflow_job
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_MAX".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    let freshness_ds = DataSet {
        filter: format!(
            r#"resource.type="dataflow_job" AND metric.type="dataflow.googleapis.com/job/data_watermark_age" AND resource.label.job_name="{}""#,
            ctx.dataflow_job
        ),
        alignment_period: "60s".into(),
        aligner: "ALIGN_MAX".into(),
        reducer: None,
        group_by: vec![],
        plot_type: PlotType::Line,
    };
    Row::new(4)
        .half(Widget::xy_chart("System lag (seconds)", vec![system_lag_ds]))
        .half(Widget::xy_chart("Data freshness (watermark age)", vec![freshness_ds]))
}
```

- [ ] **Step 3: Add to `build_dashboard_grid` between pipeline_health_row and logs_row**

```rust
    grid.add_row(dataflow_latency_row(ctx));
```

- [ ] **Step 4: Run tests**

Run: `cargo test dashboard`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add src/lib/dashboard/panels.rs
git commit -m "feat(dashboard): add Dataflow latency + freshness panels"
```

---

### Task 9: Final integration test — full round-trip YAML validation

**Files:**
- Modify: `src/lib/dashboard/mod.rs` (test section)

- [ ] **Step 1: Write comprehensive integration test**

```rust
#[test]
fn full_dashboard_round_trip_produces_valid_yaml_with_all_sections() {
    let health = vec![
        ("Cloud Run".to_string(), "projects/p/alertPolicies/cr".to_string()),
        ("Dataflow".to_string(), "projects/p/alertPolicies/df".to_string()),
        ("BigQuery".to_string(), "projects/p/alertPolicies/bq".to_string()),
        ("GCS bucket".to_string(), "projects/p/alertPolicies/gcs".to_string()),
    ];
    let policies = vec![
        "projects/p/alertPolicies/route-1".to_string(),
        "projects/p/alertPolicies/route-2".to_string(),
    ];
    let ctx = panels::PanelContext {
        project: "my-proj".into(),
        region: "us-east1".into(),
        metric_name: "beaver_detection_count_abc".into(),
        vector_service: "beaver-vector-xyz".into(),
        dataflow_job: "beaver-detections-xyz".into(),
        input_subscription: "input-sub".into(),
        output_topic: "beaver_outtopic".into(),
        bq_subscription: "beaver_bqsub".into(),
        dataflow_subscription: "beaver_dfsub".into(),
        bq_dataset: "beaver_dataset".into(),
        bq_table: "events".into(),
        bucket: "beaver_bkt".into(),
        vector_sa: "vector-sa@test.iam".into(),
        dataflow_sa: "df-sa@test.iam".into(),
        alerts_topic: "beaver-alerts".into(),
        dlq_topic: "beaver-dlq".into(),
    };
    let grid = panels::build_dashboard_grid(&ctx, &health, &policies);
    let yaml = render::render_dashboard_yaml("Beaver SIEM", &grid);

    // Must be valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    assert_eq!(parsed["displayName"].as_str().unwrap(), "Beaver SIEM");

    // Must contain key widget types
    assert!(yaml.contains("logsPanel:"));
    assert!(yaml.contains("xyChart:"));
    assert!(yaml.contains("timeSeriesTable:"));
    assert!(yaml.contains("alertChart:"));

    // Must reference all key resources
    assert!(yaml.contains("beaver-detections-xyz"));
    assert!(yaml.contains("beaver_outtopic"));
    assert!(yaml.contains("beaver_bqsub"));
    assert!(yaml.contains("beaver-alerts"));
    assert!(yaml.contains("beaver-dlq"));

    // Notification section present when policies exist
    assert!(yaml.contains("Notification delivery"));
    assert!(yaml.contains("Alert: route-1"));
    assert!(yaml.contains("Alert: route-2"));

    // Health grid: 4 components
    let alert_chart_count = yaml.matches("alertChart:").count();
    // 4 health + 2 notification alert charts = 6
    assert_eq!(alert_chart_count, 6);
}

#[test]
fn full_dashboard_without_notifications_omits_alerting_section() {
    let health = vec![
        ("Cloud Run".to_string(), "projects/p/alertPolicies/cr".to_string()),
    ];
    let ctx = panels::PanelContext {
        project: "my-proj".into(),
        region: "us-east1".into(),
        metric_name: "m".into(),
        vector_service: "v".into(),
        dataflow_job: "d".into(),
        input_subscription: "in".into(),
        output_topic: "t".into(),
        bq_subscription: "s1".into(),
        dataflow_subscription: "s2".into(),
        bq_dataset: "ds".into(),
        bq_table: "tbl".into(),
        bucket: "b".into(),
        vector_sa: "v@x".into(),
        dataflow_sa: "d@x".into(),
        alerts_topic: "beaver-alerts".into(),
        dlq_topic: "beaver-dlq".into(),
    };
    let grid = panels::build_dashboard_grid(&ctx, &health, &[]);
    let yaml = render::render_dashboard_yaml("Test", &grid);
    let _: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("valid yaml");
    assert!(!yaml.contains("Notification delivery"));
    // Only 1 health alertChart
    assert_eq!(yaml.matches("alertChart:").count(), 1);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/lib/dashboard/mod.rs
git commit -m "test(dashboard): add full round-trip integration tests for new dashboard"
```

---

## Summary

The refactored dashboard module provides:
1. **Composable widgets** — each widget type is independently testable and renderable
2. **Grid layout engine** — automatic positioning without manual yPos math
3. **Structured YAML** — uses serde_yaml Values instead of format! strings (catches structural errors at compile time)
4. **New panels** — throughput (input/output rates), Dataflow latency + freshness, subscription backlogs, DLQ monitoring
5. **Clean separation** — widgets, layout, rendering, health policies, and panel composition are all independent modules
