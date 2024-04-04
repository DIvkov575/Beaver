use anyhow::{anyhow, Result};
use std::error::Error;
use std::process::{Command, Output};
use log::{info, log};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde_yaml::Mapping;
use crate::lib::config::Config;
use crate::lib::resources::Resources;


#[macro_export]
macro_rules! gm {($a:ident, $($b:literal,)*) => {$a$(.get_mut(&serde_yaml::Value::String($b.to_owned().into())).unwrap())*};}

pub fn create_vector(resources: &mut Resources, config: &Config) -> Result<()> {
    info!("creating vector...");
    let mut crj_instance_id = &mut resources.crj_instance;
    let bucket_name = resources.bucket_name.clone();

    if crj_instance_id.as_str() == "" {
        *crj_instance_id = create_crj_unnamed(&config)?;
    } else {
        create_crj_named(&crj_instance_id, &config)?;
    }

    mount_gcs_crj(&crj_instance_id, &bucket_name, &config)?;
    Ok(())
}
fn create_crj_unnamed(config: &Config) -> Result<String>{
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let mut random_string: String;
    let mut job_name_binding: String;

    loop {
        random_string = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(9)
            .map(char::from)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        job_name_binding = format!("beaver-vectorinstance-{random_string}");

        let args: Vec<&str> =  Vec::from(["run", "jobs", "create", &job_name_binding, "--image", image_url]);

        if Command::new("gcloud").args(args).args(config.flatten()).status().unwrap().success() {
            break
        } else {
            continue
        }
    }
    Ok(job_name_binding)
}
fn create_crj_named(job_name: &str, config: &Config) -> Result<()>{
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let args: Vec<&str> =  Vec::from(["run", "jobs", "create", job_name, "--image", image_url]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}

pub fn execute_crj(resources: &Resources, config: &Config) -> Result<()> {
    info!("executing crj...");
    let crj_instance = resources.crj_instance.clone();
    let args: Vec<&str> =  Vec::from(["run", "jobs", "execute", &crj_instance]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}


#[inline(always)]
fn describe_formatted_crj(job_name: &str, config: &Config) -> Result<Vec<u8>> {
    let mut args: Vec<&str> =  Vec::from(["run", "jobs", "describe", job_name, "--format", "export"]);
    args.extend(config.flatten());

    let Output {status: _, stdout: raw_out, stderr: raw_err } = Command::new("gcloud").args(args).output()?;
    if !raw_err.is_empty() { return Err(anyhow!(String::from_utf8(raw_err)?)); }
    Ok(raw_out)
}


pub fn mount_gcs_crj(job_name: &str, bucket_name: &str, config: &Config) -> Result<()> {
    let mut description: Mapping = serde_yaml::from_slice(&describe_formatted_crj(job_name, &config)?)?;

    let volume_mounts=  Vec::from([
        Mapping::from_iter([
            ("mountPath".into(), "/etc/vector".into()),
            ("name".into(), "beaverVectorVolume".into())
        ])
    ]);
    let volumes = Vec::from([
        Mapping::from_iter([
            ("name".into(), "beaverVectorVolume".into()),
            ("csi".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
                ("driver".into(),"gcsfuse.run.googleapis.com".into()),
                ("volumeAttributes".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
                    ("bucketName".into(), bucket_name.into()),
                ])))
            ])))
        ])
    ]);

    (&mut (*gm!(description, "spec", "template", "spec", "template", "spec", "containers",))[0]
        .as_mapping_mut()
        .unwrap())
        .insert("volumeMounts".into(), volume_mounts.into());
    gm!(description, "spec", "template", "spec", "template", "spec",)
        .as_mapping_mut()
        .unwrap()
        .insert("volumes".into(), volumes.into());

    // gm!(description, "metadata", "annotations",).as_mapping_mut().unwrap().remove("run.googleapis.com/lastModifier");
    // gm!(description, "metadata", "labels",).as_mapping_mut().unwrap().remove("run.googleapis.com/lastUpdatedTime");
    let a = gm!(description, "metadata", ).as_mapping_mut().unwrap().remove("annotations");
    let a = gm!(description, "metadata", ).as_mapping_mut().unwrap().remove("labels");

    let tmp_file = tempfile::NamedTempFile::new()?;
    // let tmp_file = std::fs::OpenOptions::new().write(true).open("beaver_config.yaml")?;
    // serde_yaml::to_writer(std::io::stdout(), &description).unwrap();
    serde_yaml::to_writer(&tmp_file, &description).unwrap();

    let args: Vec<&str> =  Vec::from(["beta", "run", "jobs", "replace", tmp_file.path().to_str().unwrap()]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;

    Ok(())
}
pub fn delete_crj(job_name: &str, config: &Config) -> Result<()>{
    let args: Vec<&str> =  Vec::from(["run", "jobs", "delete", job_name]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}
