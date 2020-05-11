use tonic::transport::Server;
use tonic::{Request, Response, Status};

use samwise_proto::agent_server::{Agent, AgentServer};
use samwise_proto::{PingRequest, PingResponse};

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
