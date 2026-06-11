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
        let mut paths = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        ];
        if let Some(data_dir) = dirs::data_dir() {
            paths.push(data_dir.join("applications"));
            paths.push(data_dir.join("flatpak/exports/share/applications"));
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
        let paths = Self::get_search_paths();
        let id_lower = id.to_lowercase();
        let id_with_ext = format!("{}.desktop", id_lower);

        // Case-insensitive match against desktop filenames (with or without .desktop).
        // Many apps store their class in lowercase (e.g. "alacritty") but the desktop
        // file is capitalised ("Alacritty.desktop"), so a strict compare silently fails.
        for path in Iter::new(paths.into_iter()) {
            let fname = match path.file_name().and_then(|s| s.to_str()) {
                Some(f) => f.to_lowercase(),
                None => continue,
            };
            if fname == id_lower || fname == id_with_ext {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    return Some(AppInfo {
                        id: id.to_string(),
                        name: entry.name::<&str>(&[]).map(|s| s.to_string()).unwrap_or_else(|| id.to_string()),
                        icon: entry.icon().map(|s| resolve_icon_name(s)),
                        exec: entry.exec().map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
        }

        // Fallback: treat the id as a window class name and search by StartupWMClass.
        // Covers apps whose desktop filename differs from their WM class
        // (e.g. pinned as "code" but desktop file is "code - oss.desktop").
        Self::find_by_class_uncached(id)
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

        // Single pass: filename match is returned immediately (highest priority);
        // WMClass match is collected as a fallback for apps whose desktop filename
        // differs from their Wayland class (e.g. "code-oss.desktop" for class "code").
        // This avoids two full directory scans while preserving the same priority order.
        let mut wm_class_match: Option<Self> = None;

        for path in Iter::new(Self::get_search_paths().into_iter()) {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
            if filename == class_desktop {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    return Some(AppInfo::from_entry(class, &entry));
                }
            }
            if wm_class_match.is_none() {
                if let Ok(entry) = DesktopEntry::from_path(&path, None::<&[&str]>) {
                    if let Some(wm_class) = entry.startup_wm_class() {
                        if wm_class.to_lowercase() == class_lower {
                            wm_class_match = Some(AppInfo::from_entry(class, &entry));
                        }
                    }
                }
            }
        }

        wm_class_match
    }

    fn from_entry(class: &str, entry: &DesktopEntry) -> Self {
        let class_lower = class.to_lowercase();
        AppInfo {
            id: class_lower,
            name: entry.name::<&str>(&[]).map(|s| s.to_string()).unwrap_or_else(|| class.to_string()),
            icon: entry.icon().map(|s| resolve_icon_name(s)),
            exec: entry.exec().map(|s| s.to_string()).unwrap_or_default(),
        }
    }

    pub fn launch(&self) {
        let exec = clean_exec(&self.exec);

        // [float] makes the new window open as floating so it can be freely
        // dragged to any position. Without this, Hyprland tiles the window and
        // it can only be moved with Super+drag.
        if let Err(e) = Command::new("hyprctl")
            .args(["dispatch", "exec", &format!("[float;center 1] {}", exec)])
            .spawn()
        {
            log::error!("Failed to launch {}: {}", self.name, e);
        }
    }
}

/// Resolve an icon field value from a .desktop file.
/// If the value is already an absolute path or a plain name, return it as-is.
/// If it looks like a bare filename with an image extension (e.g. "myapp.png"),
/// try to find it in /usr/share/pixmaps before falling back to the bare name.
fn resolve_icon_name(icon: &str) -> String {
    // Already an absolute path — use as-is.
    if icon.starts_with('/') { return icon.to_string(); }

    // Check if it has an image extension but no directory component.
    let has_ext = icon.ends_with(".png") || icon.ends_with(".svg") || icon.ends_with(".xpm");
    if has_ext {
        let candidate = std::path::Path::new("/usr/share/pixmaps").join(icon);
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
        // Strip extension so the theme lookup works (e.g. "myapp.png" → "myapp").
        return icon
            .strip_suffix(".png")
            .or_else(|| icon.strip_suffix(".svg"))
            .or_else(|| icon.strip_suffix(".xpm"))
            .unwrap_or(icon)
            .to_string();
    }

    icon.to_string()
}

/// Strip desktop-entry field codes (`%u`, `%f`, …) from an Exec line.
pub fn clean_exec_pub(exec: &str) -> String { clean_exec(exec) }

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
