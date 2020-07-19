use core::fmt;

use anyhow::{anyhow, bail, Context, Result};
use pnet::util::MacAddr;
use slog::{debug, error, o, trace, Logger};
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time;
use tokio::time::Duration;

use crate::agent::{AgentConnection, AgentStatus};
use crate::config::{Configuration, DeviceConfiguration};
use crate::id::{DeviceId, TargetId};
use crate::wake::Waker;

// Device structure:
// - For each device, there are two tasks and 1+ (cheaply clonable) handles
// - One task periodically pings the agent for updates, sending them to a watch channel
// - One task responds to commands (to ensure that only one command is processed at a time)
// - The handle can pull state updates and send commands
// - When all handles have been dropped, the background tasks automatically terminate

/// Frequency at which to ping the agent for state changes
const PING_INTERVAL: Duration = Duration::from_secs(5);

/// Timeout when waiting for the device to complete an action
const ACTION_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone)]
pub struct Device {
    id: DeviceId,
    state_rx: watch::Receiver<State>,
    action_tx: mpsc::Sender<Action>,
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

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Action::Reboot => f.write_str("reboot"),
            Action::Suspend => f.write_str("suspend"),
            Action::ShutDown => f.write_str("shut down"),
            Action::Run(target) => write!(f, "run {}", target),
        }
    }
}

/// Task which polls the agent service on a device to detect state changes.
async fn state_poller(
    logger: Logger,
    mut agent: AgentConnection,
    mut state_tx: watch::Sender<State>,
    shutdown_rx: watch::Receiver<()>,
) {
    // TODO: may want to make this configurable
    let mut tick = time::interval(PING_INTERVAL);

    loop {
        tokio::select! {
            _ = state_tx.closed() => break,
            _ = tick.tick() => {
                let state = match agent.ping().await {
                    AgentStatus::Active(target) => State::Running(target),
                    AgentStatus::Inactive => State::Off,
                };

                // SendError from a watch channel also means it's closed
                if let Err(_) = state_tx.broadcast(state) {
                    break;
                }
            }
        }
    }

    trace!(&logger, "Closing state poller");
    drop(shutdown_rx); // Signal that we're done
}

struct Handler {
    id: DeviceId,
    logger: Logger,
    agent: AgentConnection,

    mac_address: MacAddr,
    network_interface: String,
    waker: Waker,

    state_rx: watch::Receiver<State>,
    action_rx: mpsc::Receiver<Action>,

    /// Receiver for signaling that this handler task has exited. When the Handler is dropped, the
    /// channel will automatically be closed
    shutdown_rx: watch::Receiver<()>,
}

impl Handler {
    async fn process(&mut self) -> Result<()> {
        loop {
            match self.action_rx.recv().await {
                Some(action) => {
                    let result = match action {
                        Action::Run(ref target) => self.handle_run(target).await,
                        Action::Reboot => self.handle_reboot().await,
                        Action::Suspend => self.handle_suspend().await,
                        Action::ShutDown => self.handle_shutdown().await,
                    };

                    if let Err(error) = result {
                        error!(&self.logger, "Handling {} action failed: {}", action, error);
                    }
                }
                None => {
                    // If action_rx returns None, all sending handles have been dropped and the
                    // processing loop can end.
                    break;
                }
            }
        }

        trace!(&self.logger, "Closing action handler");
        Ok(())
    }

    // When handling an action, ping initially to make sure we're acting on up-to-date state. When
    // looping to wait for an action to finish after that, always use self.state_rx to avoid spamming
    // the agent with pings.

    /// Handles a `Run` action.
    async fn handle_run(&mut self, target: &TargetId) -> Result<()> {
        debug!(&self.logger, "Told to run {}", target);
        match self.agent.ping().await {
            AgentStatus::Active(ref active_target) => {
                if active_target == target {
                    debug!(&self.logger, "Already running {}", target);
                    Ok(())
                } else {
                    debug!(
                        &self.logger,
                        "Running {}, but {} requested - will reboot", active_target, target
                    );
                    unimplemented!("TODO: reconfigure");
                    self.agent.reboot().await?;
                    self.await_running_target(target).await
                }
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Not running - will boot");
                self.boot().await?;
                self.await_running_target(target).await
            }
        }
    }

