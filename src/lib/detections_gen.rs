use std::arch::aarch64::veor_s8;
use std::fs::File;
use std::io::{read_to_string, Write};
use std::path::Path;
use anyhow::Result;
use log4rs::init_file;
use log::{error, info};
use thiserror::Error;

use python_parser as pp;
use python_parser::ast;
use python_parser::ast::Statement::{Assignment, Compound};
use python_parser::ast::CompoundStatement::Funcdef;
use python_parser::ast::Expression::{Call, Name};
use python_parser::ast::{Argument, Decorator, Expression, Statement, TypedArgsList};
use crate::lib::detections_gen::ASTGenError::{DuplicateFunc, UnwrapAST};
use crate::lib::utilities::overlap;
// this code causes physical pain - sorry in advance


pub fn generate_detections_file(config_path: &Path) -> Result<()> {
    info!("generating aggregated detections...");
    let detections_path = config_path.join("detections");
    let mut template_file_ast: Vec<Statement> = pp::file_input(pp::make_strspan(&read_to_string(File::open(&detections_path.join("detections_template.py"))?)?)).unwrap().1;
    let detection_functions_definitions: Vec<Statement> = generate_detection_functions(&config_path)?;

    // compare (function names in template file) to (newly generated function's names) for overlap
    let main_func_names = get_func_names(&template_file_ast)?;
    let new_func_names: Vec<String> = get_func_names(&detection_functions_definitions)?;
    if overlap(&main_func_names, &new_func_names) { return Err(DuplicateFunc.into()) }

    // generate calls for (newly generated functions)
    let function_calls_ast: Vec<Statement> = new_func_names.iter()
        .map(|name|
            Assignment(vec![Call(Box::new(Name(name.clone())), vec![Argument::Positional(Expression::Name("record".to_string()))])], vec![])
        ).collect();

    // "detections function" gen ast for function to call all (newly generated functions)
    let detections_function =
        vec![Compound(Box::new(Funcdef(
            ast::Funcdef {
                r#async: false,
                decorators: vec![],
                name: "detections".to_string(),
                parameters: TypedArgsList {
                    posonly_args: vec![("record".to_string(), None, None)],
                    args: vec![],
                    star_args: Default::default(),
                    keyword_args: vec![],
                    star_kwargs: None,
                },
                return_type: None,
                code: function_calls_ast,
            }
        )))];

    // generate new 'run' function contents
    let mut run_function;
    match &template_file_ast[index(&template_file_ast, "run")?] {
       Compound(a) => match *a.clone() {
           Funcdef(function) => {
               let mut function_mut = function.clone();
               function_mut.code.splice(1..1, [detection_functions_definitions, detections_function].concat());
               run_function = Compound(Box::new(Funcdef(function_mut)))
           },
           _ => return Err(UnwrapAST.into()),
       }
        _ => return Err(UnwrapAST.into()),
    }


    let insert_index = index(&template_file_ast, "run")?;
    template_file_ast.splice(insert_index..=insert_index, vec![run_function]);

    serialize(config_path, &template_file_ast)?;

    Ok(())
}

fn serialize(config_path: &Path, main_file_ast: &Vec<Statement>) -> Result<()> {
    // deletes  artifacts/detections_gen.py if it exists; otherwise serializes ast into it
    let output_ast_str = python_parser::visitors::printer::format_module(&main_file_ast);
    let output_path = config_path.join("artifacts").join("detections_gen.py");

    if output_path.exists() { std::fs::remove_file(&output_path)?; }

    let mut output_file = std::fs::OpenOptions::new().write(true).create(true).open(output_path)?;
    output_file.write_all(&output_ast_str.into_bytes())?;
    Ok(())
}


