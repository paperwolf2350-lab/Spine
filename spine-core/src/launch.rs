//! Finding installed emulators and building the command that starts a game.

use crate::scan::Game;
use crate::systems::{Launcher, System};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct EmulatorDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Executable file names to look for (case-insensitive), Windows then Linux.
    pub exes: &'static [&'static str],
    /// Arguments; "{rom}" is the game file, "{title_id}" the console title id.
    pub args: &'static [&'static str],
}

pub static EMULATORS: &[EmulatorDef] = &[
    EmulatorDef { id: "retroarch", name: "RetroArch", exes: &["retroarch.exe", "retroarch"], args: &["-f", "-L", "{core}", "{rom}"] },
    EmulatorDef { id: "pcsx2", name: "PCSX2", exes: &["pcsx2-qt.exe", "pcsx2-qtx64-avx2.exe", "pcsx2-qtx64.exe", "pcsx2-qt", "pcsx2.appimage"], args: &["-batch", "-nogui", "-fullscreen", "--", "{rom}"] },
    EmulatorDef { id: "dolphin", name: "Dolphin", exes: &["dolphin.exe", "dolphin-emu", "dolphin-emu.appimage"], args: &["-b", "-e", "{rom}"] },
    EmulatorDef { id: "cemu", name: "Cemu", exes: &["cemu.exe", "cemu", "cemu.appimage"], args: &["-f", "-g", "{rom}"] },
    EmulatorDef { id: "ppsspp", name: "PPSSPP", exes: &["ppssppwindows64.exe", "ppssppwindows.exe", "ppssppsdl", "ppsspp.appimage"], args: &["--fullscreen", "{rom}"] },
    EmulatorDef { id: "azahar", name: "Azahar", exes: &["azahar.exe", "azahar-qt.exe", "lime3ds.exe", "lime3ds-gui.exe", "citra-qt.exe", "azahar", "azahar.appimage"], args: &["-f", "{rom}"] },
    EmulatorDef { id: "vita3k", name: "Vita3K", exes: &["vita3k.exe", "vita3k", "vita3k.appimage"], args: &["-F", "-r", "{title_id}"] },
    EmulatorDef { id: "eden", name: "Eden", exes: &["eden.exe", "citron.exe", "sudachi.exe", "eden", "eden.appimage", "citron.appimage"], args: &["-f", "-g", "{rom}"] },
    EmulatorDef { id: "ryujinx", name: "Ryujinx", exes: &["ryujinx.exe", "ryujinx", "ryujinx.appimage"], args: &["--fullscreen", "{rom}"] },
    EmulatorDef { id: "rpcs3", name: "RPCS3", exes: &["rpcs3.exe", "rpcs3", "rpcs3.appimage"], args: &["--no-gui", "{rom}"] },
    EmulatorDef { id: "melonds", name: "melonDS", exes: &["melonds.exe", "melonds"], args: &["-f", "{rom}"] },
    EmulatorDef { id: "xemu", name: "xemu", exes: &["xemu.exe", "xemu"], args: &["-full-screen", "-dvd_path", "{rom}"] },
    EmulatorDef { id: "duckstation", name: "DuckStation", exes: &["duckstation-qt-x64-releaseltcg.exe", "duckstation-qt.exe", "duckstation-qt", "duckstation.appimage"], args: &["-batch", "-fullscreen", "--", "{rom}"] },
    EmulatorDef { id: "rmg", name: "Rosalie's Mupen GUI", exes: &["rmg.exe", "rmg", "rmg.appimage"], args: &["--fullscreen", "{rom}"] },
    EmulatorDef { id: "simple64", name: "simple64", exes: &["simple64-gui.exe", "simple64-gui"], args: &["{rom}"] },
    EmulatorDef { id: "mgba", name: "mGBA", exes: &["mgba.exe", "mgba-qt", "mgba.appimage"], args: &["-f", "{rom}"] },
];

pub fn emulator(id: &str) -> Option<&'static EmulatorDef> {
    EMULATORS.iter().find(|e| e.id == id)
}

/// Emulators found on this computer: id -> executable path, plus (on Linux)
/// emulators installed as Flatpaks: id -> Flatpak app id.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Installed {
    pub found: BTreeMap<String, PathBuf>,
    pub flatpak: BTreeMap<String, String>,
}