    /// Handles a `Reboot` action.
    async fn handle_reboot(&mut self) -> Result<()> {
        debug!(&self.logger, "Told to reboot");
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Rebooting to {}", target);
                self.agent.reboot().await?;
                self.await_running_target(&target).await
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Not running - will boot");
                self.boot().await?;
                // Can't wait for a specific target since we don't know what was running previously
                self.await_running().await
            }
        }
    }

    /// Handles a `Suspend` action.
    async fn handle_suspend(&mut self) -> Result<()> {
        debug!(&self.logger, "Told to suspend");
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Running {} - will suspend", target);
                self.agent.suspend().await?;
                self.await_off().await
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Already off or suspended");
                Ok(())
            }
        }
    }

    /// Handles a `ShutDown` action.
    async fn handle_shutdown(&mut self) -> Result<()> {
        debug!(&self.logger, "Told to shut down");
        match self.agent.ping().await {
            AgentStatus::Active(target) => {
                debug!(&self.logger, "Running {} - will shut down", target);
                self.agent.shut_down().await?;
                self.await_off().await
            }
            AgentStatus::Inactive => {
                debug!(&self.logger, "Already off or suspended");
                Ok(())
            }
        }
    }

    /// Boot the device via Wake-on-LAN.
    async fn boot(&mut self) -> Result<()> {
        self.waker
            .wake(self.network_interface.clone(), self.mac_address)
            .await
            .with_context(|| format!("Could not wake {}", self.id))
    }

    /// Waits for the device to be running a particular target.
    async fn await_running_target(&self, target: &TargetId) -> Result<()> {
        self.await_state(|state| match state {
            State::Running(ref current_target) => current_target == target,
            _ => false,
        })
        .await
    }

    /// Waits for the device to be in any running state.
    async fn await_running(&self) -> Result<()> {
        self.await_state(|state| match state {
            State::Running(_) => true,
            _ => false,
        })
        .await
    }

    /// Waits for the device to be off or suspended.
    async fn await_off(&self) -> Result<()> {
        self.await_state(|state| state == &State::Off).await
    }

    /// Waits for the device to be in a given state. Fails if the state-polling task exits in the
    /// background or the device takes too long to reach the desired state.
    async fn await_state<F>(&self, pred: F) -> Result<()>
    where
        F: Fn(&State) -> bool,
    {
        let mut state_rx = self.state_rx.clone();
        time::timeout(ACTION_TIMEOUT, async {
            // Check if the device is already in the desired state before looping, since recv() will
            // only yield any given state change once
            if pred(&*state_rx.borrow()) {
                Ok(())
            } else {
                loop {
                    match state_rx.recv().await {
                        Some(ref current_state) => {
                            if pred(current_state) {
                                break Ok(());
                            }
                        }
                        // If the state update channel closed, we'll never get notified for the desired state
                        None => break Err(anyhow!("State channel closed")),
                    }
                }
            }
        })
        .await
        .context("Timed out waiting for device to reach desired state")?
    }
}

impl Device {
    pub fn start(
        id: DeviceId,
        config: &Configuration,
        waker: Waker,
        shutdown_rx: watch::Receiver<()>,
        logger: &Logger,
    ) -> Result<Device> {
        let device_config = match config.device_config(&id) {
            Some(config) => config,
            None => bail!("Missing configuration for {}", id),
        };

        let logger = logger.new(o!("device" => id.clone()));
        let agent = AgentConnection::new(device_config.agent().to_string(), &logger)
            .with_context(|| format!("Bad agent for device {}", id))?;

        let (state_tx, state_rx) = watch::channel(State::Unknown);
        let (action_tx, action_rx) = mpsc::channel(1);

        let state_logger = logger.clone();
        let state_agent = agent.clone();
        let state_shutdown_rx = shutdown_rx.clone();
        let _ = tokio::spawn(state_poller(
            state_logger,
            state_agent,
            state_tx,
            state_shutdown_rx,
        ));

        let mut handler = Handler {
            id: id.clone(),
            logger,
            agent,
            mac_address: device_config.mac_address(),
            network_interface: device_config
                .interface()
                .unwrap_or(config.default_interface())
                .to_string(),
            waker,
            state_rx: state_rx.clone(),
            action_rx,
            shutdown_rx,
        };

        let _ = tokio::spawn(async move {
            if let Err(e) = handler.process().await {
                error!(handler.logger, "Handler failed: {}", e);
            }
        });

        Ok(Device {
            id,
            state_rx,
            action_tx,
        })
    }

    /// Tells the device to perform an action.
    pub async fn action(&mut self, action: Action) -> Result<()> {
        self.action_tx
            .send(action)
            .await
            .with_context(|| format!("Could not send action to device {:?}", self.id))
    }

    /// The most recent observed state of this device.
    pub fn latest_state(&self) -> State {
        self.state_rx.borrow().clone()
    }

    /// Poll for state updates.
    pub async fn recv_state(&mut self) -> Result<State> {
        if let Some(state) = self.state_rx.recv().await {
            Ok(state)
        } else {
            bail!("State channel closed");
        }
    }
}
