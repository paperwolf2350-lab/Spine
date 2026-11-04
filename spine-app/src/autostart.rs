//! "Start Spine when I log in" (Settings → Updates & window).
//!
//! The computer's own setting is the truth, so turning it off in Task Manager or
//! Startup Applications shows up in Spine too.
//!   Windows: a "Spine" value in HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
//!   Linux:   ~/.config/autostart/spine.desktop (works in Cinnamon, GNOME, KDE, XFCE...).

use std::path::PathBuf;

/// The program to start: the AppImage file itself on Linux (the running program
/// lives inside a temporary mount), otherwise Spine's own exe.
fn program() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(a) = crate::linux::appimage() {
        return Some(a);
    }
    std::env::current_exe().ok()
}

#[cfg(windows)]
mod imp {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Spine";

    pub fn current() -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_READ).ok()?.get_value::<String, _>(NAME).ok()
    }

    pub fn want(exe: &std::path::Path) -> String {
        format!("\"{}\"", exe.display())
    }

    pub fn write(value: &str) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN).map_err(|e| e.to_string())?;
        key.set_value(NAME, &value.to_string()).map_err(|e| e.to_string())
    }

    pub fn remove() -> Result<(), String> {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_SET_VALUE) else { return Ok(()) };
        match key.delete_value(NAME) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::fs;
    use std::path::PathBuf;

    fn file() -> PathBuf {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join(".config"));
        config.join("autostart").join("spine.desktop")
    }

    /// Present and not switched off (desktops turn an entry off with Hidden=true
    /// or X-GNOME-Autostart-enabled=false instead of deleting it).
    pub fn current() -> Option<String> {
        let t = fs::read_to_string(file()).ok()?;
        let off = t.lines().any(|l| {
            let l = l.trim().to_ascii_lowercase();
            l == "hidden=true" || l == "x-gnome-autostart-enabled=false"
        });
        (!off).then_some(t)
    }

    pub fn want(exe: &std::path::Path) -> String {
        crate::linux::desktop_entry(exe) + "X-GNOME-Autostart-enabled=true\n"
    }

    pub fn write(value: &str) -> Result<(), String> {
        let f = file();
        if let Some(d) = f.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        fs::write(&f, value).map_err(|e| e.to_string())
    }

    pub fn remove() -> Result<(), String> {
        match fs::remove_file(file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    pub fn current() -> Option<String> {
        None
    }
    pub fn want(_: &std::path::Path) -> String {
        String::new()
    }
    pub fn write(_: &str) -> Result<(), String> {
        Err("Not available on this system.".into())
    }
    pub fn remove() -> Result<(), String> {
        Ok(())
    }
}

/// Is Spine set to start at log-in?
pub fn is_on() -> bool {
    imp::current().is_some()
}

/// Turn it on or off; returns the new state as the computer now has it.
pub fn set(on: bool) -> Result<bool, String> {
    if on {
        let exe = program().ok_or("Couldn't find Spine's own program file.")?;
        imp::write(&imp::want(&exe))?;
    } else {
        imp::remove()?;
    }
    Ok(is_on())
}

/// At start-up: if it's on, keep it pointing at wherever Spine now lives
/// (a moved Spine.exe or AppImage would otherwise start nothing).
pub fn refresh() {
    let Some(cur) = imp::current() else { return };
    if let Some(exe) = program() {
        let want = imp::want(&exe);
        if cur != want {
            let _ = imp::write(&want);
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn linux_entry_on_off() {
        let tmp = std::env::temp_dir().join(format!("spine-autostart-{}", std::process::id()));
        // SAFETY: single test touching XDG_CONFIG_HOME.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };
        assert!(!is_on());
        assert_eq!(set(true), Ok(true));
        let t = std::fs::read_to_string(tmp.join("autostart/spine.desktop")).unwrap();
        assert!(t.contains("Exec=\"") && t.contains("X-GNOME-Autostart-enabled=true"), "{t}");
        // Switched off by the desktop's own Startup Applications.
        std::fs::write(tmp.join("autostart/spine.desktop"), t.replace("enabled=true", "enabled=false")).unwrap();
        assert!(!is_on());
        assert_eq!(set(true), Ok(true));
        assert_eq!(set(false), Ok(false));
        assert_eq!(set(false), Ok(false));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
