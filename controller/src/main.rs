#![feature(async_closure)]
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use slog::{debug, error, info, o, Drain, Logger};
use structopt::StructOpt;
use tokio::signal;
use tokio::sync::watch;

use crate::config::Configuration;
use crate::device::Device;
use crate::id::DeviceId;
use crate::shutdown::Shutdown;
use crate::wake::Waker;

mod agent;
mod device;
mod shutdown;
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
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl Samwise {
    fn new(logger: Logger, config: Configuration) -> Samwise {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
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

        signal::ctrl_c()
            .await
            .context("Error waiting for shutdown signal")?;
        self.shutdown().await?;

        Ok(())
    }

    async fn setup_devices(&mut self) -> Result<()> {
        for (id, device_config) in self.config.device_configs() {
            let (device, mut handler) = device::new_device(
                id.clone(),
                config,
                device_config,
                self.waker.clone(),
                Shutdown::new(self.shutdown_rx.clone()),
                &self.logger,
            )?;

            let logger = self.logger.clone();
            let id2 = id.clone();
            tokio::spawn(async move {
                if let Err(e) = handler.run().await {
                    error!(logger, "Device {} failed: {}", id2, e);
                }
            });
            self.devices.insert(id, device);
        }

        Ok(())
    }

    async fn shutdown(self) -> Result<()> {
        let Samwise {
            logger,
            mut shutdown_tx,
            shutdown_rx,
            ..
        } = self;

        drop(shutdown_rx); // Drop this so the closed().await below can complete

        info!(&logger, "Shutting down");
        shutdown_tx
            .broadcast(true)
            .context("Could not send shutdown signal")?;
        // Wait for all device handlers to drop their Shutdowns
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
