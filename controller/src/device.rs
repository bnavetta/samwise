use anyhow::{Context, Result};
use slog::{debug, o, Logger};
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time;
use tokio::time::Duration;

use crate::agent::{AgentConnection, AgentStatus};
use crate::config::{Configuration, DeviceConfiguration};
use crate::id::{DeviceId, TargetId};
use crate::shutdown::Shutdown;
use crate::wake::Waker;
use pnet::util::MacAddr;

#[derive(Clone)]
pub struct Device {
    id: DeviceId,
    latest_state: State,
    state_rx: watch::Receiver<State>,
    action_tx: mpsc::Sender<Action>,
}

/// Per-device handler
pub struct DeviceHandler {
    id: DeviceId,
    logger: Logger,

    agent: AgentConnection,

    mac_addr: MacAddr,
    network_interface: String,
    waker: Waker,

    shutdown: Shutdown,
    state_tx: watch::Sender<State>,
    action_rx: mpsc::Receiver<Action>,
}

/// Current state of a device.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum State {
    Unknown,
    Running(TargetId),
    Off,
}

/// Command representing the desired state of a device.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Action {
    Reboot,
    Suspend,
    ShutDown,
    Run(TargetId),
}

pub fn new_device(
    id: DeviceId,
    config: &Configuration,
    device_config: &DeviceConfiguration,
    waker: Waker,
    shutdown: Shutdown,
    logger: &Logger,
) -> Result<(Device, DeviceHandler)> {
    let logger = logger.new(o!("device" => id.clone()));
    let agent = AgentConnection::new(device_config.agent().to_string(), &logger)?;

    let (state_tx, state_rx) = watch::channel(State::Unknown);
    let (action_tx, action_rx) = mpsc::channel(1);

    let network_interface = device_config
        .interface()
        .unwrap_or(config.default_interface());

    let handler = DeviceHandler {
        id: id.clone(),
        logger,
        agent,
        waker,
        network_interface: network_interface.to_string(),
        mac_addr: device_config.mac_address(),
        shutdown,
        state_tx,
        action_rx,
    };

    let device = Device {
        id,
        latest_state: State::Unknown,
        state_rx,
        action_tx,
    };

    Ok((device, handler))
}

impl Device {
    /// Tells the device to perform an action.
    pub async fn action(&mut self, action: Action) -> Result<()> {
        self.action_tx
            .send(action)
            .await
            .with_context(|| format!("Could not send action to device {:?}", self.id))
    }

    /// The most recent observed state of this device.
    pub fn latest_state(&self) -> &State {
        &self.latest_state
    }

    /// Poll for state updates.
    pub async fn recv_state(&mut self) {
        if let Some(state) = self.state_rx.recv().await {
            self.latest_state = state
        }
    }
}

impl DeviceHandler {
    pub async fn run(&mut self) -> Result<()> {
        debug!(&self.logger, "Starting device handler");

        // Broadcast the device's initial state
        self.poll_state().await?;

        let mut poll = time::interval(Duration::from_secs(5));

        while !self.shutdown.is_shutdown() {
            let action = tokio::select! {
                action = self.action_rx.recv() => action,
                _ = poll.tick() => {
                    self.poll_state().await?;
                    None
                },
                _ = self.shutdown.recv() => None
            };

            if let Some(action) = action {
                self.handle(action).await?;
            }
        }

        Ok(())
    }

    async fn handle(&mut self, action: Action) -> Result<()> {
        debug!(&self.logger, "Handling action {:?}", action);
        match action {
            Action::Run(target) => self.handle_run(target).await?,
            Action::Reboot => self.handle_reboot().await?,
            Action::Suspend => self.handle_suspend().await?,
            Action::ShutDown => self.handle_shutdown().await?,
        }

        Ok(())
    }

    async fn handle_run(&mut self, target: TargetId) -> Result<()> {
        match self.agent.ping().await {
            AgentStatus::Active(active) if active == target => {
                debug!(&self.logger, "Already running {}", active);
            }
            AgentStatus::Active(other) => {
                debug!(&self.logger, "Running {}, but {} requested", other, target);
                // TODO: configure
                // TODO: reboot
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Not currently running");
                // TODO: configure
                // TODO: boot
            }
        }

        Ok(())
    }

    async fn handle_reboot(&mut self) -> Result<()> {
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Rebooting to {}", target);
                // TODO: reboot
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Booting from inactive state");
                // TODO: boot
            }
        }
        Ok(())
    }

    async fn handle_suspend(&mut self) -> Result<()> {
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Suspending from {}", target);
                // TODO: suspend
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Already inactive");
            }
        }
        Ok(())
    }

    async fn handle_shutdown(&mut self) -> Result<()> {
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Shutting down from {}", target);
                // TODO: shut down
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Already inactive");
            }
        }
        Ok(())
    }

    async fn boot(&mut self) -> Result<()> {
        debug!(&self.logger, "Booting device");
        self.waker.wake(self.config.interface())
    }

    async fn poll_state(&mut self) -> Result<()> {
        debug!(&self.logger, "Polling agent to update state");
        match self.agent.ping().await {
            AgentStatus::Active(target) => self.publish_state(State::Running(target))?,
            AgentStatus::Inactive => self.publish_state(State::Off)?,
        }
        Ok(())
    }

    fn publish_state(&mut self, state: State) -> Result<()> {
        debug!(&self.logger, "Entering state {:?}", state);
        self.state_tx
            .broadcast(state)
            .context("Could not publish device state")?;
        Ok(())
    }
}
