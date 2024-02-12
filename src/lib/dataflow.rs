use std::fs::File;
use std::io::read_to_string;
use std::path::Path;
use std::sync::WaitTimeoutResult;
use anyhow::Result;
use log::debug;
use python_parser::ast;
use python_parser::ast::CompoundStatement;
use python_parser::ast::CompoundStatement::Funcdef;
use python_parser::ast::Statement::Compound;
use thiserror::Error;


pub fn create_classic_template() -> Result<(), anyhow::Error> {


// python -m examples.mymodule \
// --runner DataflowRunner \
// --project PROJECT_ID \
// --staging_location gs://BUCKET_NAME/staging \
// --template_location gs://BUCKET_NAME/templates/TEMPLATE_NAME \
// --region REGION

    Ok(())
}


macro_rules! get_function_ast {
    ($ast: ident) => {
        $ast
            .iter()
            .filter(|a| match a {
                Compound(b) => match **b {
                    Funcdef(c) => match c {
                        ast::Funcdef{name, ..} => name == "detect",
                        _ => false,
                    },
                    _ => false,
                },
                _ => false
            })
            .collect::<Vec<_>>()[0];
    };
}

pub fn build_detection_file(config_path: &Path) -> Result<()> {
    let path = config_path.join("detections");

    for dir in path.join("output").read_dir()?.map(|x| x.unwrap()).take(1) {
        let file = dir.path().join("detect.py");

        if !file.as_path().exists() {
            return Err(DataflowError::MissingDetectFile(dir.path().to_str().unwrap().to_string()).into());
        } else {
            let contents = read_to_string(File::open(&file.as_path())?)?;
            let mut ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;
            // let mut func_ast = ast
            //     .iter()
            //     .filter(|a| match a {
            //         Compound(b) => match **b {
            //             Funcdef(c) => match c {
            //                 ast::Funcdef { name, .. } => name == "detect",
            //             },
            //             _ => false,
            //         },
            //         _ => false
            //     })
            //     .collect::<Vec<_>>();
            // if func_ast.len() != 1 { return Err(DataflowError::MissingDetectFunction(file.as_path().to_str().unwrap().to_string()).into()); }

            if let Compound(a) = ast {
                if let Funcdef(b) = *a {
                    if let ast::Funcdef {name, .. } = b {
                        if name != "detect" {
                            return Err(DataflowError::MissingDetectFunction(file.as_path().to_str().unwrap().to_string()).into());
                        } else {
                            name =
                        }
                    }
                }
            }
        }
    }


    Ok(())
}


#[derive(Error, Debug)]
#[error("Dataflow Error")]
enum DataflowError {
    #[error("Missing detect.py file within <config dir>detections/output/<generated dir>/")]
    MissingDetectFile(String),
    #[error("Detect.py file is missing a function named detect")]
    MissingDetectFunction(String),
}

