#![allow(dead_code)]
#![allow(special_module_name)]

mod commands;
mod lib;

use std::panic;
use std::panic::set_hook;
use anyhow::Result;
use clap::{self, Parser};
use log::error;

pub fn main() -> Result<()> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("hyper", log::LevelFilter::Info)
        .chain(fern::log_file("output.log")?)
        .apply()?;

    let default_panic_hook = panic::take_hook();
    set_hook(Box::new(move |info| {
        default_panic_hook(info);
        error!("{:?}", info);
    }));

    commands::Args::parse().run()?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
#[error("miscellaneous resource creation error")]
enum MiscError {
    #[error("too many resource creation attempts")]
    MaxResourceCreationRetries,
}

