//! Generates deployment artifacts for the Grafana container image.
//!
//! These files get baked into the Docker image at Cloud Build time. The
//! resulting container starts Grafana pre-configured with:
//! - A BigQuery datasource pointed at the Beaver SIEM dataset
//! - The Beaver SIEM dashboard pre-loaded as the home dashboard
//! - Anonymous read access (optional) for SOC analysts

use std::fs;
use std::path::Path;

use anyhow::Result;

use super::GrafanaConfig;

/// Generates the datasource provisioning YAML that configures Grafana to query
/// BigQuery using GCE workload identity (the attached Cloud Run service account).
pub fn generate_datasource_yaml(project: &str, _sa_email: &str) -> String {
    format!(
        r#"apiVersion: 1
datasources:
  - name: Beaver
    type: grafana-bigquery-datasource
    uid: beaver-bq
    access: proxy
    jsonData:
      authenticationType: gce
      defaultProject: {project}
    editable: false
"#,
        project = project,
    )
}

/// Generates grafana.ini with server, security, dashboard, and plugin settings.
pub fn generate_grafana_ini(admin_password: &str, allow_anonymous: bool, port: u16) -> String {
    let anonymous_enabled = if allow_anonymous { "true" } else { "false" };
    format!(
        r#"[server]
http_port = {port}

[security]
admin_password = {admin_password}

[auth.anonymous]
enabled = {anonymous_enabled}

[dashboards]
default_home_dashboard_path = /etc/grafana/provisioning/dashboards/beaver-siem.json

[plugins]
allow_loading_unsigned_plugins = frser-sqlite-datasource
"#,
        port = port,
        admin_password = admin_password,
        anonymous_enabled = anonymous_enabled,
    )
}

/// Generates the dashboard provisioning YAML that tells Grafana to load
/// dashboard JSON files from the provisioning directory.
pub fn generate_dashboard_provisioning_yaml() -> String {
    r#"apiVersion: 1
providers:
  - name: Beaver
    type: file
    options:
      path: /etc/grafana/provisioning/dashboards
      foldersFromFilesStructure: false
"#
    .to_string()
}

/// Generates the Dockerfile for the Grafana container image.
pub fn generate_dockerfile(port: u16) -> String {
    format!(
        r#"FROM grafana/grafana-oss:11.0.0
ENV GF_INSTALL_PLUGINS=grafana-bigquery-datasource
COPY provisioning/ /etc/grafana/provisioning/
COPY grafana.ini /etc/grafana/grafana.ini
EXPOSE {port}
"#,
        port = port,
    )
}

