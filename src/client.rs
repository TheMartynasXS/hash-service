use hash_service::GetStringRequest;
use hash_service::hash_loader_client::HashLoaderClient;

pub mod hash_service {
    tonic::include_proto!("hashservice"); // The string specified here must match the proto package name
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = HashLoaderClient::connect("http://[::1]:50051").await?;

    // Example hash value - you can change this to test with different hashes
    let hash_value: u64 = 1234567890;
    let hashtable_type = "game"; // or "bin"

    let request = tonic::Request::new(GetStringRequest {
        hash: hash_value,
        hashtable_type: hashtable_type.to_string(),
    });

    let response = client.get_string(request).await?;
    let inner = response.into_inner();

    if inner.found {
        println!("Found string for hash {}: {}", hash_value, inner.value);
    } else {
        println!(
            "Hash {} not found in {} hashtable",
            hash_value, hashtable_type
        );
    }

    println!("Full response: {:?}", inner);

    Ok(())
}
