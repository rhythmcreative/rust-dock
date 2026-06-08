use freedesktop_desktop_entry::{DesktopEntry, Iter};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: String,
}

impl AppInfo {
    fn get_search_paths() -> Vec<PathBuf> {
        let mut paths = vec![PathBuf::from("/usr/share/applications")];
        if let Some(mut home_apps) = dirs::data_dir() {
            home_apps.push("applications");
            paths.push(home_apps);
        }
        paths
    }

    pub fn find_by_id(id: &str) -> Option<Self> {
        let id_with_extension = if id.ends_with(".desktop") {
            id.to_string()
        } else {
            format!("{}.desktop", id)
        };

        for path in Iter::new(Self::get_search_paths().into_iter()) {
            if path.file_name().and_then(|s| s.to_str()) == Some(&id_with_extension) {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    return Some(AppInfo {
                        id: id.to_string(),
                        name: entry.name::<&str>(&[]).map(|s| s.to_string()).unwrap_or_else(|| id.to_string()),
                        icon: entry.icon().map(|s| s.to_string()),
                        exec: entry.exec().map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
        }
        None
    }

    pub fn find_by_class(class: &str) -> Option<Self> {
        let class_lower = class.to_lowercase();
        
        for path in Iter::new(Self::get_search_paths().into_iter()) {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
            if filename.starts_with(&class_lower) {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    return Some(AppInfo {
                        id: filename.replace(".desktop", ""),
                        name: entry.name::<&str>(&[]).map(|s| s.to_string()).unwrap_or_else(|| class.to_string()),
                        icon: entry.icon().map(|s| s.to_string()),
                        exec: entry.exec().map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
        }
        None
    }

    pub fn launch(&self) {
        let exec = self.exec.split_whitespace()
            .filter(|s| !s.starts_with('%'))
            .collect::<Vec<_>>()
            .join(" ");
            
        if let Err(e) = Command::new("sh").arg("-c").arg(&exec).spawn() {
            eprintln!("Failed to launch {}: {}", self.name, e);
        }
    }
}
