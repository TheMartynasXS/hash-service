#![windows_subsystem = "windows"]
use tonic::transport::Server;

mod state;
use state::ServiceHashLoader;
pub use state::hash_service;
use state::hash_service::hash_loader_server::HashLoaderServer;

mod tray;
pub use tray::{Application, UserEvent};

use tray_icon::{TrayIconEvent, menu::MenuEvent};

use winit::event_loop::EventLoop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hash_loader = ServiceHashLoader::new();

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
        let _ = proxy.send_event(UserEvent::TrayIconEvent(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
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
