use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Auto-hide: Shows dock on hotspot hover, hides when mouse leaves
    #[arg(short, long)]
    pub auto_hide: bool,

    /// Resident: Keeps the dock running in the background without a hotspot
    #[arg(short, long)]
    pub resident: bool,

    /// Position: bottom, top, left, or right
    #[arg(short, long, default_value = "bottom")]
    pub position: String,

    /// Icon size in pixels
    #[arg(short, long, default_value_t = 32)]
    pub icon_size: i32,

    /// Padding inside the dock
    #[arg(long, default_value_t = 4)]
    pub padding: i32,

    /// Spacing between icons
    #[arg(long, default_value_t = 6)]
    pub spacing: i32,

    /// Corner radius
    #[arg(long, default_value_t = 10)]
    pub radius: i32,

    /// Background opacity (0.0 to 1.0)
    #[arg(long, default_value_t = 0.8)]
    pub opacity: f32,

    /// Full screen: Makes the dock take the full width/height of the monitor
    #[arg(short, long)]
    pub full_screen: bool,

    /// Exclusive Zone: Moves other windows to prevent overlap
    #[arg(short, long)]
    pub exclusive_zone: bool,

    /// Output: Specify a specific monitor name (e.g., DP-1)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Launcher Command: Command to run when the launcher button is clicked
    #[arg(short, long, default_value = "nwg-drawer")]
    pub launcher_command: String,

    /// Styling: Path to a custom CSS file
    #[arg(short, long)]
    pub style: Option<PathBuf>,

    /// Workspaces: Number of workspaces to account for
    #[arg(short, long, default_value_t = 10)]
    pub workspaces: u32,

    /// Bottom margin in pixels
    #[arg(long, default_value_t = 0)]
    pub margin_bottom: i32,

    /// Left margin in pixels
    #[arg(long, default_value_t = 0)]
    pub margin_left: i32,

    /// Right margin in pixels
    #[arg(long, default_value_t = 0)]
    pub margin_right: i32,

    /// Top margin in pixels
    #[arg(long, default_value_t = 0)]
    pub margin_top: i32,

    /// Disables the launcher button
    #[arg(long)]
    pub no_launcher: bool,

    /// Pinned applications (list of desktop IDs)
    #[arg(skip)]
    pub pinned_apps: Vec<String>,
}

impl Config {
    pub fn new() -> Self {
        let mut config = Config::parse();
        
        // Load pinned apps from a config file if it exists
        if let Some(mut path) = dirs::config_dir() {
            path.push("rust-dock/config.json");
            if path.exists() {
                if let Ok(file_config) = std::fs::read_to_string(path) {
                    if let Ok(json_config) = serde_json::from_str::<serde_json::Value>(&file_config) {
                        if let Some(pinned) = json_config.get("pinned_apps").and_then(|v| v.as_array()) {
                            config.pinned_apps = pinned.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect();
                        }
                    }
                }
            }
        }
        
        config
    }
}
