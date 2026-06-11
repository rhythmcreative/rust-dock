use clap::Parser;
use std::path::PathBuf;
use crate::config::Config;

/// A modern, high-performance dock for Hyprland.
///
/// Command-line options override values read from the config file
/// (`~/.config/rust-dock/hypr-dock.conf`).
#[derive(Parser, Debug, Clone)]
#[command(name = "rust-dock", version, about)]
pub struct Cli {
    /// Dock position: top | bottom | left | right.
    #[arg(short = 'p', long)]
    pub position: Option<String>,

    /// Icon size in pixels.
    #[arg(short = 'i', long = "icon-size")]
    pub icon_size: Option<i32>,

    /// Internal padding of the dock.
    #[arg(long)]
    pub padding: Option<i32>,

    /// Space between application icons.
    #[arg(long)]
    pub spacing: Option<i32>,

    /// Corner radius of the dock.
    #[arg(long)]
    pub radius: Option<i32>,

    /// Background opacity (0.0 - 1.0).
    #[arg(long)]
    pub opacity: Option<f32>,

    /// Target a specific monitor by connector name (e.g. DP-6).
    #[arg(short = 'o', long)]
    pub output: Option<String>,

    /// Command run by the launcher button.
    #[arg(short = 'l', long = "launcher-command")]
    pub launcher_command: Option<String>,

    /// Show the launcher button.
    #[arg(long)]
    pub launcher: bool,

    /// Hide the launcher button.
    #[arg(long = "no-launcher")]
    pub no_launcher: bool,

    /// Reserve screen space so other windows don't overlap the dock (moves other windows aside).
    #[arg(short = 'x', long = "exclusive-zone", short_alias = 'e')]
    pub exclusive_zone: bool,

    /// Don't reserve screen space.
    #[arg(long = "no-exclusive-zone")]
    pub no_exclusive_zone: bool,

    /// Enable smart view (auto-hide until the cursor reaches the edge).
    #[arg(long = "smart-view")]
    pub smart_view: bool,

    /// Path to an extra CSS file appended to the stylesheet.
    #[arg(long)]
    pub style: Option<PathBuf>,

    /// Layer shell layer: overlay | top | bottom | background.
    #[arg(short = 'y', long = "layer")]
    pub layer: Option<String>,

    /// Margin between the dock and the screen edge.
    #[arg(short = 'm', long)]
    pub margin: Option<i32>,

    /// Align the margin with Hyprland's gaps_out.
    #[arg(long = "system-gap")]
    pub system_gap: bool,

    /// Do not align the margin with Hyprland's gaps_out.
    #[arg(long = "no-system-gap")]
    pub no_system_gap: bool,

    /// Toggle the visibility of a running rust-dock instance and exit.
    #[arg(long)]
    pub toggle: bool,
}

impl Cli {
    /// Apply any provided options on top of an existing config.
    pub fn apply_to(&self, cfg: &mut Config) {
        if let Some(v) = &self.position { cfg.position = v.clone(); }
        if let Some(v) = self.icon_size { cfg.icon_size = v; }
        if let Some(v) = self.padding { cfg.padding = v; }
        if let Some(v) = self.spacing { cfg.spacing = v; }
        if let Some(v) = self.radius { cfg.radius = v; }
        if let Some(v) = self.opacity { cfg.opacity = v; }
        if self.output.is_some() { cfg.output = self.output.clone(); }
        if let Some(v) = &self.launcher_command {
            cfg.launcher_command = v.clone();
            cfg.no_launcher = false;
        }
        if self.launcher { cfg.no_launcher = false; }
        if self.no_launcher { cfg.no_launcher = true; }
        if self.exclusive_zone { cfg.exclusive_zone = true; }
        if self.no_exclusive_zone { cfg.exclusive_zone = false; }
        if self.smart_view { cfg.smart_view = true; }
        if self.style.is_some() { cfg.style = self.style.clone(); }
        if let Some(v) = &self.layer { cfg.layer = v.clone(); }
        if let Some(v) = self.margin { cfg.margin = v; }
        if self.system_gap { cfg.system_gap_used = true; }
        if self.no_system_gap { cfg.system_gap_used = false; }
    }
}
