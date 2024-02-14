use std::fs::File;
use std::io::read_to_string;
use std::path::Path;
use anyhow::Result;
use log::error;
use thiserror::Error;

use python_parser as pp;
use python_parser::ast;
use python_parser::ast::Statement::{Assignment, Compound};
use python_parser::ast::CompoundStatement::Funcdef;
use python_parser::ast::Expression::{Call, Name};
use python_parser::ast::{Expression, Statement};
use crate::main;


pub fn create_classic_template() -> Result<(), anyhow::Error> {


// python -m examples.mymodule \
// --runner DataflowRunner \
// --project PROJECT_ID \
// --staging_location gs://BUCKET_NAME/staging \
// --template_location gs://BUCKET_NAME/templates/TEMPLATE_NAME \
// --region REGION

    Ok(())
}


pub fn generate_detections_file(config_path: &Path) -> Result<()> {
    let detections_path = config_path.join("detections");
    let main_detections_file_path = detections_path.join("detections.py");

    let mut main_file_ast = pp::file_input(pp::make_strspan(
            &read_to_string(File::open(&main_detections_file_path)?)?
        )).unwrap().1;

    println!("{:?}", main_file_ast);


    // let main_funcs = get_functions_from_ast(&main_file_ast)?;
    // let main_func_names = main_funcs.iter().map(|ast::Funcdef{name, ..}| name ).collect();
    //
    let new_funcs: Vec<Statement> = get_detection_funcs(&config_path)?;

    let new_func_names = new_funcs.iter().filter_map(|Compound(comp)| if let Funcdef(func) = comp.clone() {
        Some(func)
    } else { None })


    // let new_func_names = new_funcs.iter().filter_map(|Compound(Funcdef(func))| match func.as_ref() {
    //     ast::Funcdef{name, ..} => Some(name),
    //     _ => None                                    /// broken
    // } ).collect();



    //
    // for n1 in main_func_names {
    //     for n2 in new_func_names {
    //         if n1 == n2 {
    //             return Err("Function sharing a name w/ generated function \
    //             already exists in detects.py template".into())
    //         }
    //     }
    // }
    //
    // let run_function_index = get_func_index(&main_file_ast, "run")?;
    // main_file_ast.splice(run_function_index..=run_function_index, new_funcs);

    let function_calls: Vec<Expression> = Vec::new();
    for name in new_func_names {
        let func_call = Assignment(vec![Call(Box::new(Name(name)), Vec::new())], vec![]);

    }


    // main_file_ast.
    // TODO:
    //  - insert all fetched functions into main ast ✅
    //  - insert function calls into batch function:
    //      https://docs.rs/python-parser/latest/python_parser/ast/enum.Expression.html
    //      Assignment([Call(Name("main"), [])], [])
    //  - change location of detections.py
    //  - create dataflow.rs + create ast.rs file
    //  -






    Ok(())
}

// fn get_func_index(ast: &[Statement], func_name: &str) -> Result<usize> {
//    Ok(ast.iter()
//         .position(|a|
//             if let Compound(b) = a {
//                 if let Funcdef(c) = *b.clone() {
//                     if let ast::Funcdef{name, ..} = c {
//                         name == func_name
//                     } else { false }
//                 } else { false }
//             } else { false }
//         ).unwrap())
// }
//
fn get_detection_funcs(config_path: &Path) -> Result<Vec<Statement>> {
    /// Gets all <config>/detections/output/<detection>/detect.py file
    // Checks to make sure each detection folder contains detect.py file Check to ensure detect.py
    // contains detect func
    // // Check to ensure multiple detect dirs aren't named the same
    // Check to ensure (renamed) func does not already exist within detections.py

    let mut detections_funcs: Vec<Statement> = Vec::new();

    // iterate through folders in <config>/detections/output/
    // TODO: remove .take(1)
    let dirs = config_path.join("detections").join("output")
        .read_dir()?
        .map(|x| x.unwrap())
        .take(1);

    for dir in dirs {
        let detect_file = dir.path().join("detect.py");

        if !detect_file.as_path().exists() {
            // err if detection dir lacks detect.py file
            return Err(DataflowError::MissingDetectFile(dir.path().to_str().unwrap().to_string()).into());
        } else {
            // extract detect function
            detections_funcs.push( rename_detect_func(&detect_file)? );
        }
    }

    // let output_ast = python_parser::visitors::printer::format_module(&renamed_func);

    Ok(detections_funcs)
}
//
// // fn get_functions_from_ast(input_ast: &[Statement]) -> Result<Vec<ast::Funcdef>> {
// // // fn get_functions_from_ast(file_path: &Path) -> Result<Vec<ast::Funcdef>> {
// //     // let contents = read_to_string(File::open(&file_path)?)?;
// //     // let mut input_ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;
// //
// //     input_ast
// //         .iter()
// //         .filter_map(|a|
// //             if let Compound(b) = a {
// //                 if let Funcdef(c) = b {
// //                     Some(c)
// //                 } else { None }
// //             } else { None })
// //         .collect()
// // }
//
//
fn rename_detect_func(file_path: &Path) -> Result<Statement> {
    let input_file_name = file_path.file_name().unwrap().to_str().unwrap().to_string();
    let contents = read_to_string(File::open(&file_path)?)?;
    let mut input_ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;

    println!("{:?}", input_file_name);

    for statement in input_ast {
        if let Compound(a) = statement {
            if let Funcdef(b) = *a {
                if let ast::Funcdef { name, code, .. } = b {
                    if name != "detect" {
                        return Err(DataflowError::MissingDetectFunction(file_path.to_str().unwrap().to_string()).into());
                    } else {
                        return Ok(
                            Compound(Box::new(
                                Funcdef(
                                    ast::Funcdef {
                                        r#async: false,
                                        decorators: vec![],
                                        name: input_file_name,
                                        parameters: Default::default(),
                                        return_type: None,
                                        code,
                                    }
                                )
                            ))
                        );
                    }
                }
            }
        }
    }

    return Err(DataflowError::MiscASTGen.into());
}


#[derive(Error, Debug)]
#[error("Dataflow Error")]
enum DataflowError {
    #[error("Missing detect.py file within <config dir>detections/output/<generated dir>/")]
    MissingDetectFile(String),
    #[error("Detect.py file is missing a function named detect")]
    MissingDetectFunction(String),
    #[error("Error occurred while generating AST")]
    MiscASTGen,
}

