//! State machine for devices.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use slog::{debug, error, info, o, Logger};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time;
use tonic::transport::Channel;

use samwise_proto::agent_client::AgentClient;
use samwise_proto::PingRequest;

use crate::config::DeviceConfiguration;
use crate::model::*;

// select! loop:
//    - poll agent for state
//    - on message to switch to a target:
//         - update DHCP server (this needs to block next steps)
//         - if running: tell agent to reboot
//         - if shut down: send WoL packet
//         - set state to Starting
//    - on message to shut down:
//         - if not already shut down, tell agent to shut down
//         - set state to ShuttingDown

#[derive(Clone)]
pub struct DeviceHandle {
    id: DeviceId,
    sender: mpsc::Sender<Command>,
}

impl DeviceHandle {
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    pub async fn current_state(&mut self) -> Result<DeviceState> {
        let (tx, rx) = oneshot::channel();
        match self.sender.send(Command::GetState(tx)).await {
            Ok(_) => Ok(rx.await.context("Could not fetch state")?),
            Err(_) => bail!("Could not fetch state, device manager may have died"),
        }
    }
}

/// State for background device manager task, which maintains the device state machine
pub struct DeviceManager {
    id: DeviceId,
    logger: Logger,
    agent: AgentConnection,
    current_state: DeviceState,
    receiver: mpsc::Receiver<Command>,
}

impl DeviceManager {
    /// Spawns a device manager for the given device. Returns a `DeviceHandle` for controlling it
    /// and a `JoinHandle` for the manager task.
    pub fn start(
        id: DeviceId,
        config: &DeviceConfiguration,
        logger: &Logger,
    ) -> (DeviceHandle, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(10);
        let logger = logger.new(o!("device" => id.clone()));
        let manager = DeviceManager {
            id: id.clone(),
            logger: logger.clone(),
            agent: AgentConnection::new(config.host().to_string()),
            current_state: DeviceState::Unknown,
            receiver: rx,
        };

        let manager_handle = tokio::spawn(async move {
            if let Err(e) = manager.run().await {
                error!(logger, "Device manager failed: {}", e);
            }
        });

        let handle = DeviceHandle { id, sender: tx };

        (handle, manager_handle)
    }

    /// Main loop for the device manager task
    async fn run(mut self) -> Result<()> {
        info!(&self.logger, "Starting device manager");

        // Poll for initial device state
        self.current_state = self.fetch_state().await;
        info!(&self.logger, "Initial device state: {}", self.current_state);

        // TODO: this should be configurable
        let mut refresh = time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = refresh.tick() => {
                    debug!(&self.logger, "Pinging agent for state");
                    self.current_state = self.fetch_state().await;
                    debug!(&self.logger, "New state: {}", self.current_state);
                },
                Some(command) = self.receiver.recv() => {
                    debug!(&self.logger, "Received command: {:?}", command);
                },
                else => {
                    debug!(&self.logger, "Shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Gets the current device state by pinging the agent
    async fn fetch_state(&mut self) -> DeviceState {
        match self.agent.ping().await {
            Ok(target) => DeviceState::Running {
                target: Some(target),
            },
            Err(_) => DeviceState::ShutDown,
        }
    }
}

#[derive(Debug)]
enum Command {
    TransitionState(DesiredState),
    GetState(oneshot::Sender<DeviceState>),
}

/// gRPC Agent client that supports lazy/invalidated connections
struct AgentConnection {
    host: String,
    client: Option<AgentClient<Channel>>,
}

impl AgentConnection {
    fn new(host: String) -> AgentConnection {
        AgentConnection { host, client: None }
    }

    // TODO: may need something more like with_client to invalidate on failure
    //       or maybe even lazy initialization isn't needed
    async fn client(&mut self) -> Result<AgentClient<Channel>> {
        // See tonic docs - cloning client-side channels is intentionally cheap
        match &self.client {
            Some(client) => Ok(client.clone()),
            None => {
                // TODO: make port not a magic number
                let client = AgentClient::connect(format!("http://{}:8673", self.host)).await?;
                self.client = Some(client.clone());
                Ok(client)
            }
        }
    }

    async fn ping(&mut self) -> Result<TargetId> {
        let req = tonic::Request::new(PingRequest {});
        let mut client = self.client().await.context("Could not connect to agent")?;
        let response = client.ping(req).await.context("Could not ping agent")?;
        Ok(TargetId::new(response.into_inner().current_target))
    }
}
