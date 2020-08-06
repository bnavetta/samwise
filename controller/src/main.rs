#![feature(async_closure)]
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use slog::{debug, info, o, Drain, Logger};
use structopt::StructOpt;
use tokio::signal;
use tokio::sync::watch;

use crate::config::Configuration;
use crate::device::{Device, Action};
use crate::id::{DeviceId, TargetId};
use crate::wake::Waker;

mod agent;
mod device;
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

pub struct Samwise {
    logger: Logger,
    config: Configuration,
    waker: Waker,
    devices: HashMap<DeviceId, Device>,
    shutdown_tx: watch::Sender<()>,
    shutdown_rx: watch::Receiver<()>,
}

impl Samwise {
    fn new(logger: Logger, config: Configuration) -> Samwise {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        Samwise {
            logger,
            config,
            devices: HashMap::new(),
            waker: Waker::new(),
            shutdown_tx,
            shutdown_rx,
        }
    }

    async fn run(mut self) -> Result<()> {
        info!(&self.logger, "Starting Samwise controller");
        self.setup_devices().await?;
        info!(&self.logger, "Controller started");

        for device in self.devices.values_mut() {
            device.action(Action::Run(TargetId::new("windows"))).await?;
        }

        signal::ctrl_c()
            .await
            .context("Error waiting for shutdown signal")?;
        self.shutdown().await?;

        Ok(())
    }

    async fn setup_devices(&mut self) -> Result<()> {
        for id in self.config.devices() {
            let device = Device::start(
                id.clone(),
                &self.config,
                self.waker.clone(),
                self.shutdown_rx.clone(),
                &self.logger,
            )?;

            self.devices.insert(id, device);
        }

        Ok(())
    }

    async fn shutdown(self) -> Result<()> {
        let Samwise {
            logger,
            devices,
            mut shutdown_tx,
            shutdown_rx,
            ..
        } = self;

        info!(&logger, "Shutting down");

        drop(shutdown_rx); // Otherwise the .closed() call below will never complete
        drop(devices); // This will close channels to devices and allow them to shut down

        // Wait for all spawned tasks to drop their shutdown receivers
        shutdown_tx.closed().await;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::from_args();
    let logger = create_logger();

    debug!(&logger, "Loading configuration"; "path" => args.config_path.display());
    let config = Configuration::load_file(&args.config_path).await?;

    let samwise = Samwise::new(logger, config);
    samwise.run().await
}
