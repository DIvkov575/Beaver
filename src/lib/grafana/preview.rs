//! Local Docker-based Grafana preview and dashboard export.
//!
//! `run_preview` builds a self-contained Grafana container with synthetic data
//! in SQLite so analysts can explore the Beaver SIEM dashboard without a GCP
//! project. `export_dashboard` writes the production-ready dashboard JSON
//! (referencing `${DS_BEAVER}` BigQuery datasource variable) to disk.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use log::info;

use super::dashboard::GrafanaDashboardBuilder;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Launches a local Grafana container with synthetic SIEM data pre-loaded into
/// SQLite. The container runs in the foreground (Ctrl-C to stop).
pub fn run_preview(config_path: &Path) -> Result<()> {
    let title = load_dashboard_title(config_path);
    info!("generating preview dashboard: {}", title);

    let dashboard_value = GrafanaDashboardBuilder::build_preview(&title);
    let dashboard_json = serde_json::to_string_pretty(&dashboard_value)?;

    // Prepare build context in a temp directory
    let tmp = tempfile::tempdir()?;
    let base = tmp.path();

    // provisioning/datasources/
    let ds_dir = base.join("provisioning").join("datasources");
    fs::create_dir_all(&ds_dir)?;
    fs::write(ds_dir.join("sqlite.yaml"), SQLITE_DATASOURCE_YAML)?;

    // provisioning/dashboards/
    let dash_dir = base.join("provisioning").join("dashboards");
    fs::create_dir_all(&dash_dir)?;
    fs::write(dash_dir.join("dashboard.yaml"), DASHBOARD_PROVISIONING_YAML)?;
    fs::write(dash_dir.join("beaver-siem.json"), &dashboard_json)?;

    // grafana.ini
    fs::write(base.join("grafana.ini"), GRAFANA_INI)?;

    // synthetic.sql
    fs::write(base.join("synthetic.sql"), generate_synthetic_sql())?;

    // entrypoint.sh
    fs::write(base.join("entrypoint.sh"), ENTRYPOINT_SH)?;

    // Dockerfile
    fs::write(base.join("Dockerfile"), DOCKERFILE)?;

    // Check Docker availability
    check_docker()?;

    // Build image
    info!("building preview image...");
    let build_out = Command::new("docker")
        .args(["build", "-t", "beaver-grafana-preview", "."])
        .current_dir(base)
        .output()?;
    if !build_out.status.success() {
        return Err(anyhow!(
            "docker build failed: {}",
            String::from_utf8_lossy(&build_out.stderr)
        ));
    }

    println!("Dashboard available at http://localhost:3000 (Ctrl-C to stop)");

    // Run container in foreground (inherits stdio so user can Ctrl-C)
    let status = Command::new("docker")
        .args(["run", "--rm", "-p", "3000:3000", "beaver-grafana-preview"])
        .status()?;

    if !status.success() {
        return Err(anyhow!("docker run exited with status: {}", status));
    }
    Ok(())
}

