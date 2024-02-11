use anyhow::Result;
use std::io::{Read, Stderr, stdout, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};
use run_script::ScriptOptions;


pub fn create_pysigma_converter(path_to_config: &Path) -> Result<()> {
    /// Creates virtualenv, activates env, installs matano-pysigma-backend from "pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git"
    let path = path_to_config.join("detections");
    let args = vec![path.to_str().unwrap().to_string()];
    let options = ScriptOptions::new();

    let (code, output, error) = run_script::run(
        r#"
        cd $1 || exit
        python3 -m venv venv
        source venv/bin/activate
        pip3 install git+https://github.com/matanolabs/pySigma-backend-matano.git
         "#,
        &args,
        &options,
    ).unwrap();

    println!("exit code {}: {}", code, output);
    println!("{}", error);

    Ok(())
}

pub fn generate_detections(path_to_config: &Path) -> Result<()> {
    let path = path_to_config.join("detections");
    let args = vec![path.to_str().unwrap().to_string()];
    let options = ScriptOptions::new();

    let (code, output, error) = run_script::run(
        r#"
        cd $1
        source venv/bin/activate

        files=( $(ls))
        for file in "${files[@]}"; do
          extension="${file##*.}"
          if [ "$extension" == "yml" ] || [ "$extension" == "yaml" ]; then
            python3 sigma_generate.py "$file"
            echo "$file successfully parsed"
          fi
        done
        "#,
        &args,
        &options,
    ).unwrap();

    println!("exit code {}: {}", code, output);
    println!("{}", error);

    Ok(())
}