use std::fs::File;
use std::io::read_to_string;
use std::path::Path;
use std::ptr::write;
use std::sync::WaitTimeoutResult;
use anyhow::Result;
use log::debug;
use python_parser::ast;
use python_parser::ast::{CompoundStatement, Statement};
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


pub fn build_detection_file(config_path: &Path) -> Result<()> {
    let path = config_path.join("detections");

    for dir in path.join("output").read_dir()?.map(|x| x.unwrap()).take(1) {
        let file = dir.path().join("detect.py");

        if !file.as_path().exists() {
            return Err(DataflowError::MissingDetectFile(dir.path().to_str().unwrap().to_string()).into());
        } else {
            let input_ast = generate_ast(&file)?;
            let output_ast = python_parser::visitors::printer::format_module(&input_ast);


        }
    }


    Ok(())
}


fn generate_ast(file_path: &Path) -> Result<Vec<Statement>> {
    let name = file_path.file_name().unwrap().to_str().unwrap().to_string();
    let contents = read_to_string(File::open(&file_path)?)?;
    let mut input_ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;

    for statement in input_ast {
        if let Compound(a) = statement {
            if let Funcdef(b) = *a {
                if let ast::Funcdef {name, code, .. } = b {

                    if name != "detect" {
                        return Err(DataflowError::MissingDetectFunction(file_path.to_str().unwrap().to_string()).into());
                    } else {
                        return Ok(Vec::from_iter([
                            Compound(Box::new(
                                Funcdef(
                                    ast::Funcdef {
                                        r#async: false,
                                        decorators: vec![],
                                        name,
                                        parameters: Default::default(),
                                        return_type: None,
                                        code,
                                    }
                                )
                            ))
                        ]));
                    }

                }
            }
        }
    }

    return Err(DataflowError::MiscASTGen.into())
}


#[derive(Error, Debug)]
#[error("Dataflow Error")]
enum DataflowError {
    #[error("Missing detect.py file within <config dir>detections/output/<generated dir>/")]
    MissingDetectFile(String),
    #[error("Detect.py file is missing a function named detect")]
    MissingDetectFunction(String),
    #[error("Error occurred while generating AST")]
    MiscASTGen

}

