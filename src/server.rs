use tonic::{Request, Response, Status, transport::Server};

use directories_next::ProjectDirs;
use hash_service::hash_loader_server::{HashLoader, HashLoaderServer};
use hash_service::{
    AddHashRequest, AddHashResponse, GetStringRequest, GetStringResponse, LoadHashesRequest,
    LoadHashesResponse, UnloadHashesRequest, UnloadHashesResponse,
};
pub mod hash_service {
    tonic::include_proto!("hashservice");
}

mod tray;
pub use tray::{Application, UserEvent};

use tray_icon::{TrayIconEvent, menu::MenuEvent};

use std::{collections::HashMap, sync::Arc};
use winit::event_loop::EventLoop;

use std::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct MyHashLoader {
    game_hashes: Arc<RwLock<HashMap<u64, String>>>,
    bin_hashes: Arc<RwLock<HashMap<u64, String>>>,
}

#[tonic::async_trait]
impl HashLoader for MyHashLoader {
    async fn load_hashes(
        &self,
        request: Request<LoadHashesRequest>,
    ) -> Result<Response<LoadHashesResponse>, Status> {
        println!("load_hashes called: {:?}", request);

        // Call the private load_hashes method
        self.load_hashes_impl();

        // add 1234567890 as "test"
        self.game_hashes
            .write()
            .map_err(|_| Status::internal("Failed to lock hashtable"))?
            .insert(1234567890, "test".to_string());

        // Get the hashtable length for the response
        let hashtable = self
            .game_hashes
            .read()
            .map_err(|_| Status::internal("Failed to lock hashtable"))?;

        let count = hashtable.len() as i32;

        let response = LoadHashesResponse {
            success: true,
            message: format!("Hashtable loaded: {}!", count),
            count,
        };
        Ok(Response::new(response))
    }

    async fn get_string(
        &self,
        request: Request<GetStringRequest>,
    ) -> Result<Response<GetStringResponse>, Status> {
        let req = request.into_inner();
        println!(
            "get_string called for hash: {}, type: {}",
            req.hash, req.hashtable_type
        );

        let hashtable = match req.hashtable_type.as_str() {
            "game" => &self.game_hashes,
            "bin" => &self.bin_hashes,
            _ => {
                return Ok(Response::new(GetStringResponse {
                    found: false,
                    value: String::new(),
                }));
            }
        };

        let hashtable_guard = hashtable
            .read()
            .map_err(|_| Status::internal("Failed to lock hashtable"))?;

        let response = match hashtable_guard.get(&req.hash) {
            Some(value) => GetStringResponse {
                found: true,
                value: value.clone(),
            },
            None => GetStringResponse {
                found: false,
                value: String::new(),
            },
        };

        Ok(Response::new(response))
    }

    async fn unload_hashes(
        &self,
        request: Request<UnloadHashesRequest>,
    ) -> Result<Response<UnloadHashesResponse>, Status> {
        println!("unload_hashes called");

        // Placeholder implementation
        let response = UnloadHashesResponse {
            success: true,
            message: "unload_hashes - placeholder".to_string(),
        };
        Ok(Response::new(response))
    }

    async fn add_hash(
        &self,
        request: Request<AddHashRequest>,
    ) -> Result<Response<AddHashResponse>, Status> {
        let req = request.into_inner();
        println!(
            "add_hash called for hash: {}, value: {}, type: {}",
            req.hash, req.value, req.hashtable_type
        );

        // Placeholder implementation
        let response = AddHashResponse {
            success: true,
            message: "add_hash - placeholder".to_string(),
        };
        Ok(Response::new(response))
    }
}

impl MyHashLoader {
    pub fn new() -> Self {
        MyHashLoader {
            game_hashes: Arc::new(RwLock::new(HashMap::default())),
            bin_hashes: Arc::new(RwLock::new(HashMap::default())),
        }
    }

    fn load_hashes_impl(&self) {
        let project_dirs = ProjectDirs::from("com", "league-toolkit", "lol-hashes");
        println!("{:?}", project_dirs);

        if let Some(project_dirs) = project_dirs {
            create_project_dirs(project_dirs);
        } else {
            eprintln!("Failed to get project directories");
        }
    }
}

fn create_project_dirs(project_dirs: ProjectDirs) {
    let cache_dir = project_dirs.cache_dir();
    // check if directory exists
    if !cache_dir.exists() {
        // create directory
        if let Err(e) = std::fs::create_dir_all(cache_dir) {
            eprintln!("Failed to create cache directory: {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hash_loader = MyHashLoader::new();

    tokio::spawn(async move {
        let addr = "[::1]:50051".parse().expect("Failed to parse address");

        if let Err(e) = Server::builder()
            .add_service(HashLoaderServer::new(hash_loader))
            .serve(addr)
            .await
        {
            eprintln!("gRPC server error: {:?}", e);
        }
    });

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();

    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        proxy.send_event(UserEvent::TrayIconEvent(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        proxy.send_event(UserEvent::MenuEvent(event));
    }));

    let mut app = Application::new(); // No need to pass hash_loader

    #[cfg(target_os = "linux")]
    {
        gtk::init().unwrap();
        let _tray_icon = Application::new_tray_icon();
        gtk::main();
    }

    if let Err(err) = event_loop.run_app(&mut app) {
        println!("TrayIcon Error: {err:?}");
    }

    Ok(())
}
