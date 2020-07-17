//! State machine for devices.

use std::convert::TryFrom;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use slog::{debug, error, warn, info, o, Logger};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time;
use tonic::transport::{Endpoint, Channel};

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
    ) -> Result<(DeviceHandle, JoinHandle<()>)> {
        let (tx, rx) = mpsc::channel(10);
        let logger = logger.new(o!("device" => id.clone()));
        let manager = DeviceManager {
            id: id.clone(),
            logger: logger.clone(),
            agent: AgentConnection::new(config.agent().to_string())?,
            current_state: DeviceState::Unknown,
            receiver: rx,
        };

        let manager_handle = tokio::spawn(async move {
            if let Err(e) = manager.run().await {
                error!(logger, "Device manager failed: {}", e);
            }
        });

        let handle = DeviceHandle { id, sender: tx };

        Ok((handle, manager_handle))
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
                command = self.receiver.recv() => match command {
                   Some(command) => self.process(command).await,
                   None => {
                       debug!(&self.logger, "Shutting down");
                       break;
                   }
                }
            }
        }

        Ok(())
    }

    async fn process(&mut self, command: Command) -> Result<()> {
        match command {
            Command::GetState(dest) => dest.send(self.current_state.clone()),
            Command::SetState(DesiredState::Running(new_target)) => {
                match &self.current_state {
                    s @ DeviceState::Unknown | DeviceState::ShuttingDown | DeviceState::Suspending => {
                        warn!(&self.logger, "{}, ignoring request to run {}", s, new_target);
                        Ok(())
                    },
                    DeviceState::Running { target: current_target } if current_target == Some(new_target) => {
                        debug!(&self.logger, "Already running desired target {}", new_target);
                        Ok(())
                    },
                    DeviceState::Starting { target: current_target } if current_target == new_target => {
                        debug!(&self.logger, "Already starting desired target {}", new_target);
                        Ok(())
                    },
                    DeviceState::Starting { target: current_target } => {
                        warn!(&self.logger, "Already starting {}, ignoring request to run {}", current_target, new_target);
                        Ok(())
                    },
                    DeviceState::Running { .. } => {
                        self.select_target(new_target).await?;
                        self.reboot().await?;
                        Ok(())
                    },
                    DeviceState::Inactive => {
                        self.select_target(new_target).await?;
                        self.boot().await?;
                        Ok(())
                    }
                }
            },
            Command::SetState(DesiredState::ShutDown) => {
                match &self.current_state {
                    DeviceState::Running { .. } =>
                }
            }
        }
    }

    /// Configures the desired target for this device, which it will use on next boot
    async fn select_target(&mut self, target: TargetId) -> Result<()> {
        debug!(&self.logger, "Selecting target {}", target);
        Ok(())
    }

    async fn reboot(&mut self) -> Result<()> {

    }

    async fn boot(&mut self) -> Result<()> {

    }

    async fn shutdown(&mut self) -> Result<()> {

    }

    /// Gets the current device state by pinging the agent
    async fn fetch_state(&mut self) -> DeviceState {
        match self.agent.ping().await {
            Ok(target) => DeviceState::Running {
                target: Some(target),
            },
            Err(_) => DeviceState::Inactive,
        }
    }

    // TODO: refactor into simpler state machine, block instead of using intermediate states?

    /*
     * Separate kinds of state:
     *    AgentStatus = < Active(target) | Inactive >
     *        - refreshed by pinging
     *    Lifecycle = < Unknown | Starting(target) | Running(target) | Off >
     *        - updated while handling commands + in response to pinging
     *        - what's reported to callers
     *    Command = < Run(target) | Suspend | ShutDown >
     *
     * Store Lifecycle in a mutex, possibly with a watch channel for posting updates so API endpoints can wait for new state
     * When handling a command, loop (with timeout) until desired AgentStatus
     *
     *
     * BETTER:
     *
     * - DeviceManager publishes state on a watch channel, doesn't store itself
     * - API layer can store most recently seen state to serve
     * - When handling a command, DeviceManager just asks agent when needed
     * - Command handling blocks until desired state or timeout (so no phantom intermediates)
     * - When no command, poll agent to detect out-of-band changes
     *
     * For example, to handle a Run(foo) command:
     * 1. If agent.ping() == Active(foo), return early
     * 2. Publish Starting(foo) state
     * 3. Update DHCP server
     * 4. Reboot via agent if active, otherwise boot via WoL
     * 5. Wait for agent.ping() to equal Active(foo)
     * 6. Publish Running(foo) state
     *
     * Can abstract DHCP/WoL out of state machine with a DeviceController or something:
     * - ping for current state
     * - reboot to a target (configure DHCP + reboot via agent)
     * - boot to a target (configure DHCP + WoL)
     * - suspend
     * - shut down
     *
     * Rename DeviceManager to something more state-machine-y?
     */
}

#[derive(Debug)]
enum Command {
    SetState(DesiredState),
    GetState(oneshot::Sender<DeviceState>),
}

/// gRPC Agent client that supports lazy/invalidated connections
struct AgentConnection {
    endpoint: Endpoint,
    client: Option<AgentClient<Channel>>,
}

impl AgentConnection {
    fn new(uri: String) -> Result<AgentConnection> {
        Ok(AgentConnection {
            endpoint: Endpoint::try_from(uri).context("Could not parse agent URI")?,
            client: None
        })
    }

    async fn client(&mut self) -> Result<&mut AgentClient<Channel>> {
        if let Some(ref mut client) = self.client {
            Ok(client)
        } else {
            let client = AgentClient::connect(self.endpoint.clone()).await?;
            self.client = Some(client);
            Ok(self.client.as_mut().unwrap())
        }
    }

    async fn ping(&mut self) -> Result<TargetId> {
        let req = tonic::Request::new(PingRequest {});
        let client = self.client().await.context("Could not connect to agent")?;
        let response = client.ping(req).await.context("Could not ping agent")?;
        Ok(TargetId::new(response.into_inner().current_target))
    }
}
