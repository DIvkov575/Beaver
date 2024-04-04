use anyhow::Result;
use std::path::{Path, PathBuf};
use log::{error, info};
use run_script::ScriptOptions;


pub fn setup_detections_venv(path_to_config: &Path) -> Result<()> {
    /// Creates virtualenv, activates env, installs matano-pysigma-backend from "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"
    // opted to use sh bc executing python w/ active virtual environment was problematic
    let path = path_to_config.join("detections");
    let args = vec![path.to_str().unwrap().to_string()];
    let options = ScriptOptions::new();

    let (code, output, error) = run_script::run(
        r#"
        cd $1 || exit
        python3 -m venv venv
        source venv/bin/activate
        pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git 'apache-beam[gcp]'
        "#,
        &args,
        &options,
    ).unwrap();

    println!("exit code {}: {}", code, output);
    println!("{}", error);

    Ok(())
}

pub fn generate_detections(path_to_config: &Path) -> Result<()> {
    info!("converting sigma detections...");
    // opted to use sh bc executing python w/ active virtual environment was problematic
    let path = path_to_config.join("detections");
    let output_path = vec![path.join("output").to_str().unwrap().to_string()];
    let options = ScriptOptions::new();

    let (code, output, error) = run_script::run(
        r#"
        cd $1
        source ../venv/bin/activate

        files=( $(ls ../input))
        for file in "${files[@]}"; do
          extension="${file##*.}"
          if [ "$extension" == "yml" ] || [ "$extension" == "yaml" ]; then
            python3 ../sigma_generate.py ../input/"$file"
          fi
        done
        "#,
        &output_path,
        &options,
    ).unwrap();

    if error != "" {
        error!("{}", error);
    } else {
        output.lines().for_each(|line| info!("{}", line));
    }


    Ok(())
}