/// Generates the production Grafana dashboard JSON (with `${DS_BEAVER}`
/// datasource variable) and writes it to `output_path`.
pub fn export_dashboard(config_path: &Path, output_path: &Path) -> Result<()> {
    let title = load_dashboard_title(config_path);
    info!("exporting production dashboard: {}", title);

    let builder = GrafanaDashboardBuilder {
        project: String::new(),
        dataset: String::new(),
        table: "events".to_string(),
        title: title.clone(),
    };
    let dashboard_value = builder.build();
    let dashboard_json = serde_json::to_string_pretty(&dashboard_value)?;

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, &dashboard_json)?;

    println!("Dashboard written to {}", output_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reads the dashboard title from config, falling back to a sensible default.
fn load_dashboard_title(config_path: &Path) -> String {
    let config_file = config_path.join("beaver_config.yaml");
    if !config_file.exists() {
        return "Beaver SIEM".to_string();
    }
    // Attempt to parse; on any failure use default
    let Ok(file) = std::fs::File::open(&config_file) else {
        return "Beaver SIEM".to_string();
    };
    let Ok(mapping): Result<serde_yaml::Mapping, _> = serde_yaml::from_reader(file) else {
        return "Beaver SIEM".to_string();
    };
    mapping
        .get(&serde_yaml::Value::String("grafana".into()))
        .and_then(|g| g.get(&serde_yaml::Value::String("title".into())))
        .and_then(|v| v.as_str())
        .unwrap_or("Beaver SIEM")
        .to_string()
}

fn check_docker() -> Result<()> {
    let out = Command::new("docker")
        .arg("--version")
        .output()
        .map_err(|_| anyhow!("docker not found in PATH -- install Docker to use preview"))?;
    if !out.status.success() {
        return Err(anyhow!("docker --version failed"));
    }
    Ok(())
}

/// Generates SQL statements to create the events table and insert synthetic
/// data from the bundled fixture.
fn generate_synthetic_sql() -> String {
    let events: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/synthetic_events.json"))
            .expect("bundled synthetic_events.json must be valid");

    let mut sql = String::from(
        "CREATE TABLE IF NOT EXISTS events (\n\
         \x20 timestamp TEXT,\n\
         \x20 rule_name TEXT,\n\
         \x20 severity TEXT,\n\
         \x20 mitre_tactic TEXT,\n\
         \x20 mitre_technique TEXT,\n\
         \x20 source_ip TEXT,\n\
         \x20 destination_ip TEXT,\n\
         \x20 hostname TEXT,\n\
         \x20 raw_event TEXT\n\
         );\n\n",
    );

    for event in &events {
        let ts = event["timestamp"].as_str().unwrap_or("");
        let rule = event["rule_name"].as_str().unwrap_or("");
        let sev = event["severity"].as_str().unwrap_or("");
        let tactic = event["mitre_tactic"].as_str().unwrap_or("");
        let technique = event["mitre_technique"].as_str().unwrap_or("");
        let src = event["source_ip"].as_str().unwrap_or("");
        let dst = event["destination_ip"].as_str().unwrap_or("");
        let host = event["hostname"].as_str().unwrap_or("");
        let raw = event["raw_event"].as_str().unwrap_or("{}");
        // Escape single quotes for SQL
        let raw_escaped = raw.replace('\'', "''");
        sql.push_str(&format!(
            "INSERT INTO events VALUES ('{ts}','{rule}','{sev}','{tactic}','{technique}','{src}','{dst}','{host}','{raw}');\n",
            ts = ts, rule = rule, sev = sev, tactic = tactic,
            technique = technique, src = src, dst = dst, host = host, raw = raw_escaped
        ));
    }

    sql
}

// ---------------------------------------------------------------------------
// Static file contents
// ---------------------------------------------------------------------------

const SQLITE_DATASOURCE_YAML: &str = r#"apiVersion: 1
datasources:
  - name: DS_BEAVER
    type: frser-sqlite-datasource
    uid: sqlite
    access: proxy
    isDefault: true
    jsonData:
      path: /var/lib/grafana/beaver.db
    editable: false
"#;

const DASHBOARD_PROVISIONING_YAML: &str = r#"apiVersion: 1
providers:
  - name: Beaver
    orgId: 1
    folder: ''
    type: file
    disableDeletion: true
    editable: false
    options:
      path: /etc/grafana/provisioning/dashboards
      foldersFromFilesStructure: false
"#;

const GRAFANA_INI: &str = r#"[server]
http_port = 3000

[auth.anonymous]
enabled = true
org_role = Viewer

[security]
admin_user = admin
admin_password = admin

[users]
default_theme = dark

[plugins]
allow_loading_unsigned_plugins = frser-sqlite-datasource
"#;

const ENTRYPOINT_SH: &str = r#"#!/bin/sh
set -e
sqlite3 /var/lib/grafana/beaver.db < /tmp/synthetic.sql
exec /run.sh
"#;

const DOCKERFILE: &str = r#"FROM grafana/grafana-oss:latest

USER root

# Install SQLite for data loading
RUN apk add --no-cache sqlite

# Install the SQLite datasource plugin
RUN grafana cli plugins install frser-sqlite-datasource

# Copy provisioning and config
COPY provisioning /etc/grafana/provisioning
COPY grafana.ini /etc/grafana/grafana.ini
COPY synthetic.sql /tmp/synthetic.sql
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

USER grafana

ENTRYPOINT ["/entrypoint.sh"]
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn export_dashboard_writes_valid_json() {
        let config_dir = TempDir::new().unwrap();
        let output_dir = TempDir::new().unwrap();
        let output_path = output_dir.path().join("grafana-dashboard.json");

        // No config file means defaults are used
        export_dashboard(config_dir.path(), &output_path).unwrap();

        assert!(output_path.exists());
        let content = fs::read_to_string(&output_path).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("output must be valid JSON");
        // Should have a dashboard.title field
        assert!(parsed["dashboard"]["title"].is_string());
        assert_eq!(
            parsed["dashboard"]["title"].as_str().unwrap(),
            "Beaver SIEM"
        );
    }

    #[test]
    fn export_dashboard_uses_config_title() {
        let config_dir = TempDir::new().unwrap();
        let config_yaml = r#"
beaver:
  project_id: test-proj
  region: us-central1
grafana:
  title: "My Custom SIEM"
"#;
        fs::write(config_dir.path().join("beaver_config.yaml"), config_yaml).unwrap();

        let output_dir = TempDir::new().unwrap();
        let output_path = output_dir.path().join("out.json");
        export_dashboard(config_dir.path(), &output_path).unwrap();

        let content = fs::read_to_string(&output_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            parsed["dashboard"]["title"].as_str().unwrap(),
            "My Custom SIEM"
        );
    }

    #[test]
    fn export_dashboard_creates_parent_dirs() {
        let config_dir = TempDir::new().unwrap();
        let output_dir = TempDir::new().unwrap();
        let output_path = output_dir.path().join("nested").join("dir").join("dash.json");

        export_dashboard(config_dir.path(), &output_path).unwrap();
        assert!(output_path.exists());
    }

    #[test]
    fn synthetic_sql_has_insert_statements() {
        let sql = generate_synthetic_sql();
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("INSERT INTO events"));
        // Should have roughly 100 inserts
        let count = sql.matches("INSERT INTO events").count();
        assert!(
            count >= 90 && count <= 110,
            "expected ~100 inserts, got {}",
            count
        );
    }

    #[test]
    fn synthetic_sql_escapes_single_quotes() {
        let sql = generate_synthetic_sql();
        // The raw_event fields contain JSON with escaped backslashes but should
        // not contain unescaped single quotes that would break SQL
        // (all single quotes in data are doubled)
        for line in sql.lines() {
            if line.starts_with("INSERT") {
                // Count quotes — should be even (paired)
                let quote_positions: Vec<usize> =
                    line.match_indices('\'').map(|(i, _)| i).collect();
                assert!(
                    quote_positions.len() % 2 == 0,
                    "unbalanced quotes in SQL line"
                );
            }
        }
    }

    #[test]
    fn load_dashboard_title_returns_default_when_no_config() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(load_dashboard_title(tmp.path()), "Beaver SIEM");
    }

    #[test]
    fn load_dashboard_title_reads_grafana_section() {
        let tmp = TempDir::new().unwrap();
        let yaml = "grafana:\n  title: \"SOC Dashboard\"\n";
        fs::write(tmp.path().join("beaver_config.yaml"), yaml).unwrap();
        assert_eq!(load_dashboard_title(tmp.path()), "SOC Dashboard");
    }
}
