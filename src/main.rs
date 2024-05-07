mod commands;
mod lib;

use std::panic;
use std::panic::set_hook;
use anyhow::Result;
use clap::{self, Parser};
use log::error;

pub fn main() -> Result<()> {
    // logger config
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
        // .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;

    // on-panic -> preform default behavior & log error
    let default_panic_hook = panic::take_hook();
    set_hook(Box::new(move |info| {
        default_panic_hook(&info);
        error!("{:?}", info);
    }));

    // parse cli arguments
    commands::Args::parse().run()?;
    Ok(())
}

#[derive(thiserror::Error, Debug)]
#[error("miscellaneous resource creation error")]
enum MiscError {
    #[error("too many resource creation attempts")]
    MaxResourceCreationRetries,
}

