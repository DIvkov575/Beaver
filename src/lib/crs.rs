use anyhow::{anyhow, Result};
use std::process::{Command, ExitStatus, Output};
use log::{error, info};
use rand::distributions::Alphanumeric;
use rand::Rng;
use crate::lib::config::Config;
use crate::lib::resources::Resources;
use crate::lib::utilities::log_output;
use crate::MiscError;


pub fn create_vector(resources: &mut Resources, config: &Config) -> Result<()>{
    info!("creating vector...");
    let mut crs_instance_id = &mut resources.crs_instance;
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let mut random_string: String;
    let mut service_name_binding: String;

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
        service_name_binding = format!("beaver-vector-instance-{random_string}");

        let args: Vec<&str> =  Vec::from(["run", "deploy", &service_name_binding, "--image", image_url]);

        let output = Command::new("gcloud").args(args).args(config.flatten()).output()?;
        log_output(&output)?;
        if output.status.success() { break }
    }

    *crs_instance_id = service_name_binding;
    mount_gcs_crs(&config, &resources)?;

    Ok(())
}
fn create_crs_named(service_name: &str, config: &Config) -> Result<()>{
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let args: Vec<&str> =  Vec::from(["run", "deploy", service_name, "--image", image_url]);
    Command::new("gcloud").args(args).args(config.flatten()).status()?;
    Ok(())
}

fn mount_gcs_crs(config: &Config, resources: &Resources) -> Result<()> {
    // gcloud beta run services update SERVICE \
    // --execution-environment gen2 \
    // --add-volume name=VOLUME_NAME,type=cloud-storage,bucket=BUCKET_NAME \
    // --add-volume-mount volume=VOLUME_NAME,mount-path=MOUNT_PATH}


    let crs_instance_name = resources.crs_instance.clone();
    let bucket_name = resources.bucket_name.clone();

    let volume_name = "vector.yaml";
    let mount_path = "/etc/vector";
    let volume = format!("name={},bucket={}", volume_name, &bucket_name);
    let volume_mount = format!("volume={},mount-path={}", volume_name, mount_path);
    let args = vec!["beta", "run", "services", "update", &crs_instance_name,
                   "--execution-environment", "gen2", "--add-volume", &volume,
                   "--add-volume-mount", &volume_mount];

    let output = Command::new("gcloud").args(args).output()?;
    log_output(&output)?;

    Ok(())
}



// #[inline(always)]
// fn describe_formatted_crs(service_name: &str, config: &Config) -> Result<Vec<u8>> {
//     let mut args: Vec<&str> =  Vec::from(["run", "service", "describe", service_name, "--format", "export"]);
//     args.extend(config.flatten());
//
//     let Output {status: _, stdout: raw_out, stderr: raw_err } = Command::new("gcloud").args(args).output()?;
//     if !raw_err.is_empty() { return Err(anyhow!(String::from_utf8(raw_err)?)); }
//     Ok(raw_out)
// }
//

// pub fn mount_gcs_crs(service_name: &str, bucket_name: &str, config: &Config) -> Result<()> {
//     let mut description: Mapping = serde_yaml::from_slice(&describe_formatted_crs(service_name, &config)?)?;
//
//     let volume_mounts=  Vec::from([
//         Mapping::from_iter([
//             ("mountPath".into(), "/etc/vector".into()),
//             ("name".into(), "beaverVectorVolume".into())
//         ])
//     ]);
//     let volumes = Vec::from([
//         Mapping::from_iter([
//             ("name".into(), "beaverVectorVolume".into()),
//             ("csi".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
//                 ("driver".into(),"gcsfuse.run.googleapis.com".into()),
//                 ("volumeAttributes".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
//                     ("bucketName".into(), bucket_name.into()),
//                 ])))
//             ])))
//         ])
//     ]);
//
//     (&mut (*gm!(description, "spec", "template", "spec", "template", "spec", "containers",))[0]
//         .as_mapping_mut()
//         .unwrap())
//         .insert("volumeMounts".into(), volume_mounts.into());
//     gm!(description, "spec", "template", "spec", "template", "spec",)
//         .as_mapping_mut()
//         .unwrap()
//         .insert("volumes".into(), volumes.into());
//
//     // gm!(description, "metadata", "annotations",).as_mapping_mut().unwrap().remove("run.googleapis.com/lastModifier");
//     // gm!(description, "metadata", "labels",).as_mapping_mut().unwrap().remove("run.googleapis.com/lastUpdatedTime");
//     let a = gm!(description, "metadata", ).as_mapping_mut().unwrap().remove("annotations");
//     let a = gm!(description, "metadata", ).as_mapping_mut().unwrap().remove("labels");
//
//     let tmp_file = tempfile::NamedTempFile::new()?;
//     // let tmp_file = std::fs::OpenOptions::new().write(true).open("beaver_config.yaml")?;
//     // serde_yaml::to_writer(std::io::stdout(), &description).unwrap();
//     serde_yaml::to_writer(&tmp_file, &description).unwrap();
//
//     let args: Vec<&str> =  Vec::from(["beta", "run", "service", "replace", tmp_file.path().to_str().unwrap()]);
//     Command::new("gcloud").args(args).args(config.flatten()).status()?;
//
//     Ok(())
// }

// pub fn delete_crs(service_name: &str, config: &Config) -> Result<()>{
//     let args: Vec<&str> =  Vec::from(["run", "service", "delete", service_name]);
//     Command::new("gcloud").args(args).args(config.flatten()).status()?;
//     Ok(())
// }
