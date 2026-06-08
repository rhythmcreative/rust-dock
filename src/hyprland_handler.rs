use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprClient {
    pub class: String,
    pub title: String,
    pub pid: i32,
    pub workspace: HyprWorkspace,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HyprWorkspace {
    pub id: i32,
    pub name: String,
}

pub struct HyprlandHandler {}

impl HyprlandHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_clients(&self) -> Vec<HyprClient> {
        let output = Command::new("hyprctl")
            .arg("clients")
            .arg("-j")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(clients) = serde_json::from_slice::<Vec<HyprClient>>(&out.stdout) {
                    return clients;
                }
            }
        }
        Vec::new()
    }
}

pub fn start_listener<F>(on_event: F) 
where 
    F: Fn() + Send + Sync + 'static 
{
    use hyprland::event_listener::EventListener;
    let on_event = Arc::new(on_event);
    
    std::thread::spawn(move || {
        let mut event_listener = EventListener::new();
        
        let oe1 = Arc::clone(&on_event);
        event_listener.add_window_open_handler(move |_| oe1());
        
        let oe2 = Arc::clone(&on_event);
        event_listener.add_window_close_handler(move |_| oe2());
        
        let oe3 = Arc::clone(&on_event);
        event_listener.add_active_window_change_handler(move |_| oe3());
        
        let oe4 = Arc::clone(&on_event);
        event_listener.add_workspace_change_handler(move |_| oe4());
        
        // Use a safe catch for the listener too
        if let Err(e) = event_listener.start_listener() {
            eprintln!("Hyprland event listener error: {}", e);
        }
    });
}
