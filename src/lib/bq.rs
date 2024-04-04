use std::fmt::format;
use std::path::Component::ParentDir;
use std::process::Command;
use anyhow::Result;
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::lib::config::Config;
use crate::lib::resources::Resources;


#[derive(Deserialize, Serialize, Debug)]
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

    loop {
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

        // TODO: Test in depth -> when does it have stderr
        if output.stderr != [0u8; 0] {
            error!("{:?}", String::from_utf8(output.stderr)?) }

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


pub fn create(resources: &mut Resources, config: &Config) -> Result<()> {
    info!("creating bq...");
    // create bq instance from config.artifacts.resources.yaml if names were provided, otherwise names dataset dynamically "beaver_{random_string}" and table "table1"
    // let mut bq_binding = resources.biq_query.borrow_mut();
    // let mut bq = bq_binding.as_mut().unwrap();
    let mut bq = &mut resources.biq_query;


    // create dataset & store id
    if bq.dataset_id == "" {
        bq.dataset_id = create_dataset_unnamed(&bq.project_id)?;
    } else { create_dataset_named(&bq.dataset_id, &bq.project_id)? }

    // create table & store id
    if bq.table_id == "" {
        bq.table_id = String::from("table1");
        create_table(&bq.dataset_id, &bq.table_id, &bq.project_id)?;
    } else {
        create_table(&bq.dataset_id, &bq.table_id, &bq.project_id)?;
    }

    Ok(())
}