fn generate_detection_functions(config_path: &Path) -> Result<Vec<Statement>> {
    /// Gets all <config>/detections/output/<detection>/detect.py file
    // Checks to make sure each detection folder contains detect.py file Check to ensure detect.py
    // contains detect func
    // // Check to ensure multiple detect dirs aren't named the same
    // Check to ensure (renamed) func does not already exist within detections_template.py

    /// Returns:
    ///     list of functions containing a detection (from detections dir) -> wraps each w/ Harness decorator

    let mut detections_funcs: Vec<Statement> = Vec::new();

    // iterate through folders in <config>/detections/output/
    let dirs = config_path.join("detections").join("output")
        .read_dir()?
        .map(|x| x.unwrap());

    for dir in dirs {
        let file_path = dir.path().join("detect.py");

        if !file_path.as_path().exists() {
            // err if detection dir lacks detect.py file
            return Err(ASTGenError::MissingDetectFile(dir.path().to_str().unwrap().to_string()).into());
        } else {
            // extract detect function

            let contents = read_to_string(File::open(&file_path)?)?;
            let mut input_ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;


            for statement in input_ast {
                if let Compound(a) = statement {
                    match *a {
                        Funcdef(ast::Funcdef {name, code, ..}) => {
                            if name != "detect" {
                                return Err(ASTGenError::MissingDetectFunction(file_path.to_str().unwrap().to_string()).into());
                            } else {
                                detections_funcs.push(Compound(Box::new(
                                    Funcdef(
                                        ast::Funcdef {
                                            r#async: false,
                                            decorators: vec![
                                                Decorator{ name: vec!["harness".into()], args: None}
                                            ],
                                            name: dir.file_name().to_str().unwrap().to_string(),
                                            parameters: TypedArgsList {
                                                posonly_args: vec![("record".to_string(), None, None)],
                                                args: vec![],
                                                star_args: Default::default(),
                                                keyword_args: vec![],
                                                star_kwargs: None,
                                            },
                                            return_type: None,
                                            code,
                                        }
                                    )
                                )));
                            }
                        },
                        _ => (),
                    }
                }
            }

            // detections_funcs.push( rename_detect_func(&file_path)? );
        }
    }

    Ok(detections_funcs)
}

fn get_func_names(ast: &[Statement]) -> Result<Vec<String>> {
    Ok(ast
        .iter()
        .filter_map(|a|
            if let Compound(b) = a {
                if let Funcdef(ast::Funcdef{name, ..}) = *b.clone() {
                    Some(name)
                } else { None }
            } else { None })
        .collect())
}


fn rename_detect_func(file_path: &Path) -> Result<Statement> {
    // gets detect func from a detections/<dir>/detect.py file -> returns function contents but named after its <dir>
    let input_file_name = file_path.file_name().unwrap().to_str().unwrap().to_string();
    let contents = read_to_string(File::open(&file_path)?)?;
    let mut input_ast = python_parser::file_input(python_parser::make_strspan(&contents)).unwrap().1;

    println!("{:?}", input_file_name);

    for statement in input_ast {
        if let Compound(a) = statement {
            if let Funcdef(b) = *a {
                if let ast::Funcdef { name, code, .. } = b {
                    if name != "detect" {
                        return Err(ASTGenError::MissingDetectFunction(file_path.to_str().unwrap().to_string()).into());
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

    return Err(ASTGenError::MiscASTGen.into());
}


fn index(ast: &[Statement], func_name: &str) -> Result<usize> {
    // get index of specified function index within Vec<Statements>
    Ok(ast.iter()
        .position(|a|
            if let Compound(b) = a {
                if let Funcdef(c) = *b.clone() {
                    if let ast::Funcdef{name, ..} = c {
                        name == func_name
                    } else { false }
                } else { false }
            } else { false }
        ).unwrap())
}


#[derive(Error, Debug)]
#[error("Dataflow Error")]
enum ASTGenError {
    #[error("Missing detect.py file within <config dir>detections/output/<generated dir>/")]
    MissingDetectFile(String),
    #[error("Detect.py file is missing a function named detect")]
    MissingDetectFunction(String),
    #[error("Error occurred while generating AST")]
    MiscASTGen,
    #[error("Function sharing a name w/ generated function already exists in detects.py template")]
    DuplicateFunc,
    #[error("error occured while unwrapping template ast")]
    UnwrapAST
}