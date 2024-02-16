use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::lib::config::Config;
use crate::lib::resources::Resources;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct SA {
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub description: String,
}
impl SA {
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            roles: Vec::new(),
            description: String::new()
        }
    }
    // pub fn new(name: &str, roles: Vec<String>, description: &str) -> Self {
    //     Self {
    //         name: name.to_string(),
    //         roles: roles.clone(),
    //         description: description
    //
    //     }
    // }
}


pub fn create_service_account(sa_name: &str) -> Result<()> {
    let args: Vec<&str> =  Vec::from([
        "iam", "service-accounts", "create", sa_name,
        "--description=\"Service account created by beaver SIEM tool for impersonation during resource creation\"",
        "--display-name=\" Beaver Impersonation Account\""
    ]);
    Command::new("gcloud").args(args).status()?;
    Ok(())
}

pub fn add_sa_roles(sa_name: &str, config: &Config, roles: Vec<&str>) -> Result<()> {
    let member_binding = format!("--member=serviceAccount:{}@{}.iam.gserviceaccount.com", sa_name, config.project);
    let mut args: Vec<String> =  Vec::from([
        "projects", "add-iam-policy-binding", &config.project,
        &member_binding,
    ].map(|x|x.to_string()));

    args.extend(roles.iter().map(|x| -> String {format!("--role={}", x)}));

    Command::new("gcloud").args(args).status()?;
    Ok(())
}

pub fn allow_service_account_impersonation(sa_name: &str, user_email: &str, config: &Config) -> Result<()> {
    let sa_binding = format!("{}@{}.iam.gserviceaccount.com", sa_name, config.project);
    let member_binding = format!("--member=user:{}", user_email);
    let args: Vec<&str> =  Vec::from([
        "iam", "service-accounts", "add-iam-policy-binding",
        &sa_binding,
        &member_binding,
        "--role=roles/iam.serviceAccountUser"
    ]);
    Command::new("gcloud").args(args).status()?;

    Ok(())
}

pub fn create_creational_sa(user_email: &str, config: &Config) -> Result<()> {
    create_service_account("BeaverCreationalSA")?;
    allow_service_account_impersonation("BeaverCreationalSA", user_email, &config)?;
    add_sa_roles("BeaverCreationalSA", &config, Vec::from([
        "roles/compute.instanceAdmin.v1", // creating compute sa
        "roles/iam.serviceAccountCreator", // creating compute sa
        "roles/run.developer", // creating cloud run?
        "roles/biqquery.admin", //bq

    ]))?;


    // roles/bigquery.dataEditor
    // roles/bigquery.dataOwner
    // roles/bigquery.user
    // roles/bigquery.admin
    Ok(())
}

pub fn create_compute_sa(user_email: &str, resources: &Resources, config: &Config) -> Result<()> {
    let mut compute_sa = resources.compute_sa.borrow_mut();
    compute_sa.name = String::from("BeaverComputeSA");
    compute_sa.email =format!("{}@{}.iam.gserviceaccount.com", compute_sa.name, config.project);

    create_service_account(&compute_sa.name)?;
    allow_service_account_impersonation(&compute_sa.name, user_email, &config)?;
    add_sa_roles("BeaverComputeSA", &config, vec![
        "roles/compute.serviceAgent",
        "roles/pubsub.publisher",
        "roles/pubsub.subscriber",
        "roles/run.invoker" // cron
    ])?;


    Ok(())
}