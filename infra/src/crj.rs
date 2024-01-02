use std::collections::HashMap;
use std::error::Error;
use std::process::{Command, Output, Stdio};
use serde_yaml::Mapping;
use crate::config::Config;


pub fn delete_crj(job_name: &str, config: &Config) -> Result<(), Box<dyn Error>>{
    let mut args: Vec<&str> =  Vec::from(["run", "jobs", "delete", job_name]);
    args.extend(config.flatten());

    Command::new("gcloud").args(args).status()?;
    Ok(())
}
pub fn create_crj(job_name: &str, config: &Config) -> Result<(), Box<dyn Error>>{
    let image_url = "docker.io/timberio/vector:latest-alpine";
    let mut args: Vec<&str> =  Vec::from(["run", "jobs", "create", job_name, "--image", image_url]);
    args.extend(config.flatten());

    Command::new("gcloud").args(args).status()?;
    Ok(())
}

pub fn describe_formatted_crj(job_name: &str, config: &Config) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut args: Vec<&str> =  Vec::from(["run", "jobs", "describe", job_name, "--format", "export"]);
    args.extend(config.flatten());

    let Output {status: _, stdout: raw_out, stderr: raw_err } = Command::new("gcloud").args(args).output()?;
    if !raw_err.is_empty() {
        return Err(String::from_utf8(raw_err)?.into())
    }

    Ok(raw_out)
}

pub fn exec_crj(job_name: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    let mut args: Vec<&str> = Vec::from(["run", "jobs", "describe", job_name]);
    args.extend(config.flatten());


    Ok(())
}

// pub fn enable_apis(config: Config) -> Result<(), Box<dyn Error>> {
//     // gcloud services enable cloudscheduler.googleapis.com pubsub.googleapis.com
//     let job_name = "beaver-vrl";
//     let mut args: Vec<&str> = Vec::from(["run", "jobs", "describe", job_name]);
//     args.extend(config.decompose());
//
//
//     Ok(())
// }


pub fn mount_gcs_crj(job_name: &str, config: &Config) -> Result<(), Box<dyn Error>> {
    // update crj_description to have gcs mounting point
    #[inline(always)]
    fn update_crj_config_file(volume_name: &str, bucket_name: &str, input_description: &mut Mapping ) -> Result<(), Box<dyn Error>> {
        // teehee visitor pattern?!?!
        // https://cloud.google.com/run/docs/configuring/jobs/cloud-storage-volume-mounts

        let mount_path = "/etc/vector";
        let volume_name = "vector.yaml";

        let volume_mounts=  Vec::from([
            Mapping::from_iter(HashMap::from([
                ("mountPath".into(), mount_path.into()),
                ("name".into(), volume_name.into())
            ]))
        ]);

        let volumes = Vec::from([
            Mapping::from_iter([
                ("name".into(), volume_name.into()),
                ("csi".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
                    ("driver".into(),"gcsfuse.run.googleapis.com".into()),
                    ("volumeAttributes".into(), serde_yaml::Value::Mapping(Mapping::from_iter([
                        ("bucketName".into(), bucket_name.into()),
                    ])))
                ])))
            ])
        ]);


        let mut point_a=  gm!(input_description, "spec", "template", "spec", "template", "spec", "containers", 0,).as_mapping_mut().unwrap();
        point_a.insert("volumeMounts".into(), volume_mounts.into());

        let mut point_b =  gm!(input_description, "spec", "template", "spec", "template", "spec",).as_mapping_mut().unwrap();
        point_b.insert("volumes".into(), volumes.into());

        #[macro_export]
        macro_rules! gm {
        ($a:ident, $($b:literal,)*) => {
           $a$(.get_mut($b).ok_or("error during description unwrap")?)*
        };
    }



        Ok(())
    }

    let mut description = serde_yaml::from_slice(&describe_formatted_crj(job_name, &config)?)?;
    update_crj_config_file("", "", &mut description)?;

    let tmp_file = tempfile::NamedTempFile::new()?;
    serde_yaml::to_writer(tmp_file, &description).unwrap();

    let mut args: Vec<&str> =  Vec::from(["beta", "run", "jobs", "replace", tmp_file.path()]);
    args.extend(config.flatten());
    Command::new("gcloud").args(args).status()?;

    Ok(())
}

