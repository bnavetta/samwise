use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Error};
use itertools::Itertools;
use slog::{debug, error, info, o, warn, Drain, Logger};
use structopt::StructOpt;
use tokio::fs;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use samwise_proto::agent_server::{Agent, AgentServer};
use samwise_proto::{
    PingRequest, PingResponse, RebootRequest, RebootResponse, ShutdownRequest, ShutdownResponse,
    SuspendRequest, SuspendResponse,
};

mod config;

use config::AgentConfiguration;

#[derive(StructOpt)]
#[structopt(name = "samwise-agent", about = "Local agent for Samwise")]
struct Args {
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

struct AgentImpl {
    logger: Logger,
    config: AgentConfiguration,
}

impl AgentImpl {
    /// Start a command in the background. Fails if the command line is empty or starting the process fails, but does
    /// not wait for the process to complete.
    fn spawn(&self, command: &[String]) -> Result<(), Status> {
        if command.is_empty() {
            warn!(&self.logger, "Tried to run an empty command");
            return Err(Status::unimplemented("Command not provided"));
        }

        info!(&self.logger, "Running `{}`", command.iter().format(" "));

        // Using std::process::Command because both it and tokio's implementation start processes synchronously
        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);

        if let Err(error) = cmd.spawn() {
            error!(
                &self.logger,
                "Could not start `{}`: {:?}",
                command.iter().format(" "),
                error
            );
            Err(Status::internal("Spawning command failed"))
        } else {
            Ok(())
        }
    }
}

#[tonic::async_trait]
impl Agent for AgentImpl {
    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        debug!(&self.logger, "Got a ping request");
        let reply = PingResponse {
            current_target: self.config.target_name.clone(),
        };
        Ok(Response::new(reply))
    }

    async fn reboot(
        &self,
        _request: Request<RebootRequest>,
    ) -> Result<Response<RebootResponse>, Status> {
        info!(&self.logger, "Rebooting...");
        match self.config.reboot_command {
            Some(ref command) => self.spawn(command.as_slice())?,
            None => {
                warn!(&self.logger, "Reboot command not set");
                return Err(Status::unimplemented("Reboot command not set"));
            }
        }
        Ok(Response::new(RebootResponse {}))
    }

    async fn shut_down(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        info!(&self.logger, "Shutting down...");
        match self.config.shutdown_command {
            Some(ref command) => self.spawn(command.as_slice())?,
            None => {
                warn!(&self.logger, "Shutdown command not set");
                return Err(Status::unimplemented("Shutdown command not set"));
            }
        }
        Ok(Response::new(ShutdownResponse {}))
    }

    async fn suspend(
        &self,
        _request: Request<SuspendRequest>,
    ) -> Result<Response<SuspendResponse>, Status> {
        info!(&self.logger, "Suspending...");
        match self.config.suspend_command {
            Some(ref command) => self.spawn(command.as_slice())?,
            None => {
                warn!(&self.logger, "Suspend command not set");
                return Err(Status::unimplemented("Suspend command not set"));
            }
        }
        Ok(Response::new(SuspendResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::from_args();
    let logger = create_logger();

    debug!(&logger, "Loading configuration"; "path" => args.config_path.display());
    let config_str = fs::read_to_string(&args.config_path)
        .await
        .with_context(|| format!("Could not read config from {}", args.config_path.display()))?;
    let config: AgentConfiguration = toml::from_str(&config_str)
        .with_context(|| format!("Invalid config file {}", args.config_path.display()))?;

    let addr = config.listen_address.parse()?;
    Server::builder()
        .add_service(AgentServer::new(AgentImpl { logger, config }))
        .serve(addr)
        .await?;

    Ok(())
}