/// Flathub app ids of the emulators Spine knows.
pub const FLATPAKS: &[(&str, &str)] = &[
    ("retroarch", "org.libretro.RetroArch"),
    ("pcsx2", "net.pcsx2.PCSX2"),
    ("dolphin", "org.DolphinEmu.dolphin-emu"),
    ("cemu", "info.cemu.Cemu"),
    ("ppsspp", "org.ppsspp.PPSSPP"),
    ("azahar", "org.azahar_emu.Azahar"),
    ("duckstation", "org.duckstation.DuckStation"),
    ("rpcs3", "net.rpcs3.RPCS3"),
    ("melonds", "net.kuribo64.melonDS"),
    ("xemu", "app.xemu.xemu"),
    ("mgba", "io.mgba.mGBA"),
    ("rmg", "com.github.Rosalie241.RMG"),
    ("simple64", "io.github.simple64.simple64"),
    ("ryujinx", "io.github.ryubing.Ryujinx"),
    ("vita3k", "org.vita3k.Vita3K"),
];

pub fn flatpak_id(emulator: &str) -> Option<&'static str> {
    FLATPAKS.iter().find(|(e, _)| *e == emulator).map(|(_, f)| *f)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Flatpaks installed system-wide or for this user (checked on disk, no commands run).
fn installed_flatpaks(roots: &[PathBuf]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (emu, id) in FLATPAKS {
        if roots.iter().any(|r| r.join(id).join("current").exists()) {
            out.insert(emu.to_string(), id.to_string());
        }
    }
    out
}

pub fn flatpak_roots() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/var/lib/flatpak/app")];
    if let Some(h) = home_dir() {
        v.push(h.join(".local/share/flatpak/app"));
    }
    v
}

impl Installed {
    /// Look for emulators under the given folders (two levels deep, which covers
    /// "Emulators/PCSX2/pcsx2-qt.exe" and "RetroArch-Win64/retroarch.exe").
    pub fn detect(search: &[PathBuf]) -> Self {
        let mut found = BTreeMap::new();
        for dir in search {
            visit(dir, 0, &mut found);
        }
        let flatpak = if cfg!(target_os = "linux") { installed_flatpaks(&flatpak_roots()) } else { BTreeMap::new() };
        Installed { found, flatpak }
    }

    /// For tests: Flatpaks under the given folders.
    pub fn detect_with_flatpaks(search: &[PathBuf], flatpak_dirs: &[PathBuf]) -> Self {
        let mut i = Self::detect(search);
        i.flatpak = installed_flatpaks(flatpak_dirs);
        i
    }

    pub fn has(&self, id: &str) -> bool {
        self.found.contains_key(id) || self.flatpak.contains_key(id)
    }

    pub fn path(&self, id: &str) -> Option<&PathBuf> {
        self.found.get(id)
    }
}

fn visit(dir: &Path, depth: usize, found: &mut BTreeMap<String, PathBuf>) {
    if depth > 2 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            visit(&p, depth + 1, found);
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        for emu in EMULATORS {
            // Exact program names, or any AppImage named after the emulator
            // ("pcsx2-Qt.AppImage", "Cemu-2.6-x86_64.AppImage").
            let appimage = name.ends_with(".appimage") && (name.starts_with(emu.id) || (emu.id == "eden" && (name.starts_with("citron") || name.starts_with("sudachi"))));
            if emu.exes.contains(&name.as_str()) || appimage {
                found.entry(emu.id.to_string()).or_insert(p.clone());
            }
        }
    }
}

