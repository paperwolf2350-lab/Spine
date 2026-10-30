//! Linux: where Spine keeps its things, and putting Spine in the app menu.
//!
//! The AppImage is a single read-only file, so Spine keeps settings, covers and
//! favorites in ~/.local/share/spine instead of next to the program. (Put an empty
//! portable.txt next to Spine to keep everything beside it, like on Windows.)

use std::fs;
use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// ~/.local/share (or $XDG_DATA_HOME).
pub fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(".local").join("share"))
}

pub fn data_dir(exe_dir: &Path) -> PathBuf {
    if exe_dir.join("portable.txt").exists() {
        return exe_dir.to_path_buf();
    }
    let d = data_home().join("spine");
    let _ = fs::create_dir_all(&d);
    d
}

/// The AppImage file itself, when Spine runs as one.
pub fn appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file())
}

const ICON: &[u8] = include_bytes!("../icons/icon.png");

pub fn desktop_entry(exec: &Path) -> String {
    // Desktop-entry Exec quoting: wrap in quotes, escape backslash, quote, backtick and dollar.
    let mut q = String::from('"');
    for c in exec.to_string_lossy().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    format!(
        "[Desktop Entry]\nType=Application\nName=Spine\nGenericName=Game Library\nComment=Your game library, like a console\nExec={q}\nIcon=spine\nTerminal=false\nCategories=Game;Emulator;\nKeywords=games;emulator;roms;retro;\nStartupWMClass=Spine\n"
    )
}

/// Add Spine to the app menu (and keep the entry pointing at wherever the AppImage
/// now lives). Nothing to do for a .deb install, which brings its own entry.
pub fn ensure_menu_entry() {
    let Some(app) = appimage() else { return };
    let apps = data_home().join("applications");
    let icons = data_home().join("icons").join("hicolor").join("512x512").join("apps");
    let entry = apps.join("spine.desktop");
    let want = desktop_entry(&app);
    if fs::read_to_string(&entry).ok().as_deref() != Some(want.as_str()) {
        let _ = fs::create_dir_all(&apps);
        let _ = fs::write(&entry, want);
    }
    let icon = icons.join("spine.png");
    if !icon.exists() {
        let _ = fs::create_dir_all(&icons);
        let _ = fs::write(&icon, ICON);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_quotes_odd_paths() {
        let e = desktop_entry(Path::new("/home/me/My Apps/Spine$1.AppImage"));
        assert!(e.contains("Exec=\"/home/me/My Apps/Spine\\$1.AppImage\"\n"), "{e}");
        assert!(e.contains("Icon=spine"));
    }
}
