//! Spine Emulators: a small window that installs popular emulators from their official
//! releases into one folder Spine searches. The work lives in `spine_core::emus`.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use serde::Serialize;
use spine_core::emus::{self, Step};
use spine_core::launch::{self, Installed};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

struct App {
    dir: PathBuf,
    busy: Mutex<bool>,
}

#[derive(Serialize)]
struct EmuView {
    id: &'static str,
    name: &'static str,
    consoles: &'static str,
    group: &'static str,
    site: &'static str,
    /// Version this installer put in the folder (None if it didn't).
    version: Option<String>,
    /// Program found in our folder.
    ours: Option<String>,
    /// Program found somewhere else Spine looks (Program Files, EmuDeck...).
    elsewhere: Option<String>,
}

/// A RetroArch core (installed into our RetroArch's cores folder).
#[derive(Serialize)]
struct CoreView {
    id: &'static str,
    name: &'static str,
    consoles: &'static str,
    installed: bool,
    /// Build date of the copy this installer put there.
    version: Option<String>,
}

#[derive(Serialize)]
struct Boot {
    dir: String,
    windows: bool,
    emus: Vec<EmuView>,
    cores: Vec<CoreView>,
}

fn core_views(dir: &PathBuf) -> Vec<CoreView> {
    let stamps = emus::read_stamps(dir);
    emus::CORES
        .iter()
        .map(|c| {
            let installed = emus::core_installed(dir, c.id);
            CoreView { id: c.id, name: c.name, consoles: c.consoles, installed, version: installed.then(|| stamps.get(&emus::core_key(c.id)).map(|s| s.version.clone())).flatten() }
        })
        .collect()
}

fn views(dir: &PathBuf) -> Vec<EmuView> {
    let stamps = emus::read_stamps(dir);
    // Same places Spine looks (a fake ROM folder inside ours, so its parent is searched too).
    let mut search = vec![dir.clone()];
    search.extend(launch::default_search_paths(&dir.join("roms")));
    let found = Installed::detect(&search);
    emus::EMUS
        .iter()
        .map(|e| {
            let ours = emus::find_exe(&emus::target(dir, e), e);
            let elsewhere = found.path(e.id).filter(|p| ours.as_ref() != Some(*p)).map(|p| p.display().to_string());
            EmuView {
                id: e.id,
                name: e.name,
                consoles: e.consoles,
                group: e.group,
                site: e.site,
                version: ours.as_ref().and_then(|_| stamps.get(e.id).map(|s| s.version.clone())),
                ours: ours.map(|p| p.display().to_string()),
                elsewhere,
            }
        })
        .collect()
}

#[tauri::command]
fn boot(app: tauri::State<App>) -> Boot {
    Boot { dir: app.dir.display().to_string(), windows: cfg!(windows), emus: views(&app.dir), cores: core_views(&app.dir) }
}

#[derive(Clone, Serialize)]
struct Progress {
    id: String,
    text: String,
    got: u64,
    total: u64,
}

#[derive(Clone, Serialize)]
struct Done {
    id: String,
    ok: bool,
    msg: String,
}

/// Install the chosen emulators one after another; the window follows along through
/// "emu-progress", "emu-done" and finally "emu-finished" events.
#[tauri::command]
fn install(handle: AppHandle, app: tauri::State<App>, ids: Vec<String>) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("Spine Emulators installs Windows versions. On Linux, get emulators from Flathub.".into());
    }
    {
        let mut b = app.busy.lock().unwrap();
        if *b {
            return Err("Already installing.".into());
        }
        *b = true;
    }
    let dir = app.dir.clone();
    // Emulators first (RetroArch before its cores), then cores ("core:<id>").
    let (core_ids, mut emu_ids): (Vec<String>, Vec<String>) = ids.into_iter().partition(|i| i.starts_with("core:"));
    let ra = emus::emu("retroarch").expect("retroarch");
    if !core_ids.is_empty() && emus::find_exe(&emus::target(&dir, ra), ra).is_none() && !emu_ids.iter().any(|i| i == "retroarch") {
        emu_ids.insert(0, "retroarch".into());
    }
    std::thread::spawn(move || {
        let progress = |id: &str| {
            let h = handle.clone();
            let id = id.to_string();
            move |s: Step| {
                let p = match s {
                    Step::Text(t) => Progress { id: id.clone(), text: t, got: 0, total: 0 },
                    Step::Bytes(got, total) => Progress { id: id.clone(), text: String::new(), got, total },
                };
                let _ = h.emit("emu-progress", p);
            }
        };
        for id in emu_ids {
            let Some(def) = emus::emu(&id) else { continue };
            let done = match emus::install(&dir, def, &progress(&id)) {
                Ok((_, version)) => Done { id, ok: true, msg: version },
                Err(e) => Done { id, ok: false, msg: e },
            };
            let _ = handle.emit("emu-done", done);
        }
        // libretro's core list is fetched once for all the cores.
        let mut index = None;
        for id in core_ids {
            let Some(c) = id.strip_prefix("core:").and_then(emus::core) else { continue };
            if index.is_none() {
                progress(&id)(Step::Text("Getting libretro's core list…".into()));
                index = Some(emus::core_index());
            }
            let r = match index.as_ref().unwrap() {
                Ok(ix) => emus::install_core(&dir, c, ix, &progress(&id)),
                Err(e) => Err(e.clone()),
            };
            let done = match r {
                Ok((_, version)) => Done { id, ok: true, msg: version },
                Err(e) => Done { id, ok: false, msg: e },
            };
            let _ = handle.emit("emu-done", done);
        }
        *handle.state::<App>().busy.lock().unwrap() = false;
        let _ = handle.emit("emu-finished", ());
    });
    Ok(())
}

#[derive(Serialize)]
struct UpdatesView {
    /// When the newest versions were asked (unix seconds).
    at: u64,
    /// id -> newer version, for items installed here that have one.
    updates: std::collections::BTreeMap<String, String>,
    /// id -> why it couldn't be checked.
    errors: std::collections::BTreeMap<String, String>,
}

/// Which installed emulators and cores have a newer version. Uses the saved answer when
/// it's under an hour old, unless `force` (the "Check again" link).
#[tauri::command]
async fn check_updates(handle: AppHandle, force: bool) -> Result<UpdatesView, String> {
    let dir = handle.state::<App>().dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let l = emus::latest_versions(&dir, force);
        UpdatesView { at: l.at, updates: emus::updates(&dir, &l), errors: l.errors }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Show a folder in Explorer (our emulators folder, or one emulator's).
#[tauri::command]
fn open_folder(app: tauri::State<App>, id: Option<String>) -> Result<(), String> {
    let path = match id.as_deref().and_then(emus::emu) {
        Some(def) => emus::target(&app.dir, def),
        None => app.dir.clone(),
    };
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    spine_core::hostenv::command(opener).arg(&path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open an emulator's website in the browser (only the addresses in our own list).
#[tauri::command]
fn open_site(id: String) -> Result<(), String> {
    let def = emus::emu(&id).ok_or("unknown emulator")?;
    // explorer.exe hands a web address to the default browser (no console window flashes).
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    spine_core::hostenv::command(opener).arg(def.site).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn main() {
    let dir = emus::default_dir().unwrap_or_else(|| PathBuf::from("Emulators"));
    tauri::Builder::default()
        .manage(App { dir, busy: Mutex::new(false) })
        .invoke_handler(tauri::generate_handler![boot, install, check_updates, open_folder, open_site])
        .run(tauri::generate_context!())
        .expect("error while running Spine Emulators");
}