/// Default places to look, given where the ROM library lives.
/// Covers ES-DE portable ("<drive>/ES-DE/Emulators"), EmuDeck, and normal installs.
pub fn default_search_paths(rom_root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(parent) = rom_root.parent() {
        v.push(parent.join("ES-DE").join("Emulators"));
        v.push(parent.join("Emulators"));
        v.push(parent.join("Emulation").join("tools"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA", "APPDATA"] {
        if let Ok(p) = std::env::var(var) {
            v.push(PathBuf::from(p));
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home = PathBuf::from(home);
        v.push(home.join("EmuDeck").join("Emulators"));
        v.push(home.join("Emulation").join("tools"));
        v.push(home.join("Applications"));
    }
    if cfg!(target_os = "linux") {
        if let Some(h) = home_dir() {
            v.push(h.join(".local/bin"));
            v.push(h.join("Applications")); // EmuDeck and most AppImage users
            v.push(h.join("AppImages"));
            v.push(h.join("Emulation").join("tools").join("launchers"));
        }
        for d in ["/usr/bin", "/usr/games", "/usr/local/bin", "/opt"] {
            v.push(PathBuf::from(d));
        }
    } else {
        v.push(PathBuf::from("/usr/bin"));
    }
    v
}

/// Everything needed to start a game, or why it can't start yet.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchPlan {
    pub emulator: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Empty when ready; otherwise what the user needs to do.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
}

/// A way to launch games of a console, for the "Change emulator" menu.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchOption {
    /// Stable key: "retroarch:mgba", "pcsx2", "system".
    pub key: String,
    pub label: String,
    /// Empty when ready; otherwise what the user needs to do.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
}

pub fn launcher_key(l: &Launcher) -> String {
    match l {
        Launcher::RetroArch(core) => format!("retroarch:{core}"),
        Launcher::Standalone(id) => id.to_string(),
        Launcher::ShellOpen => "system".into(),
    }
}

/// Every way this console's games can be launched, best first.
pub fn options(sys: &System, game: &Game, installed: &Installed) -> Vec<LaunchOption> {
    sys.launchers
        .iter()
        .map(|l| {
            let p = plan_with(l, game, installed);
            LaunchOption { key: launcher_key(l), label: p.emulator, blocked: p.blocked }
        })
        .collect()
}

/// Plan with a preferred launcher (a per-game choice), falling back to the console default
/// if that key is unknown.
pub fn plan_preferred(sys: &System, game: &Game, installed: &Installed, preferred: Option<&str>) -> LaunchPlan {
    if let Some(key) = preferred {
        if let Some(l) = sys.launchers.iter().find(|l| launcher_key(l) == key) {
            return plan_with(l, game, installed);
        }
    }
    plan(sys, game, installed)
}

pub fn plan(sys: &System, game: &Game, installed: &Installed) -> LaunchPlan {
    let mut first_block = None;
    for l in sys.launchers {
        let p = plan_with(l, game, installed);
        if p.blocked.is_none() {
            return p;
        }
        first_block.get_or_insert(p);
    }
    first_block.unwrap_or(LaunchPlan { emulator: "none".into(), program: PathBuf::new(), args: vec![], blocked: Some("No emulator configured for this console.".into()) })
}

fn plan_with(l: &Launcher, game: &Game, installed: &Installed) -> LaunchPlan {
    let rom = game.path.to_string_lossy().to_string();
    match l {
        Launcher::ShellOpen => LaunchPlan { emulator: "system".into(), program: game.path.clone(), args: vec![], blocked: None },
        Launcher::RetroArch(core) => {
            let def = emulator("retroarch").unwrap();
            let file = if cfg!(windows) { format!("{core}_libretro.dll") } else { format!("{core}_libretro.so") };
            let (program, prefix, dirs) = if let Some(exe) = installed.path("retroarch") {
                (exe.clone(), vec![], retroarch_core_dirs(Some(exe)))
            } else if let Some(fp) = installed.flatpak.get("retroarch") {
                (PathBuf::from("flatpak"), flatpak_run(fp, game), retroarch_core_dirs(None))
            } else {
                return LaunchPlan { emulator: format!("RetroArch · {core}"), program: PathBuf::from(def.exes[0]), args: vec![], blocked: Some("Install RetroArch.".into()) };
            };
            let core_file = dirs.iter().map(|d| d.join(&file)).find(|p| p.exists()).unwrap_or_else(|| dirs[0].join(&file));
            let blocked = (!core_file.exists()).then(|| format!("Download the \"{core}\" core in RetroArch (Main Menu → Online Updater → Core Downloader)."));
            // On Linux, ask RetroArch to explain itself (its messages go to Spine's log);
            // on Windows that option opens an extra console window, so not there.
            let verbose = cfg!(target_os = "linux").then(|| "--verbose".to_string());
            let args = prefix.into_iter().chain(verbose).chain(def.args.iter().map(|a| a.replace("{core}", &core_file.to_string_lossy()).replace("{rom}", &rom))).collect();
            LaunchPlan { emulator: format!("RetroArch · {core}"), program, args, blocked }
        }
        Launcher::Standalone(id) => {
            let def = emulator(id).expect("unknown emulator id");
            let (exe, prefix) = match (installed.path(id), installed.flatpak.get(*id)) {
                (Some(exe), _) => (exe.clone(), vec![]),
                (None, Some(fp)) => (PathBuf::from("flatpak"), flatpak_run(fp, game)),
                (None, None) => return LaunchPlan { emulator: def.name.into(), program: PathBuf::from(def.exes[0]), args: vec![], blocked: Some(format!("Install {}.", def.name)) },
            };
            let needs_id = def.args.iter().any(|a| a.contains("{title_id}"));
            let blocked = (needs_id && game.title_id.is_none()).then(|| format!("{} needs this game's title id first.", def.name));
            let tid = game.title_id.clone().unwrap_or_default();
            let args = prefix.into_iter().chain(def.args.iter().map(|a| a.replace("{rom}", &rom).replace("{title_id}", &tid))).collect();
            LaunchPlan { emulator: def.name.into(), program: exe, args, blocked }
        }
    }
}

/// Start the game. Returns the child process so the app can track playtime.
pub fn launch(plan: &LaunchPlan) -> std::io::Result<std::process::Child> {
    launch_logged(plan, None)
}

/// Like `launch`, writing the emulator's own messages to `log` so Spine can say
/// why a game closed right away.
pub fn launch_logged(plan: &LaunchPlan, log: Option<&Path>) -> std::io::Result<std::process::Child> {
    if let Some(why) = &plan.blocked {
        return Err(std::io::Error::other(why.clone()));
    }
    if plan.emulator == "system" {
        let target = plan.program.to_string_lossy().to_string();
        // A store game's own .exe (GOG): start it from its folder so it finds its files,
        // and Spine can time the session.
        if target.to_lowercase().ends_with(".exe") && !target.contains("://") {
            let mut cmd = crate::hostenv::command(&plan.program);
            if let Some(dir) = plan.program.parent() {
                cmd.current_dir(dir);
            }
            return cmd.spawn();
        }
        #[cfg(windows)]
        {
            if target.contains("://") {
                // Launcher links (steam://, com.epicgames.launcher://) contain '&', which cmd would
                // split on, so hand them to the URL handler directly.
                use std::os::windows::process::CommandExt;
                return std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &target]).creation_flags(0x0800_0000).spawn();
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            return std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&plan.program).creation_flags(0x0800_0000).spawn();
        }
        #[cfg(target_os = "macos")]
        return std::process::Command::new("open").arg(&plan.program).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        return crate::hostenv::command("xdg-open").arg(&plan.program).spawn();
    }
    let mut cmd = crate::hostenv::command(&plan.program);
    cmd.args(&plan.args);
    if let Some(f) = log.and_then(|p| fs::File::create(p).ok()) {
        if let Ok(f2) = f.try_clone() {
            cmd.stdout(f).stderr(f2);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no stray console window behind the emulator
    }
    if let Some(dir) = plan.program.parent().filter(|d| d.is_dir()) {
        cmd.current_dir(dir); // Several emulators expect to run from their own folder.
    }
    #[cfg(unix)]
    {
        // Its own process group, so "close the game" reaches everything it started.
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// "flatpak run --filesystem=<game folder> <app id>": a Flatpak only sees a few folders
/// on its own, so ROMs on another drive (/media/...) are shared with it for this run.
fn flatpak_run(app_id: &str, game: &Game) -> Vec<String> {
    let mut v = vec!["run".to_string()];
    let mut dirs: Vec<PathBuf> = game.versions.iter().map(|x| x.path.clone()).chain(std::iter::once(game.path.clone())).filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
    dirs.sort();
    dirs.dedup();
    for d in dirs {
        if d.as_os_str().is_empty() {
            continue;
        }
        v.push(format!("--filesystem={}", d.display()));
    }
    v.push(app_id.to_string());
    v
}

/// Where RetroArch keeps its cores: next to a portable retroarch(.exe), in the
/// user's config (cores from the Online Updater), the Flatpak's folder, or the
/// distribution's libretro folder.
pub fn retroarch_core_dirs(exe: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home = home_dir().unwrap_or_default();
    match exe {
        Some(exe) => {
            let dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
            v.push(dir.join("cores"));
            if cfg!(target_os = "linux") {
                v.push(home.join(".config/retroarch/cores"));
                for d in ["/usr/lib/x86_64-linux-gnu/libretro", "/usr/lib/libretro", "/usr/lib64/libretro", "/usr/local/lib/libretro"] {
                    v.push(PathBuf::from(d));
                }
            }
            // A cores folder set in retroarch.cfg wins.
            for cfg in [dir.join("retroarch.cfg"), home.join(".config/retroarch/retroarch.cfg")] {
                if let Some(d) = cfg_path(&cfg, "libretro_directory", &dir) {
                    v.insert(0, d);
                    break;
                }
            }
        }
        None => {
            let base = home.join(".var/app/org.libretro.RetroArch/config/retroarch");
            if let Some(d) = cfg_path(&base.join("retroarch.cfg"), "libretro_directory", &base) {
                v.push(d);
            }
            v.push(base.join("cores"));
        }
    }
    v
}

/// A folder setting from retroarch.cfg: "default" means unset, ":/x" or ":\\x" is
/// relative to RetroArch's folder, "~/" is the home folder.
pub fn cfg_path(cfg: &Path, key: &str, ra_dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(cfg).ok()?;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix(key) else { continue };
        let rest = rest.trim_start();
        let Some(v) = rest.strip_prefix('=') else { continue };
        let v = v.trim().trim_matches('"');
        if v.is_empty() || v == "default" {
            return None;
        }
        if let Some(rel) = v.strip_prefix(":\\").or_else(|| v.strip_prefix(":/")) {
            return Some(ra_dir.join(rel));
        }
        if let Some(rel) = v.strip_prefix("~/") {
            return home_dir().map(|h| h.join(rel));
        }
        return Some(PathBuf::from(v));
    }
    None
}

/// Official download pages for the emulators Spine can set up a console with.
pub fn download_pages() -> &'static [(&'static str, &'static str)] {
    &[
        ("retroarch", "https://www.retroarch.com/?page=platforms"),
        ("pcsx2", "https://pcsx2.net/downloads"),
        ("dolphin", "https://dolphin-emu.org/download/"),
        ("cemu", "https://cemu.info/"),
        ("ppsspp", "https://www.ppsspp.org/download/"),
        ("azahar", "https://azahar-emu.org/"),
        ("vita3k", "https://vita3k.org/"),
        ("rpcs3", "https://rpcs3.net/download"),
        ("melonds", "https://melonds.kuribo64.net/downloads.php"),
        ("xemu", "https://xemu.app/"),
        ("duckstation", "https://www.duckstation.org/"),
        ("rmg", "https://github.com/Rosalie241/RMG/releases"),
        ("simple64", "https://simple64.github.io/"),
        ("mgba", "https://mgba.io/downloads.html"),
    ]
}

