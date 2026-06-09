use freedesktop_desktop_entry::{DesktopEntry, Iter};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

thread_local! {
    /// Caches desktop-entry lookups so we don't rescan and reparse every
    /// `.desktop` file on each dock refresh. Negative results are cached too.
    static ID_CACHE:    RefCell<HashMap<String, Option<AppInfo>>> = RefCell::new(HashMap::new());
    static CLASS_CACHE: RefCell<HashMap<String, Option<AppInfo>>> = RefCell::new(HashMap::new());
}

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
        if let Some(hit) = ID_CACHE.with(|c| c.borrow().get(id).cloned()) {
            return hit;
        }
        let result = Self::find_by_id_uncached(id);
        ID_CACHE.with(|c| c.borrow_mut().insert(id.to_string(), result.clone()));
        result
    }

    fn find_by_id_uncached(id: &str) -> Option<Self> {
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
        let key = class.to_lowercase();
        if let Some(hit) = CLASS_CACHE.with(|c| c.borrow().get(&key).cloned()) {
            return hit;
        }
        let result = Self::find_by_class_uncached(class);
        CLASS_CACHE.with(|c| c.borrow_mut().insert(key, result.clone()));
        result
    }

    fn find_by_class_uncached(class: &str) -> Option<Self> {
        let class_lower = class.to_lowercase();
        let class_desktop = format!("{}.desktop", class_lower);

        // First pass: try exact filename match
        for path in Iter::new(Self::get_search_paths().into_iter()) {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
            if filename == class_desktop {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    return Some(AppInfo::from_entry(class, &entry));
                }
            }
        }

        // Second pass: try reading StartupWMClass from each desktop entry
        // (some apps have class different from their desktop filename)
        for path in Iter::new(Self::get_search_paths().into_iter()) {
            if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                if let Some(wm_class) = entry.startup_wm_class() {
                    if wm_class.to_lowercase() == class_lower {
                        return Some(AppInfo::from_entry(class, &entry));
                    }
                }
            }
        }

        None
    }

    fn from_entry(class: &str, entry: &DesktopEntry) -> Self {
        let class_lower = class.to_lowercase();
        AppInfo {
            id: class_lower,
            name: entry.name::<&str>(&[]).map(|s| s.to_string()).unwrap_or_else(|| class.to_string()),
            icon: entry.icon().map(|s| s.to_string()),
            exec: entry.exec().map(|s| s.to_string()).unwrap_or_default(),
        }
    }

    pub fn launch(&self) {
        let exec = clean_exec(&self.exec);

        if let Err(e) = Command::new("sh").arg("-c").arg(&exec).spawn() {
            log::error!("Failed to launch {}: {}", self.name, e);
        }
    }
}

/// Strip desktop-entry field codes (`%u`, `%f`, …) from an Exec line.
fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|s| !s.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_exec_removes_field_codes() {
        assert_eq!(clean_exec("firefox %u"), "firefox");
        assert_eq!(clean_exec("code --new-window %F"), "code --new-window");
        assert_eq!(clean_exec("kitty"), "kitty");
        assert_eq!(clean_exec(""), "");
    }
}
