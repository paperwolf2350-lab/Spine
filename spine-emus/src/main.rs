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

#[derive(Serialize)]
struct Boot {
    dir: String,
    windows: bool,
    emus: Vec<EmuView>,
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
    Boot { dir: app.dir.display().to_string(), windows: cfg!(windows), emus: views(&app.dir) }
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
    std::thread::spawn(move || {
        for id in ids {
            let Some(def) = emus::emu(&id) else { continue };
            let h = handle.clone();
            let id2 = id.clone();
            let step = move |s: Step| {
                let p = match s {
                    Step::Text(t) => Progress { id: id2.clone(), text: t, got: 0, total: 0 },
                    Step::Bytes(got, total) => Progress { id: id2.clone(), text: String::new(), got, total },
                };
                let _ = h.emit("emu-progress", p);
            };
            let done = match emus::install(&dir, def, &step) {
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
        .invoke_handler(tauri::generate_handler![boot, install, open_folder, open_site])
        .run(tauri::generate_context!())
        .expect("error while running Spine Emulators");
}
