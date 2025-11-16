use tonic::Request;
use tonic::transport::Channel;

use hash_service::hash_loader_client::HashLoaderClient;
use hash_service::{AddHashRequest, GetStringRequest, GetStringResponse, UnloadHashesRequest};

pub mod hash_service {
    tonic::include_proto!("hashservice");
}

fn hex_to_u64(hex: &str) -> Result<u64, String> {
    let s = hex.trim_start_matches("0x");
    u64::from_str_radix(s, 16).map_err(|e| format!("invalid hex '{}': {}", hex, e))
}

async fn create_client(
    address: &str,
) -> Result<HashLoaderClient<Channel>, Box<dyn std::error::Error>> {
    Ok(HashLoaderClient::connect(address.to_string()).await?)
}

async fn rpc_get_string(
    client: &mut HashLoaderClient<Channel>,
    hash_hex: &str,
    hashtable_type: &str,
) -> Result<GetStringResponse, tonic::Status> {
    let hash = hex_to_u64(hash_hex).map_err(|e| tonic::Status::invalid_argument(e))?;
    let req = Request::new(GetStringRequest {
        hash,
        hashtable_type: hashtable_type.to_string(),
    });
    let resp = client.get_string(req).await?;
    Ok(resp.into_inner())
}

async fn rpc_add_hash(
    client: &mut HashLoaderClient<Channel>,
    string_value: &str,
    hashtable_type: &str,
) -> Result<hash_service::AddHashResponse, tonic::Status> {
    let req = Request::new(AddHashRequest {
        string: string_value.to_string(),
        hashtable_type: hashtable_type.to_string(),
    });
    let resp = client.add_hash(req).await?;
    Ok(resp.into_inner())
}

async fn rpc_unload_hashes(
    client: &mut HashLoaderClient<Channel>,
) -> Result<hash_service::UnloadHashesResponse, tonic::Status> {
    let req = Request::new(UnloadHashesRequest {});
    let resp = client.unload_hashes(req).await?;
    Ok(resp.into_inner())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "http://[::1]:50051";
    let mut client = create_client(addr).await?;

    let example_hash = "a7cf5b14b9b659e0";
    let string_to_hash = "data/characters/sru_es_bannerplatform_order/skins/skin33.bin";
    let hashtable_type = "game";

    println!("Getting string for hash {}...", example_hash);
    match rpc_get_string(&mut client, example_hash, hashtable_type).await {
        Ok(inner) => {
            if inner.found {
                println!("Found: {}", inner.value);
            } else {
                println!("Not found");
            }
            println!("Full response: {:?}", inner);
        }
        Err(e) => eprintln!("GetString error: {}", e),
    }

    println!("\nAdding new hash entry...");
    match rpc_add_hash(&mut client, string_to_hash, hashtable_type).await {
        Ok(inner) => println!(
            "AddHash response: success={}, message={}",
            inner.success, inner.message
        ),
        Err(e) => eprintln!("AddHash error: {}", e),
    }

    println!("\nVerifying insertion by fetching again...");
    match rpc_get_string(&mut client, example_hash, hashtable_type).await {
        Ok(inner) => {
            if inner.found {
                println!("Found after add: {}", inner.value);
            } else {
                println!("Still not found after add");
            }
        }
        Err(e) => eprintln!("GetString error: {}", e),
    }

    // Example of unloading if needed:
    // match rpc_unload_hashes(&mut client).await {
    //     Ok(inner) => println!("Unload response: success={}, message={}", inner.success, inner.message),
    //     Err(e) => eprintln!("Unload error: {}", e),
    // }

    Ok(())
}