/// The download page for whatever a blocked plan is missing ("Install PCSX2." -> PCSX2's page).
pub fn download_for(plan: &LaunchPlan) -> Option<&'static str> {
    let why = plan.blocked.as_deref()?;
    if why.starts_with("Install RetroArch") {
        if cfg!(target_os = "linux") {
            return flathub_pages().first().map(|(_, u)| *u);
        }
        return download_pages().iter().find(|(id, _)| *id == "retroarch").map(|(_, u)| *u);
    }
    let name = why.strip_prefix("Install ")?.trim_end_matches('.');
    let id = EMULATORS.iter().find(|e| e.name == name)?.id;
    if cfg!(target_os = "linux") {
        // On Linux the easy way is the app store: Flathub (Discover / GNOME Software).
        if let Some(url) = flathub_pages().iter().find(|(i, _)| *i == id).map(|(_, u)| *u) {
            return Some(url);
        }
    }
    download_pages().iter().find(|(i, _)| *i == id).map(|(_, u)| *u)
}

/// Flathub pages (one click to install from the desktop's software center).
pub fn flathub_pages() -> &'static [(&'static str, &'static str)] {
    &[
        ("retroarch", "https://flathub.org/apps/org.libretro.RetroArch"),
        ("pcsx2", "https://flathub.org/apps/net.pcsx2.PCSX2"),
        ("dolphin", "https://flathub.org/apps/org.DolphinEmu.dolphin-emu"),
        ("cemu", "https://flathub.org/apps/info.cemu.Cemu"),
        ("ppsspp", "https://flathub.org/apps/org.ppsspp.PPSSPP"),
        ("azahar", "https://flathub.org/apps/org.azahar_emu.Azahar"),
        ("rpcs3", "https://flathub.org/apps/net.rpcs3.RPCS3"),
        ("melonds", "https://flathub.org/apps/net.kuribo64.melonDS"),
        ("xemu", "https://flathub.org/apps/app.xemu.xemu"),
        ("mgba", "https://flathub.org/apps/io.mgba.mGBA"),
        ("rmg", "https://flathub.org/apps/com.github.Rosalie241.RMG"),
        ("simple64", "https://flathub.org/apps/io.github.simple64.simple64"),
    ]
}

