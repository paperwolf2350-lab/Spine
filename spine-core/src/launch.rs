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

/// Emulators found on this computer: id -> executable path.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Installed {
    pub found: BTreeMap<String, PathBuf>,
}

impl Installed {
    /// Look for emulators under the given folders (two levels deep, which covers
    /// "Emulators/PCSX2/pcsx2-qt.exe" and "RetroArch-Win64/retroarch.exe").
    pub fn detect(search: &[PathBuf]) -> Self {
        let mut found = BTreeMap::new();
        for dir in search {
            visit(dir, 0, &mut found);
        }
        Installed { found }
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
            if emu.exes.contains(&name.as_str()) {
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
    v.push(PathBuf::from("/usr/bin"));
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
            let Some(exe) = installed.path("retroarch") else {
                return LaunchPlan { emulator: format!("RetroArch · {core}"), program: PathBuf::from(def.exes[0]), args: vec![], blocked: Some("Install RetroArch.".into()) };
            };
            let core_file = exe.parent().unwrap_or(Path::new(".")).join("cores").join(if cfg!(windows) { format!("{core}_libretro.dll") } else { format!("{core}_libretro.so") });
            let blocked = (!core_file.exists()).then(|| format!("Download the \"{core}\" core in RetroArch (Main Menu → Online Updater → Core Downloader)."));
            let args = def.args.iter().map(|a| a.replace("{core}", &core_file.to_string_lossy()).replace("{rom}", &rom)).collect();
            LaunchPlan { emulator: format!("RetroArch · {core}"), program: exe.clone(), args, blocked }
        }
        Launcher::Standalone(id) => {
            let def = emulator(id).expect("unknown emulator id");
            let Some(exe) = installed.path(id) else {
                return LaunchPlan { emulator: def.name.into(), program: PathBuf::from(def.exes[0]), args: vec![], blocked: Some(format!("Install {}.", def.name)) };
            };
            let needs_id = def.args.iter().any(|a| a.contains("{title_id}"));
            let blocked = (needs_id && game.title_id.is_none()).then(|| format!("{} needs this game's title id first.", def.name));
            let tid = game.title_id.clone().unwrap_or_default();
            let args = def.args.iter().map(|a| a.replace("{rom}", &rom).replace("{title_id}", &tid)).collect();
            LaunchPlan { emulator: def.name.into(), program: exe.clone(), args, blocked }
        }
    }
}

/// Start the game. Returns the child process so the app can track playtime.
pub fn launch(plan: &LaunchPlan) -> std::io::Result<std::process::Child> {
    if let Some(why) = &plan.blocked {
        return Err(std::io::Error::other(why.clone()));
    }
    if plan.emulator == "system" {
        let target = plan.program.to_string_lossy().to_string();
        // A store game's own .exe (GOG): start it from its folder so it finds its files,
        // and Spine can time the session.
        if target.to_lowercase().ends_with(".exe") && !target.contains("://") {
            let mut cmd = std::process::Command::new(&plan.program);
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
        return std::process::Command::new("xdg-open").arg(&plan.program).spawn();
    }
    let mut cmd = std::process::Command::new(&plan.program);
    cmd.args(&plan.args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no stray console window behind the emulator
    }
    if let Some(dir) = plan.program.parent() {
        cmd.current_dir(dir); // Several emulators expect to run from their own folder.
    }
    cmd.spawn()
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
        return download_pages().iter().find(|(id, _)| *id == "retroarch").map(|(_, u)| *u);
    }
    let name = why.strip_prefix("Install ")?.trim_end_matches('.');
    let id = EMULATORS.iter().find(|e| e.name == name)?.id;
    download_pages().iter().find(|(i, _)| *i == id).map(|(_, u)| *u)
}
