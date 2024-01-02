use std::error::Error;
use std::fmt::format;
use std::process::Command;
use crate::config::Config;
// use uuid

use rand::{distributions::Alphanumeric, Rng}; // 0.8


pub struct Bucket {
}

impl Bucket {

    pub fn create_bucket(config: &Config) -> Result<String, Box<dyn Error>> {
        let mut random_string: String;

        loop {
            random_string = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(9)
                .map(char::from)
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let binding = format!("gs://beaver_{random_string}");

            let mut flags: Vec<&str> = Vec::from(["--location", config.region, "--project", config.project]);
            let mut args: Vec<&str> = Vec::from(["storage", "buckets", "create", &binding]);


            if Command::new("gcloud").args([args.as_slice(), &flags.as_slice()].concat()).status().unwrap().success() {
                break
            } else {
                continue
            }
        }

        Ok(format!("beaver_{random_string}"))
    }
}