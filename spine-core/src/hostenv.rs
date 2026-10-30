//! Starting other programs with the computer's own settings.
//!
//! When Spine runs as a Linux AppImage, the AppImage points Spine at the libraries
//! bundled inside it (LD_LIBRARY_PATH, GIO_MODULE_DIR, GST_PLUGIN_PATH, ...). Those
//! settings must not reach curl, RetroArch, flatpak or any other program Spine
//! starts: they'd load Spine's libraries instead of their own and crash or fail.

use std::ffi::OsString;
use std::process::Command;

/// Settings an AppImage adds that only make sense inside it.
const APPIMAGE_ONLY: &[&str] = &["APPDIR", "APPIMAGE_EXTRACT_AND_RUN", "PYTHONHOME", "PYTHONPATH", "PERLLIB", "GTK_THEME", "GTK_EXE_PREFIX", "GTK_DATA_PREFIX", "GTK_IM_MODULE_FILE", "GDK_PIXBUF_MODULE_FILE", "GST_REGISTRY_REUSE_PLUGIN_SCANNER", "PYTHONDONTWRITEBYTECODE", "PYTHONUNBUFFERED"];

/// The environment a program should get: every path inside the AppImage removed
/// (variables left empty are unset). Returns (name, value or None to unset).
pub fn host_env_changes(vars: impl Iterator<Item = (OsString, OsString)>, appdir: &str) -> Vec<(OsString, Option<OsString>)> {
    let mut out = Vec::new();
    for (k, v) in vars {
        let name = k.to_string_lossy().to_string();
        if APPIMAGE_ONLY.contains(&name.as_str()) {
            out.push((k, None));
            continue;
        }
        let val = v.to_string_lossy();
        if !val.contains(appdir) {
            continue;
        }
        let kept: Vec<&str> = val.split(':').filter(|p| !p.is_empty() && !p.starts_with(appdir)).collect();
        if kept.is_empty() {
            out.push((k, None));
        } else {
            out.push((k, Some(OsString::from(kept.join(":")))));
        }
    }
    out
}

/// A Command for an outside program, with the computer's own settings.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    sanitize(&mut cmd);
    cmd
}

/// Apply the clean-up to an existing Command (no-op unless running as an AppImage).
pub fn sanitize(cmd: &mut Command) {
    let Some(appdir) = std::env::var_os("APPDIR").map(|a| a.to_string_lossy().trim_end_matches('/').to_string()).filter(|a| a.len() > 1) else { return };
    for (k, v) in host_env_changes(std::env::vars_os(), &appdir) {
        match v {
            Some(v) => cmd.env(k, v),
            None => cmd.env_remove(k),
        };
    }
    // The AppImage starts Spine inside itself; programs should start from home instead
    // (an emulator's own folder, set later by the launcher, still takes priority).
    if std::env::current_dir().map_or(false, |d| d.starts_with(&appdir)) {
        if let Some(home) = std::env::var_os("HOME") {
            cmd.current_dir(home);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_appimage_paths() {
        let a = "/tmp/.mount_SpineXYZ";
        let vars = vec![
            ("LD_LIBRARY_PATH", format!("{a}/usr/lib/:{a}/usr/lib/x86_64-linux-gnu")),
            ("PATH", format!("{a}/usr/bin/:/usr/local/bin:/usr/bin")),
            ("XDG_DATA_DIRS", format!("{a}/usr/share/:/usr/share:/usr/local/share")),
            ("GTK_THEME", "Adwaita:light".to_string()),
            ("HOME", "/home/b".to_string()),
        ];
        let ch = host_env_changes(vars.into_iter().map(|(k, v)| (OsString::from(k), OsString::from(v))), a);
        let get = |k: &str| ch.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert_eq!(get("LD_LIBRARY_PATH"), Some(None));
        assert_eq!(get("PATH"), Some(Some(OsString::from("/usr/local/bin:/usr/bin"))));
        assert_eq!(get("XDG_DATA_DIRS"), Some(Some(OsString::from("/usr/share:/usr/local/share"))));
        assert_eq!(get("GTK_THEME"), Some(None));
        assert_eq!(get("HOME"), None, "untouched");
    }
}
