use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use crate::config::Config;

pub fn create_table(table_name: &str,dataset_name: &str, config: &Config) -> Result<()> {
    let id_binding = format!("{}:{}.{}", config.project, dataset_name, table_name);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--table",
        id_binding.as_ref(),
        "data: JSON"
    ]);
    Command::new("bq").args(args).spawn()?;
    Ok(())
}

pub fn create_dataset(dataset_name: &str, config: &Config) -> Result<()> {
    // roles/bigquery.dataEditor
    // roles/bigquery.dataOwner
    // roles/bigquery.user
    // roles/bigquery.admin

    // bq --location=LOCATION mk \
    // --dataset \
    // --default_kms_key=KMS_KEY_NAME \
    // --default_partition_expiration=PARTITION_EXPIRATION \
    // --default_table_expiration=TABLE_EXPIRATION \
    // --description="DESCRIPTION" \
    // --label=LABEL_1:VALUE_1 \
    // --label=LABEL_2:VALUE_2 \
    // --max_time_travel_hours=HOURS \
    // --storage_billing_model=BILLING_MODEL \
    // PROJECT_ID:DATASET_ID

    let id_binding = format!("{}:{}", config.project, dataset_name);
    let args: Vec<&str> = Vec::from([
        "mk",
        "--dataset",
        id_binding.as_ref(),
    ]);

    Command::new("bq").args(args).spawn()?;
    Ok(())
}


pub fn check_for_bq() -> Result<()> {
    match Command::new("bq").output() {
        Ok(_) => return Ok(()),
        Err(_) => panic!("Please ensure you have bq (biqquery utility tool installed)"),
    }
}



pub fn check_for_python() -> Result<(String, String)> {
    //!  - Inner line doc
    ///  - Outer line doc (exactly 3 slashes)
    /// Output Result< python command, pip command >

    let py_version: Option<String>;
    let py3_version: Option<String>;
    let pip_version: Option<String>;
    let pip3_version: Option<String>;

    let mut py_command;
    let mut pip_command;

    match Command::new("python").arg("--version").output() {
        Ok(output) => py_version = Some(String::from_utf8(output.stdout)?),
        Err(_) => py_version = None,
    }
    match Command::new("python3").arg("--version").output() {
        Ok(output) => py3_version = Some(String::from_utf8(output.stdout)?),
        Err(_) => py3_version = None,
    }
    match Command::new("pip").arg("--version").output() {
        Ok(output) => pip_version = Some(String::from_utf8(output.stdout)?),
        Err(_) => pip_version = None,
    }
    match Command::new("pip3").arg("--version").output() {
        Ok(output) => pip3_version = Some(String::from_utf8(output.stdout)?),
        Err(_) => pip3_version = None,
    }

    if py_version.is_none() && py3_version.is_none() { panic!("Please have python installed on your system as `python` or `python3`"); }
    if pip_version.is_none() && pip3_version.is_none() { panic!("Please have pip installed on your system as `pip` or `pip3`"); }

    if py3_version.is_some() { py_command = "python3".to_string(); }
    else { py_command = "python".to_string()}

    if pip3_version.is_some() { pip_command = "pip3".to_string(); }
    else { pip_command = "pip".to_string()}

    Ok((py_command, pip_command))
}
