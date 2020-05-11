use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use slog::{debug, o, Drain, Logger};
use structopt::StructOpt;

use crate::config::Configuration;
use crate::device_manager::DeviceManager;
use crate::model::*;

mod config;
mod device_manager;
mod model;

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

    debug!(&logger, "Loading configuration"; "path" => args.config_path.display());
    let config = Configuration::load_file(&args.config_path).await?;

    let mut handles = vec![];

    for (device, config) in config.device_configs() {
        let (_, handle) = DeviceManager::start(device, config, &logger);
        handles.push(handle);
    }

    for handle in handles.into_iter() {
        let _ = handle.await?;
    }

    Ok(())
}
