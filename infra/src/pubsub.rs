use std::process::Command;
use crate::config::Config;
use anyhow::Result;
use rand::distributions::Alphanumeric;
use rand::Rng;


pub fn create_pubsub_topic(config: &Config) -> Result<()> {
   let mut random_string: String;
    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let topic_name= format!("beaver_{random_string}");


        let args: Vec<&str> = Vec::from([
            "pubsub",
            "topics",
            "create",
            topic_name.as_ref()
        ]);

        if Command::new("gcloud").args(args).args(config.get_project()).status().unwrap().success() {
            break
        } else {
            continue
        }
    }
   Ok(())
}