/// Writes all build-context files to `dir`, producing a directory structure
/// ready for `gcloud builds submit`:
///
/// ```text
/// dir/
///   Dockerfile
///   grafana.ini
///   provisioning/
///     datasources/bigquery.yaml
///     dashboards/dashboard.yaml
///     dashboards/beaver-siem.json
/// ```
pub fn write_build_context(
    dir: &Path,
    dashboard_json: &str,
    config: &GrafanaConfig,
    project: &str,
    sa_email: &str,
) -> Result<()> {
    let port = config.port;
    let admin_password = if config.admin_password.is_empty() {
        "beaver-admin"
    } else {
        &config.admin_password
    };
    let allow_anonymous = config.allow_anonymous;

    // Create directory structure
    let datasources_dir = dir.join("provisioning").join("datasources");
    let dashboards_dir = dir.join("provisioning").join("dashboards");
    fs::create_dir_all(&datasources_dir)?;
    fs::create_dir_all(&dashboards_dir)?;

    // Write Dockerfile
    fs::write(dir.join("Dockerfile"), generate_dockerfile(port))?;

    // Write grafana.ini
    fs::write(
        dir.join("grafana.ini"),
        generate_grafana_ini(admin_password, allow_anonymous, port),
    )?;

    // Write datasource provisioning
    fs::write(
        datasources_dir.join("bigquery.yaml"),
        generate_datasource_yaml(project, sa_email),
    )?;

    // Write dashboard provisioning config
    fs::write(
        dashboards_dir.join("dashboard.yaml"),
        generate_dashboard_provisioning_yaml(),
    )?;

    // Write the actual dashboard JSON
    fs::write(dashboards_dir.join("beaver-siem.json"), dashboard_json)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn datasource_yaml_contains_project() {
        let yaml = generate_datasource_yaml("my-gcp-project", "grafana-reader@my-gcp-project.iam.gserviceaccount.com");
        assert!(yaml.contains("defaultProject: my-gcp-project"));
        assert!(yaml.contains("grafana-bigquery-datasource"));
        assert!(yaml.contains("uid: beaver-bq"));
        assert!(yaml.contains("authenticationType: gce"));
        assert!(yaml.contains("editable: false"));
    }

    #[test]
    fn grafana_ini_contains_port_and_password() {
        let ini = generate_grafana_ini("s3cr3t", true, 8080);
        assert!(ini.contains("http_port = 8080"));
        assert!(ini.contains("admin_password = s3cr3t"));
        assert!(ini.contains("enabled = true"));
        assert!(ini.contains("default_home_dashboard_path = /etc/grafana/provisioning/dashboards/beaver-siem.json"));
        assert!(ini.contains("allow_loading_unsigned_plugins = frser-sqlite-datasource"));
    }

    #[test]
    fn grafana_ini_anonymous_disabled() {
        let ini = generate_grafana_ini("pass", false, 3000);
        assert!(ini.contains("enabled = false"));
        assert!(ini.contains("http_port = 3000"));
    }

    #[test]
    fn dashboard_provisioning_yaml_structure() {
        let yaml = generate_dashboard_provisioning_yaml();
        assert!(yaml.contains("apiVersion: 1"));
        assert!(yaml.contains("name: Beaver"));
        assert!(yaml.contains("type: file"));
        assert!(yaml.contains("path: /etc/grafana/provisioning/dashboards"));
        assert!(yaml.contains("foldersFromFilesStructure: false"));
    }

    #[test]
    fn dockerfile_contains_port_and_plugins() {
        let df = generate_dockerfile(3000);
        assert!(df.contains("FROM grafana/grafana-oss:11.0.0"));
        assert!(df.contains("GF_INSTALL_PLUGINS=grafana-bigquery-datasource"));
        assert!(df.contains("EXPOSE 3000"));
        assert!(df.contains("COPY provisioning/ /etc/grafana/provisioning/"));
        assert!(df.contains("COPY grafana.ini /etc/grafana/grafana.ini"));
    }

    #[test]
    fn dockerfile_custom_port() {
        let df = generate_dockerfile(9090);
        assert!(df.contains("EXPOSE 9090"));
    }

    #[test]
    fn write_build_context_creates_expected_tree() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let config = GrafanaConfig {
            enabled: true,
            name: "Beaver SIEM".to_string(),
            port: 8080,
            admin_password: "test-pass".to_string(),
            allow_anonymous: true,
        };

        let dashboard_json = r#"{"title": "Beaver SIEM"}"#;

        write_build_context(dir, dashboard_json, &config, "test-project", "sa@test.iam")
            .expect("write_build_context should succeed");

        // Verify all expected files exist
        assert!(dir.join("Dockerfile").exists());
        assert!(dir.join("grafana.ini").exists());
        assert!(dir.join("provisioning/datasources/bigquery.yaml").exists());
        assert!(dir.join("provisioning/dashboards/dashboard.yaml").exists());
        assert!(dir.join("provisioning/dashboards/beaver-siem.json").exists());

        // Verify content correctness
        let dockerfile = fs::read_to_string(dir.join("Dockerfile")).unwrap();
        assert!(dockerfile.contains("EXPOSE 8080"));

        let ini = fs::read_to_string(dir.join("grafana.ini")).unwrap();
        assert!(ini.contains("http_port = 8080"));
        assert!(ini.contains("admin_password = test-pass"));
        assert!(ini.contains("enabled = true"));

        let ds = fs::read_to_string(dir.join("provisioning/datasources/bigquery.yaml")).unwrap();
        assert!(ds.contains("defaultProject: test-project"));

        let dash_prov = fs::read_to_string(dir.join("provisioning/dashboards/dashboard.yaml")).unwrap();
        assert!(dash_prov.contains("name: Beaver"));

        let dash_json = fs::read_to_string(dir.join("provisioning/dashboards/beaver-siem.json")).unwrap();
        assert_eq!(dash_json, dashboard_json);
    }

    #[test]
    fn write_build_context_uses_defaults_when_optional_fields_absent() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let config = GrafanaConfig {
            enabled: true,
            name: "Beaver SIEM".to_string(),
            port: 3000,
            admin_password: String::new(),
            allow_anonymous: false,
        };

        write_build_context(dir, "{}", &config, "proj", "sa@x.iam")
            .expect("write_build_context with defaults");

        let dockerfile = fs::read_to_string(dir.join("Dockerfile")).unwrap();
        assert!(dockerfile.contains("EXPOSE 3000"));

        let ini = fs::read_to_string(dir.join("grafana.ini")).unwrap();
        assert!(ini.contains("http_port = 3000"));
        assert!(ini.contains("admin_password = beaver-admin"));
        assert!(ini.contains("enabled = false"));
    }
}
