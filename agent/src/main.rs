use tonic::transport::Server;
use tonic::{Request, Response, Status};

use samwise_proto::agent_server::{Agent, AgentServer};
use samwise_proto::{
    PingRequest, PingResponse, RebootRequest, RebootResponse, ShutdownRequest, ShutdownResponse,
    SuspendRequest, SuspendResponse,
};

struct AgentImpl;

#[tonic::async_trait]
impl Agent for AgentImpl {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        println!("Got a request: {:?}", request);
        let reply = PingResponse {
            current_target: "linux".to_string(),
        };
        Ok(Response::new(reply))
    }

    async fn reboot(
        &self,
        request: Request<RebootRequest>,
    ) -> Result<Response<RebootResponse>, Status> {
        println!("TODO: reboot");
        Ok(Response::new(RebootResponse {}))
    }

    async fn shut_down(
        &self,
        request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        println!("TODO: shut down");
        Ok(Response::new(ShutdownResponse {}))
    }

    async fn suspend(
        &self,
        request: Request<SuspendRequest>,
    ) -> Result<Response<SuspendResponse>, Status> {
        println!("TODO: suspend");
        Ok(Response::new(SuspendResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:8673".parse()?;
    Server::builder()
        .add_service(AgentServer::new(AgentImpl))
        .serve(addr)
        .await?;

    Ok(())
}
