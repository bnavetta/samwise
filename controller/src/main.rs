#![feature(async_closure)]
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use slog::{debug, o, Drain, Logger};
use structopt::StructOpt;

use crate::config::Configuration;
use crate::device::Device;
use crate::id::DeviceId;
use crate::wake::Waker;

mod agent;
mod device;
mod server;
mod wake;

mod config;

mod id;

#[derive(StructOpt)]
#[structopt(
    name = "samwise-controller",
    about = "Multiboot control server for Samwise"
)]
pub struct Args {
    #[structopt(long = "--config")]
    #[structopt(parse(from_os_str))]
    pub config_path: PathBuf,
}

fn create_logger() -> Logger {
    let drain = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(drain)
        .use_local_timestamp()
        .build()
        .fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    Logger::root(drain, o!())
}

/// Starts a background task for each configured device, returning a map of device handles
fn start_devices(logger: &Logger, config: &Configuration) -> Result<HashMap<DeviceId, Device>> {
    let waker = Waker::new();
    let mut devices = HashMap::new();
    for id in config.devices() {
        let device = Device::start(id.clone(), &config, waker.clone(), logger)?;

        devices.insert(id, device);
    }
    Ok(devices)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::from_args();
    let logger = create_logger();

    debug!(&logger, "Loading configuration"; "path" => args.config_path.display());
    let config = Configuration::load_file(&args.config_path).await?;

    let devices = Arc::new(start_devices(&logger, &config)?);

    server::serve(logger.clone(), devices, config.listen_address()).await;
    Ok(())
}
