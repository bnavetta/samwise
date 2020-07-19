use anyhow::{Context, Result};
use slog::{o, trace, Logger};
use tonic::transport::{Channel, Endpoint};

use samwise_proto::agent_client::AgentClient;
use samwise_proto::{PingRequest, RebootRequest, ShutdownRequest, SuspendRequest};

use crate::id::TargetId;

pub enum AgentStatus {
    Active(TargetId),
    Inactive,
}

// Cloning Tonic `Channel`s is cheap and encouraged, so cloning `AgentConnection` is as well

#[derive(Clone)]
pub struct AgentConnection {
    logger: Logger,
    client: AgentClient<Channel>,
}

impl AgentConnection {
    pub fn new(uri: String, logger: &Logger) -> Result<AgentConnection> {
        let endpoint = Endpoint::from_shared(uri.clone()).context("Malformed agent address")?;
        // TODO: configure timeout, keepalive, TLS

        let channel = endpoint.connect_lazy()?;
        let client = AgentClient::new(channel);

        Ok(AgentConnection {
            logger: logger.new(o!("agent" => uri)),
            client,
        })
    }
    pub async fn ping(&mut self) -> AgentStatus {
        let req = tonic::Request::new(PingRequest {});

        let ping_response = self.client.ping(req).await;
        match ping_response {
            Ok(response) => {
                let target_id = TargetId::new(response.into_inner().current_target);
                AgentStatus::Active(target_id)
            }
            Err(error) => {
                trace!(&self.logger, "Pinging agent failed: {}", error);
                AgentStatus::Inactive
            }
        }
    }

    pub async fn reboot(&mut self) -> Result<()> {
        let req = tonic::Request::new(RebootRequest {});
        self.client
            .reboot(req)
            .await
            .context("Rebooting via agent failed")?;
        Ok(())
    }

    pub async fn suspend(&mut self) -> Result<()> {
        let req = tonic::Request::new(SuspendRequest {});
        self.client
            .suspend(req)
            .await
            .context("Suspending via agent failed")?;
        Ok(())
    }

    pub async fn shut_down(&mut self) -> Result<()> {
        let req = tonic::Request::new(ShutdownRequest {});
        self.client
            .shut_down(req)
            .await
            .context("Shutting down via agent failed")?;
        Ok(())
    }
}
