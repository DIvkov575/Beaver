use std::fmt::format;
use std::process::Command;
use anyhow::Result;
use crate::lib::config::Config;

pub fn create_service_account(sa_name: &str) -> Result<()> {
    let args: Vec<&str> =  Vec::from([
        "iam", "service-accounts", "create", sa_name,
        "--description=\"Service account created by beaver SIEM tool for impersonation during resource creation\"",
        "--display-name=\" Beaver Impersonation Account\""
    ]);
    Command::new("gcloud").args(args).status()?;
    Ok(())
}

pub fn add_service_account_roles(sa_name: &str, config: &Config, roles: Vec<String>) -> Result<()> {
    // gcloud projects add-iam-policy-binding PROJECT_ID \
    // --member="serviceAccount:SA_NAME@PROJECT_ID.iam.gserviceaccount.com" \
    // --role="ROLE_NAME"
    let member_binding = format!("--member=serviceAccount:{}@{}.iam.gserviceaccount.com", sa_name, config.project);
    let mut args: Vec<String> =  Vec::from([
        "projects", "add-iam-policy-binding", config.project,
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
