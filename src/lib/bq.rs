use std::fmt::format;
use std::path::Component::ParentDir;
use std::process::Command;
use anyhow::Result;
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::lib::config::Config;
use crate::lib::resources::{Resources, Tracker};
use crate::MiscError;


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BqTable {
    pub project_id: String,
    pub dataset_id: String,
    pub table_id: String,

}

impl BqTable {
    pub fn empty(config: &Config) -> Self {
        Self {
            project_id: config.project.clone(),
            dataset_id: String::new(),
            table_id: String::new(),

        }
    }
    pub fn new(project_id: &str, dataset_id: &str, table_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            dataset_id: dataset_id.to_string(),
            table_id: table_id.to_string(),
        }
    }
    pub fn flatten(&self) -> String {
        format!("{}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }

    pub fn formatted_flatten(&self) -> String {
        format!("--bigquery-table={}:{}.{}", self.project_id, self.dataset_id, self.table_id)
    }
}

pub fn create_dataset_unnamed(project_id: &str) -> Result<String> {
    let mut random_string: String;
    let mut dataset_id_binding: String;

    let mut ctr = 0u8;
    loop {
        if ctr >= 5 { return Err(MiscError::MaxResourceCreationRetries.into()) }
        ctr += 1;

        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(4)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        dataset_id_binding = format!("beaver_datalake_{}", random_string);
        let dataset_formatted_binding = format!("{}:beaver_datalake_{}", project_id, random_string);
        let args: Vec<&str> = Vec::from(["mk", "--dataset", &dataset_formatted_binding, ]);


        let output = Command::new("bq").args(args).output()?;

        if !output.stderr.is_empty() {
            error!("{:?}", String::from_utf8(output.stderr)?)
        }

        if output.status.success() {
            info!("{:?}", String::from_utf8(output.stdout)?);
            break;
        } else {
            continue;
        }


    }

    Ok(dataset_id_binding)
}

pub fn create_dataset_named(dataset_id: &str, project_id: &str) -> Result<()> {
    let id_binding = format!("{}:{}", project_id, dataset_id);
    let args: Vec<&str> = Vec::from(["mk", "--dataset", &id_binding, ]);
    let output = Command::new("bq").args(args).output()?;

    if output.stderr != [0u8; 0] {
        error!("{:?}", String::from_utf8(output.stderr)?) }
    else {
        info!("{:?}", String::from_utf8(output.stdout)?)
    }

    Ok(())
}

pub fn create_table(dataset_id: &str, table_id: &str, project_id: &str) -> Result<()> {
    let id_binding = format!("{}:{}.{}", project_id, dataset_id, table_id);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--table",
        id_binding.as_ref(),
        "data: JSON"
    ]);
    let output = Command::new("bq").args(args).output()?;

    if output.stderr != [0u8; 0] {
        error!("{:?}", String::from_utf8(output.stderr)?) }
    else {
        info!("{:?}", String::from_utf8(output.stdout)?)
    }
    Ok(())
}


pub fn delete_dataset(dataset_id: &str, project_id: &str) -> Result<()> {
    info!("deleting bq dataset: {}", dataset_id);
    let target = format!("{}:{}", project_id, dataset_id);
    let output = Command::new("bq")
        .args(["rm", "-r", "-f", "-d", &target])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("bq dataset delete failed: {}", stderr));
    }
    Ok(())
}

pub fn create(tracker: &mut Tracker, config: &Config) -> Result<()> {
    info!("creating bq...");
    let project_id = tracker.resources().biq_query.project_id.clone();

    let dataset_id = create_dataset_unnamed(&project_id)?;
    tracker.record_bq_dataset(dataset_id.clone())?;

    let table_id = String::from("table1");
    create_table(&dataset_id, &table_id, &project_id)?;
    tracker.record_bq_table(table_id)?;

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lib::resources::Tracker;
    use crate::lib::test_helpers::{bq_dataset_exists, test_config, tempdir_resources};

    #[test]
    #[ignore]
    fn create_then_delete_leaves_no_dataset() {
        let config = test_config();
        let (_dir, mut res) = tempdir_resources();
        let mut tracker = Tracker::new(&mut res);

        create(&mut tracker, &config).expect("bq create failed");
        let dataset_id = tracker.resources().biq_query.dataset_id.clone();
        assert!(!dataset_id.is_empty(), "create must record dataset_id");
        assert!(
            bq_dataset_exists(&dataset_id, &config.project),
            "dataset {} should exist after create",
            dataset_id
        );

        delete_dataset(&dataset_id, &config.project).expect("bq delete failed");
        assert!(
            !bq_dataset_exists(&dataset_id, &config.project),
            "dataset {} should be gone after delete",
            dataset_id
        );
    }
}
