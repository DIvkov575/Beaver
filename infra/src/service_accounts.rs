use std::fmt::format;
use std::process::Command;
use anyhow::Result;

pub fn create_service_account(sa_name: &str) -> Result<()> {
    let args: Vec<&str> =  Vec::from([
        "iam", "service-accounts", "create", sa_name,
        "--description=\"Service account created by beaver SIEM tool for impersonation during resource creation\"",
        "--display-name=\" Beaver Impersonation Account\""
    ]);
    Command::new("gcloud").args(args).status()?;
    Ok(())
}



pub fn update_service_account_roles(sa_name: &str) -> Result<()> {
    // gcloud projects add-iam-policy-binding PROJECT_ID \
    // --member="serviceAccount:SA_NAME@PROJECT_ID.iam.gserviceaccount.com" \
    // --role="ROLE_NAME"
    let args: Vec<&str> =  Vec::from([
        "iam", "service-accounts", "create", sa_name,
        "--description=\"Service account created by beaver SIEM tool for impersonation during resource creation\"",
        "--display-name=\" Beaver Impersonation Account\""
    ]);
    Command::new("gcloud").args(args).status()?;
    Ok(())
}
