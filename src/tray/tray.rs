use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

use winit::application::ApplicationHandler;

use crate::hash_service::hash_loader_client::HashLoaderClient;
use crate::hash_service::{LoadHashesRequest, UnloadHashesRequest};

#[derive(Debug)]
pub enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

pub struct Application {
    tray_icon: Option<TrayIcon>,
}

impl Application {
    pub fn new() -> Application {
        Application { tray_icon: None }
    }

    pub fn new_tray_icon() -> TrayIcon {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/regular.png");
        let icon = load_icon(std::path::Path::new(path));

        TrayIconBuilder::new()
            .with_menu(Box::new(Self::new_tray_menu()))
            .with_tooltip("Hash Service")
            .with_icon(icon)
            .with_title("Hash Service")
            .build()
            .unwrap()
    }

    pub fn new_tray_menu() -> Menu {
        let menu = Menu::new();
        let load = MenuItem::new("Load Hashes", true, None);
        if let Err(err) = menu.append(&load) {
            println!("{err:?}");
        }
        let unload = MenuItem::new("Unload Hashes", true, None);
        if let Err(err) = menu.append(&unload) {
            println!("{err:?}");
        }
        let quit = MenuItem::new("Quit", true, None);
        if let Err(err) = menu.append(&quit) {
            println!("{err:?}");
        }
        menu
    }

    fn spawn_grpc_call<F>(call_fn: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(call_fn);
        } else {
            eprintln!("No tokio runtime available");
        }
    }

    fn call_load_hashes() {
        Self::spawn_grpc_call(async {
            match HashLoaderClient::connect("http://[::1]:50051").await {
                Ok(mut client) => {
                    let request = tonic::Request::new(LoadHashesRequest {});
                    match client.load_hashes(request).await {
                        Ok(response) => {
                            let inner = response.into_inner();
                            if inner.success {
                                println!("Loaded {} hashes successfully", inner.count);
                            } else {
                                eprintln!("Failed to load hashes: {}", inner.message);
                            }
                        }
                        Err(e) => eprintln!("gRPC error calling load_hashes: {}", e),
                    }
                }
                Err(e) => eprintln!("Failed to connect to gRPC server: {}", e),
            }
        });
    }

    fn call_unload_hashes() {
        Self::spawn_grpc_call(async {
            match HashLoaderClient::connect("http://[::1]:50051").await {
                Ok(mut client) => {
                    let request = tonic::Request::new(UnloadHashesRequest {});
                    match client.unload_hashes(request).await {
                        Ok(response) => {
                            let inner = response.into_inner();
                            if inner.success {
                                println!("Unloaded hashes successfully");
                            } else {
                                eprintln!("Failed to unload hashes: {}", inner.message);
                            }
                        }
                        Err(e) => eprintln!("gRPC error calling unload_hashes: {}", e),
                    }
                }
                Err(e) => eprintln!("Failed to connect to gRPC server: {}", e),
            }
        });
    }
}

impl ApplicationHandler<UserEvent> for Application {
    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn new_events(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        if winit::event::StartCause::Init == cause {
            #[cfg(not(target_os = "linux"))]
            {
                self.tray_icon = Some(Self::new_tray_icon());
            }

            #[cfg(target_os = "macos")]
            unsafe {
                use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

                let rl = CFRunLoopGetMain().unwrap();
                CFRunLoopWakeUp(&rl);
            }
        }
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::MenuEvent(event) => match &event.id {
                MenuId(id) if id == "1001" => {
                    Self::call_load_hashes();
                }
                MenuId(id) if id == "1002" => {
                    Self::call_unload_hashes();
                }
                _ => {
                    std::process::exit(0);
                }
            },
            UserEvent::TrayIconEvent(_event) => {}
        }
    }
}

pub fn load_icon(path: &std::path::Path) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
