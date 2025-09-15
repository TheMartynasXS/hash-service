use tonic::{Request, Response, Status, transport::Server};

use hash_service::hash_loader_server::{HashLoader, HashLoaderServer};
use hash_service::{LoadRequest, LoadResponse};

pub mod hash_service {
    tonic::include_proto!("hashservice");
}

// Minimal tray setup
use std::path::Path;
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

#[derive(Debug, Default)]
pub struct MyHashLoader {}

#[tonic::async_trait]
impl HashLoader for MyHashLoader {
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        println!("Got a request: {:?}", request);

        let response = LoadResponse {
            message: format!("Hello {}!", request.into_inner().name),
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let icon_path = concat!(env!("CARGO_MANIFEST_DIR"), "/icon.png");
    let icon = {
        let image = image::open(Path::new(icon_path))
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        tray_icon::Icon::from_rgba(rgba, width, height).expect("Failed to create icon")
    };

    std::thread::spawn(move || {
        let menu = Menu::new();
        menu.append(&MenuItem::new("Quit", true, None)).unwrap();

        #[cfg(target_os = "linux")]
        gtk::init().unwrap();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("system-tray - tray icon library!")
            .with_icon(icon)
            .build()
            .unwrap();

        #[cfg(target_os = "linux")]
        gtk::main();
    });

    std::thread::spawn(move || {
        println!("listening...");
        let receiver = MenuEvent::receiver();
        loop {
            match receiver.recv() {
                Ok(event) => {
                    println!("{:?}", event);
                }
                Err(e) => {
                    eprintln!("Tray event receiver error: {:?}", e);
                    break;
                }
            }
        }
    });

    let addr = "[::1]:50051".parse()?;
    let hash_loader = MyHashLoader::default();

    Server::builder()
        .add_service(HashLoaderServer::new(hash_loader))
        .serve(addr)
        .await?;

    Ok(())
}
