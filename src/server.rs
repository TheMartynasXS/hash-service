use tonic::{Request, Response, Status, transport::Server};

use hash_service::hash_loader_server::{HashLoader, HashLoaderServer};
use hash_service::{LoadRequest, LoadResponse};

pub mod hash_service {
    tonic::include_proto!("hashservice"); // The string specified here must match the proto package name
}

#[derive(Debug, Default)]
pub struct MyHashLoader {}

#[tonic::async_trait]
impl HashLoader for MyHashLoader {
    async fn load(
        &self,
        request: Request<LoadRequest>, // Accept request of type LoadRequest
    ) -> Result<Response<LoadResponse>, Status> {
        println!("Got a request: {:?}", request);

        let response = LoadResponse {
            message: format!("Hello {}!", request.into_inner().name).into(),
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let hash_loader = MyHashLoader::default();

    Server::builder()
        .add_service(HashLoaderServer::new(hash_loader))
        .serve(addr)
        .await?;

    Ok(())
}
