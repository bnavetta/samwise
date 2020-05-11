use std::fs;
use std::path::PathBuf;

use actix::Actor;
use anyhow::{Context, Result};
use slog::{Drain, Logger, o, debug};
use structopt::StructOpt;

use crate::config::Configuration;
use crate::device_manager::DeviceManager;
use crate::model::*;

mod config;
mod model;
mod device_manager;

#[derive(StructOpt)]
#[structopt(
    name = "samwise-controller",
    about = "Multiboot control server for Samwise"
)]
pub struct Cli {
    #[structopt(long = "--config")]
    #[structopt(parse(from_os_str))]
    pub config_path: PathBuf,
}

fn create_logger() -> slog::Logger {
    let drain = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(drain)
        .use_local_timestamp()
        .build()
        .fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    Logger::root(drain, o!())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::from_args();
    let logger = create_logger();

    // Use the Tokio runtime instead of actix_rt because actix_rt doesn't support file I/O.
    let local_set = tokio::task::LocalSet::new();
    let system = actix::System::run_in_tokio("samwise", &local_set);

    debug!(&logger, "Loading configuration"; "path" => args.config_path.display());
    let config = Configuration::load_file(&args.config_path).await?;

    for device in config.devices() {
        let _ = DeviceManager::new(device, &logger).start();
    }

    local_set.run_until(system).await?;

    Ok(())
}
