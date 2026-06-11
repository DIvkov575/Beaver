use anyhow::Result;
use std::path::Path;
use log::{error, info};
use run_script::ScriptOptions;


/// Creates `<path>/detections/venv` and installs `pySigma-backend-matano` + `apache-beam[gcp]`.
// Shells out to bash because activating a venv from inside Rust's process table is awkward.
pub fn setup_detections_venv(path_to_config: &Path) -> Result<()> {
    let path = path_to_config.join("detections");
    let sigma_beam_path = crate::lib::utilities::sigma_beam_dir();

    let args = vec![path.to_str().unwrap().to_string(), sigma_beam_path];
    let options = ScriptOptions::new();

    let (code, output, error) = run_script::run(
        r#"
        cd "$1" || exit
        python3 -m venv venv
        source venv/bin/activate
        pip3 install --upgrade pip
        # pySigma for compile-time rule validation; apache-beam for the
        # template build; sigma_beam as a local editable install so the
        # template's `from sigma_beam.correlation_pipeline import run` works.
        pip3 install 'pysigma>=0.11,<0.12' 'apache-beam[gcp]>=2.55,<3'
        pip3 install -e "$2"
        "#,
        &args,
        &options,
    )?;

    if code != 0 {
        error!("venv setup stdout: {}", output);
        error!("venv setup stderr: {}", error);
        return Err(anyhow::anyhow!(
            "detections venv setup failed (exit {})", code
        ));
    }
    info!("detections venv ready: {}", output.lines().last().unwrap_or(""));

    Ok(())
}

/// Compiles every `*.yml`/`*.yaml` under `<path>/detections/input/` into Python
/// detection modules under `<path>/detections/output/` via `sigma_generate.py`.
pub fn generate_detections(path_to_config: &Path) -> Result<()> {
    info!("converting sigma detections...");
    let path = path_to_config.join("detections");
    let output_path = vec![path.join("output").to_str().unwrap().to_string()];
    let options = ScriptOptions::new();

    let (_code, output, error) = run_script::run(
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

    if !error.is_empty() {
        error!("{}", error);
    } else {
        output.lines().for_each(|line| info!("{}", line));
    }


    Ok(())
}
