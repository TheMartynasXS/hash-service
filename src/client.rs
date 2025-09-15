use hash_service::LoadRequest;
use hash_service::hash_loader_client::HashLoaderClient;

pub mod hash_service {
    tonic::include_proto!("hashservice"); // The string specified here must match the proto package name
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = HashLoaderClient::connect("http://[::1]:50051").await?;

    let request = tonic::Request::new(LoadRequest {
        name: "Tonic".into(),
    });

    let response = client.load(request).await?;

    println!("RESPONSE={:?}", response);

    Ok(())
}
