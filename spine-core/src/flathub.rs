//! Spine Emulators on Linux (0.36): emulators come from Flathub as Flatpaks, installed for
//! the current user (no password needed). The `flatpak` program does the downloading,
//! checking (signed by Flathub) and updating; this module only asks it.
//!
//! The Flathub app ids are written into the program (checked against flathub.org on
//! Nov 6, 2026) and each is one Spine's launcher already knows (`launch::FLATPAKS`).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::Stdio;

/// Flathub's own repository file (adds the "flathub" remote for this user).
pub const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// Spine Emulators id -> Flathub app id. Mesen, Vita3K and Xenia have no Flathub app.
pub const APPS: &[(&str, &str)] = &[
    ("retroarch", "org.libretro.RetroArch"),
    ("mgba", "io.mgba.mGBA"),
    ("flycast", "org.flycast.Flycast"),
    ("duckstation", "org.duckstation.DuckStation"),
    ("pcsx2", "net.pcsx2.PCSX2"),
    ("ppsspp", "org.ppsspp.PPSSPP"),
    ("rpcs3", "net.rpcs3.RPCS3"),
    ("melonds", "net.kuribo64.melonDS"),
    ("azahar", "org.azahar_emu.Azahar"),
    ("dolphin", "org.DolphinEmu.dolphin-emu"),
    ("cemu", "info.cemu.Cemu"),
    ("eden", "dev.eden_emu.eden"),
    ("xemu", "app.xemu.xemu"),
];

pub fn app_id(emu: &str) -> Option<&'static str> {
    APPS.iter().find(|(e, _)| *e == emu).map(|(_, a)| *a)
}

pub fn emu_of(app: &str) -> Option<&'static str> {
    APPS.iter().find(|(_, a)| *a == app).map(|(e, _)| *e)
}

fn flatpak() -> std::process::Command {
    let mut c = crate::hostenv::command("flatpak");
    // English messages, so the few lines we read look the same everywhere.
    c.env("LC_ALL", "C");
    c
}

/// Is the `flatpak` program there? (Linux Mint, Fedora, SteamOS, Bazzite... have it.)
pub fn available() -> bool {
    flatpak().arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |s| s.success())
}

/// An installed Flatpak: version and where ("user" or "system").
#[derive(Debug, Clone, PartialEq)]
pub struct Have {
    pub version: String,
    pub installation: String,
}

/// `flatpak list --app --columns=application,version,installation` (tab separated).
pub fn parse_list(text: &str) -> BTreeMap<String, Have> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            let app = it.next()?.trim();
            if app.is_empty() || !app.contains('.') {
                return None;
            }
            let version = it.next().unwrap_or("").trim().to_string();
            let installation = it.next().unwrap_or("").trim().to_string();
            Some((app.to_string(), Have { version, installation }))
        })
        .collect()
}