#[cfg(all(test, unix))]
mod linux_tests {
    use super::*;
    use crate::systems::system_by_id;

    fn game(path: &str) -> Game {
        Game { id: "gba/x".into(), title: "X".into(), sort_key: "x".into(), letter: 'X', path: PathBuf::from(path), format: "gba".into(), region: None, revision: None, title_id: None, versions: vec![], updates: 0, dlc: 0, cover: None, video: None, meta: None, info: None, warnings: vec![] }
    }

    #[test]
    fn flatpak_emulators_and_appimages() {
        let base = std::env::temp_dir().join(format!("spine-fp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("flatpak/net.pcsx2.PCSX2/current")).unwrap();
        fs::create_dir_all(base.join("apps")).unwrap();
        fs::write(base.join("apps/Cemu-2.6-x86_64.AppImage"), b"").unwrap();
        let inst = Installed::detect_with_flatpaks(&[base.join("apps")], &[base.join("flatpak")]);
        assert_eq!(inst.flatpak.get("pcsx2").map(String::as_str), Some("net.pcsx2.PCSX2"));
        assert!(inst.path("cemu").is_some());
        // PS2 through the Flatpak: "flatpak run net.pcsx2.PCSX2 -batch ... game.iso"
        let p = plan(system_by_id("ps2").unwrap(), &game("/roms/ps2/Okami.iso"), &inst);
        assert_eq!(p.program, PathBuf::from("flatpak"));
        assert_eq!(&p.args[..3], &["run".to_string(), "--filesystem=/roms/ps2".to_string(), "net.pcsx2.PCSX2".to_string()]);
        assert_eq!(p.args.last().map(String::as_str), Some("/roms/ps2/Okami.iso"));
        assert!(p.blocked.is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn retroarch_cfg_paths() {
        let base = std::env::temp_dir().join(format!("spine-racfg-{}", std::process::id()));
        let _ = fs::create_dir_all(&base);
        fs::write(base.join("retroarch.cfg"), "libretro_directory = \":/mycores\"\nsystem_directory = \"default\"\n").unwrap();
        assert_eq!(cfg_path(&base.join("retroarch.cfg"), "libretro_directory", &base), Some(base.join("mycores")));
        assert_eq!(cfg_path(&base.join("retroarch.cfg"), "system_directory", &base), None);
        let _ = fs::remove_dir_all(&base);
    }
}

/// The useful end of an emulator's log: error lines first, else the last lines.
pub fn log_reason(log: &str) -> Option<String> {
    let lines: Vec<&str> = log.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return None;
    }
    let errors: Vec<&str> = lines.iter().copied().filter(|l| {
        let low = l.to_lowercase();
        low.contains("[error]") || low.contains("error:") || low.contains("failed") || low.contains("no such file") || low.contains("permission denied") || low.contains("could not")
    }).collect();
    let pick = if errors.is_empty() { &lines[lines.len().saturating_sub(4)..] } else { &errors[errors.len().saturating_sub(4)..] };
    Some(pick.join("\n").chars().take(800).collect())
}

#[cfg(test)]
mod launch_log_tests {
    use super::*;

    #[test]
    fn picks_error_lines() {
        let log = "[INFO] RetroArch 1.21\n[INFO] Loading content...\n[ERROR] Could not read content file \"/media/b/Roms/x.z64\".\n[ERROR] Failed to load content.\n[INFO] Exiting";
        let r = log_reason(log).unwrap();
        assert!(r.contains("Could not read content file") && r.contains("Failed to load content") && !r.contains("Exiting"));
        assert_eq!(log_reason("  \n"), None);
    }
}