/// The Flatpak apps installed on this computer (user and system).
pub fn list() -> BTreeMap<String, Have> {
    flatpak()
        .args(["list", "--app", "--columns=application,version,installation"])
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_list(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// A line of flatpak's output in plain words ("Installing runtime/org.kde.Platform/..." ->
/// "Getting shared parts (org.kde.Platform)…"). None for lines not worth showing.
pub fn friendly(line: &str, name: &str) -> Option<String> {
    let line = line.trim();
    for (verb, word) in [("Installing ", "Installing"), ("Updating ", "Updating")] {
        if let Some(r) = line.strip_prefix(verb) {
            let r = r.trim_end_matches('…').trim_end_matches("...").trim();
            let mut parts = r.split('/');
            let kind = parts.next().unwrap_or("");
            let id = parts.next().unwrap_or("");
            if kind == "app" {
                return Some(format!("{word} {name}…"));
            }
            if kind == "runtime" && !id.is_empty() {
                return Some(format!("Getting shared parts ({id})…"));
            }
        }
    }
    if line.starts_with("Looking for matches") || line.starts_with("Looking for updates") {
        return Some("Asking Flathub…".into());
    }
    None
}

/// Run flatpak with the output turned into progress lines; on failure, flatpak's own last
/// error line.
fn run(args: &[&str], name: &str, step: &dyn Fn(String)) -> Result<(), String> {
    let mut child = flatpak().args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Couldn't start flatpak ({e})."))?;
    let err = child.stderr.take().map(|e| std::thread::spawn(move || BufReader::new(e).lines().map_while(Result::ok).collect::<Vec<_>>()));
    if let Some(out) = child.stdout.take() {
        // flatpak redraws progress with "\r": split on both.
        let mut buf = String::new();
        let mut r = BufReader::new(out);
        let mut bytes = Vec::new();
        while r.read_until(b'\n', &mut bytes).map_or(false, |n| n > 0) {
            buf.push_str(&String::from_utf8_lossy(&bytes));
            bytes.clear();
            for piece in buf.split(['\r', '\n']) {
                if let Some(t) = friendly(piece, name) {
                    step(t);
                }
            }
            buf.clear();
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errs = err.and_then(|t| t.join().ok()).unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let last = errs.iter().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().trim_start_matches("error:").trim().to_string()).unwrap_or_else(|| "flatpak stopped".into());
    Err(last)
}

/// Install (or, when it's already installed anywhere, update) one emulator from Flathub.
/// New installs go to the user's own Flatpak folder. Returns the installed version.
pub fn install(emu: &str, name: &str, step: &dyn Fn(String)) -> Result<String, String> {
    let app = app_id(emu).ok_or_else(|| format!("{name} isn't on Flathub."))?;
    if !available() {
        return Err("Flatpak isn't installed. Install it from your distribution first (on Ubuntu: sudo apt install flatpak).".into());
    }
    let have = list();
    let r = if have.contains_key(app) {
        step(format!("Updating {name}…"));
        run(&["update", "--noninteractive", "-y", app], name, step)
    } else {
        step("Asking Flathub…".into());
        run(&["remote-add", "--user", "--if-not-exists", "flathub", FLATHUB_REPO], name, step).map_err(|e| format!("Couldn't add Flathub ({e})."))?;
        run(&["install", "--user", "--noninteractive", "-y", "flathub", app], name, step)
    };
    r.map_err(|e| format!("Flathub didn't install {name}: {e}"))?;
    Ok(list().get(app).map(|h| h.version.clone()).filter(|v| !v.is_empty()).unwrap_or_else(|| "installed".into()))
}

/// `flatpak remote-ls --updates --app --columns=application,version`: app id -> new version
/// (may be empty: not every app lists one).
pub fn parse_updates(text: &str) -> BTreeMap<String, String> {
    parse_list(text).into_iter().map(|(k, h)| (k, h.version)).collect()
}

/// Our emulators with an update waiting on Flathub: emulator id -> new version ("" when
/// Flathub doesn't say).
pub fn updates() -> Result<BTreeMap<String, String>, String> {
    let o = flatpak().args(["remote-ls", "--updates", "--app", "--columns=application,version"]).output().map_err(|e| format!("Couldn't start flatpak ({e})."))?;
    if !o.status.success() {
        let e = String::from_utf8_lossy(&o.stderr);
        return Err(format!("Couldn't ask Flathub ({}).", e.lines().last().unwrap_or("no answer").trim()));
    }
    Ok(parse_updates(&String::from_utf8_lossy(&o.stdout)).into_iter().filter_map(|(app, v)| emu_of(&app).map(|e| (e.to_string(), v))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_is_one_spine_launches() {
        for (emu, app) in APPS {
            assert!(crate::emus::emu(emu).is_some(), "{emu} not in Spine Emulators' list");
            assert_eq!(crate::launch::flatpak_id(emu), Some(*app), "Spine wouldn't find {app}");
        }
    }

    #[test]
    fn reads_flatpak_list() {
        let l = parse_list("io.mgba.mGBA\t0.10.5\tuser\nnet.pcsx2.PCSX2\t\tsystem\n\n");
        assert_eq!(l["io.mgba.mGBA"], Have { version: "0.10.5".into(), installation: "user".into() });
        assert_eq!(l["net.pcsx2.PCSX2"].installation, "system");
        assert_eq!(l.len(), 2);
        let u = parse_updates("org.libretro.RetroArch\t1.22.2\n");
        assert_eq!(u["org.libretro.RetroArch"], "1.22.2");
    }

    #[test]
    fn plain_words_for_progress() {
        assert_eq!(friendly("Installing runtime/org.kde.Platform/x86_64/5.15-25.08", "mGBA").as_deref(), Some("Getting shared parts (org.kde.Platform)…"));
        assert_eq!(friendly("Installing app/io.mgba.mGBA/x86_64/stable", "mGBA").as_deref(), Some("Installing mGBA…"));
        assert_eq!(friendly("Updating app/io.mgba.mGBA/x86_64/stable…", "mGBA").as_deref(), Some("Updating mGBA…"));
        assert_eq!(friendly("Note that the directories", "mGBA"), None);
    }

    /// Real install through flatpak (needs flatpak and the network; downloads Flycast and
    /// its runtime): cargo test --lib -- --ignored live_flathub
    #[test]
    #[ignore]
    #[cfg(unix)]
    fn live_flathub() {
        assert!(available());
        let lines = std::sync::Mutex::new(Vec::new());
        let v = install("flycast", "Flycast", &|t| lines.lock().unwrap().push(t)).unwrap();
        assert!(!v.is_empty() && v != "installed", "{v}");
        let lines = lines.into_inner().unwrap();
        assert!(lines.iter().any(|l| l == "Installing Flycast…" || l == "Updating Flycast…"), "{lines:?}");
        assert!(list().contains_key("org.flycast.Flycast"));
        // Spine finds it and starts it through flatpak.
        let inst = crate::launch::Installed::detect(&[]);
        assert_eq!(inst.flatpak.get("flycast").map(String::as_str), Some("org.flycast.Flycast"));
        // A second run updates (nothing to do) instead of failing.
        install("flycast", "Flycast", &|_| {}).unwrap();
        updates().unwrap();
    }
}
