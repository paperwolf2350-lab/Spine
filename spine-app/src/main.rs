//! Spine desktop app: a Tauri window around spine-core.
//!
//! Spine is "portable": everything it keeps (settings, favorites, playtime,
//! covers, credentials) lives in the folder next to Spine.exe.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use spine_core::config::Credentials;
use spine_core::launch::{self, Installed};
use spine_core::{covers, screenscraper, systems, Library};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

mod pad;
mod update;
#[cfg(target_os = "linux")]
mod linux;

// ---------------------------------------------------------------------------
// Saved state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    rom_root: Option<String>,
    #[serde(default)]
    favorites: BTreeSet<String>,
    /// Seconds played per game id.
    #[serde(default)]
    playtime: BTreeMap<String, u64>,
    /// Unix time each game was last played.
    #[serde(default)]
    last_played: BTreeMap<String, u64>,
    /// Per-game emulator choice: game id -> launcher key ("retroarch:mgba", "pcsx2").
    #[serde(default)]
    overrides: BTreeMap<String, String>,
    /// Free-form UI settings (theme, accent, layout per console...).
    #[serde(default)]
    settings: serde_json::Map<String, serde_json::Value>,
    /// More ROM folders besides `rom_root` (other drives, other folders).
    #[serde(default)]
    extra_roots: Vec<String>,
    /// Games the user hid (junk files, duplicates).
    #[serde(default)]
    hidden: BTreeSet<String>,
    /// Names the user typed for games Spine named wrong.
    #[serde(default)]
    renames: BTreeMap<String, String>,
    /// When each game first showed up (for "Recently added").
    #[serde(default)]
    first_seen: BTreeMap<String, u64>,
    /// The user's own lists: name -> game ids, in the order added.
    #[serde(default)]
    collections: BTreeMap<String, Vec<String>>,
    /// Folders with movies in them (Settings → Movies). Kept apart from ROM folders.
    #[serde(default)]
    movie_roots: Vec<String>,
    /// Folders with TV shows in them (Settings → TV): one show per folder inside.
    #[serde(default)]
    tv_roots: Vec<String>,
    /// Which TMDB season each arc folder is: "<show id>|<arc name>" -> season.
    #[serde(default)]
    arc_seasons: BTreeMap<String, spine_core::tv::ArcMap>,
    /// Names the user typed for movies in show folders: "<show id>|<file name>" -> name.
    #[serde(default)]
    special_queries: BTreeMap<String, String>,
}

impl Saved {
    fn roots(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.rom_root.iter().chain(self.extra_roots.iter()).map(PathBuf::from).collect();
        v.dedup();
        v
    }
    fn movie_roots(&self) -> Vec<PathBuf> {
        self.movie_roots.iter().map(PathBuf::from).collect()
    }
    fn tv_roots(&self) -> Vec<PathBuf> {
        self.tv_roots.iter().map(PathBuf::from).collect()
    }
    /// Nothing to show yet: no ROM, movie or TV folder.
    fn empty(&self) -> bool {
        self.roots().is_empty() && self.movie_roots.is_empty() && self.tv_roots.is_empty()
    }
    /// Already used for games, movies or TV.
    fn uses(&self, p: &Path) -> Option<&'static str> {
        if self.roots().iter().any(|r| r == p) {
            Some("one of your ROM folders")
        } else if self.movie_roots().iter().any(|r| r == p) {
            Some("in Movies")
        } else if self.tv_roots().iter().any(|r| r == p) {
            Some("in TV")
        } else {
            None
        }
    }
}

struct App {
    dir: PathBuf,
    saved: Mutex<Saved>,
    library: Mutex<Option<Library>>,
    installed: Mutex<Installed>,
    running: Mutex<Option<String>>,
    fetching: Mutex<bool>,
    /// ROM folders whose drive isn't connected right now.
    offline: Mutex<Vec<PathBuf>>,
}

impl App {
    fn media(&self) -> PathBuf {
        self.dir.join("media")
    }
    fn save(&self) {
        let s = self.saved.lock().unwrap().clone();
        let tmp = self.dir.join("spine-state.json.tmp");
        if fs::write(&tmp, serde_json::to_string_pretty(&s).unwrap_or_default()).is_ok() {
            let _ = fs::rename(&tmp, self.dir.join("spine-state.json"));
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where Spine keeps its settings, covers and favorites: next to Spine.exe on
/// Windows (portable), ~/.local/share/spine on Linux.
fn app_dir() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "linux")]
    return linux::data_dir(&exe_dir);
    #[cfg(not(target_os = "linux"))]
    exe_dir
}

// ---------------------------------------------------------------------------
// What the interface sees
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct GameView {
    id: String,
    title: String,
    letter: char,
    format: String,
    file: String,
    region: Option<String>,
    cover: Option<String>,
    custom_cover: bool,
    /// URL path of the game's video ("video/<id>?v=..."), if it has one.
    video: Option<String>,
    /// Separate soundtrack for videos stitched from streaming trailers.
    video_audio: Option<String>,
    custom_video: bool,
    /// The emulator this game launches with when it differs from the console's.
    launcher: Option<String>,
    copies: usize,
    updates: usize,
    dlc: usize,
    warnings: Vec<String>,
    info: Option<screenscraper::GameInfo>,
    meta: Option<spine_core::meta::GameMeta>,
    /// Wide banner and logo for the game page, and screenshots.
    hero: Option<String>,
    logo: Option<String>,
    shots: Vec<String>,
    /// Its drive isn't connected right now.
    offline: bool,
    /// Unix time it first showed up (0 = was there from the start).
    added: u64,
    /// The user gave it its name.
    renamed: bool,
    /// TV shows: episodes in viewing order (the index is what `play` takes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    episodes: Vec<EpisodeView>,
    /// TV shows: TMDB's seasons, for "Which season is this?" on arcs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tmdb_seasons: Vec<spine_core::tv::TmdbSeason>,
    /// TV shows: the arcs the user matched, by arc name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    arcs: BTreeMap<String, spine_core::tv::ArcMap>,
}

#[derive(Serialize, Clone)]
struct EpisodeView {
    group: String,
    season: Option<u32>,
    number: Option<u32>,
    /// TMDB's name, else the one in the file name.
    title: Option<String>,
    overview: Option<String>,
    still: Option<String>,
    file: String,
    /// Movies & specials: "Movie · 2021 · 1 h 45 min", or why there are no details.
    note: Option<String>,
    /// Movies & specials: what Spine searched TMDB for (to fix a wrong match).
    query: Option<String>,
}

#[derive(Serialize, Clone)]
struct RootView {
    path: String,
    online: bool,
}

#[derive(Serialize, Clone)]
struct HiddenView {
    id: String,
    title: String,
    system: &'static str,
}

#[derive(Serialize, Clone)]
struct SystemView {
    id: &'static str,
    name: &'static str,
    short: &'static str,
    color: &'static str,
    emulator: String,
    blocked: Option<String>,
    /// A BIOS / keys file this console's emulator still needs.
    bios: Option<spine_core::bios::BiosIssue>,
    /// Where to get the missing emulator.
    download: Option<&'static str>,
    games: Vec<GameView>,
}

#[derive(Serialize, Clone)]
struct Boot {
    rom_root: Option<String>,
    systems: Vec<SystemView>,
    favorites: BTreeSet<String>,
    playtime: BTreeMap<String, u64>,
    last_played: BTreeMap<String, u64>,
    settings: serde_json::Map<String, serde_json::Value>,
    notes: Vec<String>,
    roots: Vec<RootView>,
    movie_roots: Vec<RootView>,
    tv_roots: Vec<RootView>,
    hidden: Vec<HiddenView>,
    collections: BTreeMap<String, Vec<String>>,
}

fn view(app: &App) -> Boot {
    let saved = app.saved.lock().unwrap().clone();
    let lib = app.library.lock().unwrap();
    let installed = app.installed.lock().unwrap();
    let offline = app.offline.lock().unwrap().clone();
    let media = app.media();
    let is_offline = |p: &Path| offline.iter().any(|r| p.starts_with(r));
    let mut hidden: Vec<HiddenView> = vec![];
    // Relative media path, plus a version stamp so a changed cover shows right away.
    let rel = |p: &Path| {
        let r = p.strip_prefix(&media).ok()?.to_string_lossy().replace('\\', "/");
        let v = fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        Some(format!("{r}?v={v}"))
    };
    let systems = lib
        .as_ref()
        .map(|l| {
            l.systems
                .iter()
                .map(|s| {
                    let def = systems::system_by_id(s.id).unwrap();
                    let plan = launch::plan(def, &s.games[0], &installed);
                    SystemView {
                        id: s.id,
                        name: s.name,
                        short: s.short,
                        color: s.color,
                        bios: spine_core::bios::check(s.id, &plan),
                        download: launch::download_for(&plan),
                        emulator: plan.emulator,
                        blocked: plan.blocked,
                        games: s
                            .games
                            .iter()
                            .filter(|g| {
                                if saved.hidden.contains(&g.id) {
                                    hidden.push(HiddenView { id: g.id.clone(), title: g.title.clone(), system: s.short });
                                    false
                                } else {
                                    true
                                }
                            })
                            .map(|g| GameView {
                                id: g.id.clone(),
                                title: g.title.clone(),
                                letter: g.letter,
                                format: g.format.clone(),
                                file: match g.format.as_str() {
                                    "steam" => "Installed on Steam".into(),
                                    "epic" if g.path.to_string_lossy().starts_with("heroic://") => "Installed with Heroic (Epic Games)".into(),
                                    "gog" if g.path.to_string_lossy().starts_with("heroic://") => "Installed with Heroic (GOG)".into(),
                                    "epic" => "Installed on Epic Games".into(),
                                    "gog" => format!("Installed from GOG · {}", g.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()),
                                    _ => g.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
                                },
                                region: g.region.clone(),
                                cover: g.cover.as_deref().and_then(rel),
                                custom_cover: covers::custom_cover(&media, &g.id).is_some(),
                                video: g.video.as_ref().map(|p| {
                                    let v = fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                                    format!("video/{}?v={v}", g.id)
                                }),
                                video_audio: g.video.as_ref().filter(|p| p.with_extension("audio.m4a").is_file()).map(|_| format!("video-audio/{}", g.id)),
                                custom_video: covers::custom_video(&media, &g.id).is_some(),
                                launcher: saved.overrides.get(&g.id).and_then(|k| {
                                    let p = launch::plan_preferred(def, g, &installed, Some(k));
                                    Some(p.emulator)
                                }),
                                copies: g.versions.len(),
                                updates: g.updates,
                                dlc: g.dlc,
                                warnings: g.warnings.clone(),
                                info: g.info.clone(),
                                meta: g.meta.clone(),
                                hero: spine_core::extras::existing_hero(&media, &g.id).as_deref().and_then(rel),
                                logo: spine_core::extras::existing_logo(&media, &g.id).as_deref().and_then(rel),
                                shots: spine_core::extras::existing_shots(&media, &g.id).iter().filter_map(|p| rel(p)).collect(),
                                offline: is_offline(&g.path),
                                added: saved.first_seen.get(&g.id).copied().unwrap_or(0),
                                renamed: saved.renames.contains_key(&g.id),
                                episodes: if g.episodes.is_empty() { vec![] } else { episode_views(g, &media, &rel, &saved.arc_seasons) },
                                tmdb_seasons: if g.episodes.iter().any(|e| e.season.is_none() && e.number.is_some()) { spine_core::tv::load_seasons(&media, &g.id).map(|s| s.list).unwrap_or_default() } else { vec![] },
                                arcs: saved.arc_seasons.iter().filter_map(|(k, v)| k.strip_prefix(&format!("{}|", g.id)).map(|a| (a.to_string(), *v))).collect(),
                            })
                            .collect(),
                    }
                })
                .filter(|s: &SystemView| !s.games.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let roots = saved.roots().iter().map(|r| RootView { path: r.to_string_lossy().to_string(), online: !offline.contains(r) }).collect();
    let movie_roots = saved.movie_roots().iter().map(|r| RootView { path: r.to_string_lossy().to_string(), online: !offline.contains(r) }).collect();
    let tv_roots = saved.tv_roots().iter().map(|r| RootView { path: r.to_string_lossy().to_string(), online: !offline.contains(r) }).collect();
    Boot {
        roots,
        movie_roots,
        tv_roots,
        hidden,
        collections: saved.collections.clone(),
        rom_root: saved.rom_root,
        systems,
        favorites: saved.favorites,
        playtime: saved.playtime,
        last_played: saved.last_played,
        settings: saved.settings,
        notes: lib.as_ref().map(|l| l.notes.clone()).unwrap_or_default(),
    }
}

/// A show's episodes, with names, summaries and thumbnails from TMDB where known.
fn episode_views(g: &spine_core::Game, media: &Path, rel: &dyn Fn(&Path) -> Option<String>, arcs: &BTreeMap<String, spine_core::tv::ArcMap>) -> Vec<EpisodeView> {
    let tmdb = spine_core::tv::load_seasons(media, &g.id);
    let arc = |group: &str| arcs.get(&format!("{}|{group}", g.id)).copied();
    let matched = tmdb.as_ref().map(|t| spine_core::tv::resolve(g, t, &arc)).unwrap_or_else(|| vec![None; g.episodes.len()]);
    g.episodes
        .iter()
        .zip(matched)
        .map(|(e, m)| {
            let t = match (m, &tmdb) {
                (Some((s, n)), Some(t)) => t.seasons.get(&s).and_then(|v| v.iter().find(|x| x.number == n)),
                _ => None,
            };
            let still = m.map(|(s, n)| spine_core::tv::still_path(media, &g.id, s, n)).filter(|p| p.is_file()).and_then(|p| rel(&p));
            let file = e.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            // A movie in the show's folder: what TMDB's movies said about it.
            if e.number.is_none() {
                let sp = tmdb.as_ref().and_then(|t| t.specials.get(&file));
                let found = sp.filter(|x| x.tmdb != 0);
                let note = match (sp, found) {
                    (_, Some(x)) => {
                        let mut parts = vec!["Movie".to_string()];
                        parts.extend(x.year.clone());
                        if let Some(m) = x.runtime {
                            parts.push(if m >= 60 { format!("{} h {} min", m / 60, m % 60) } else { format!("{m} min") });
                        }
                        Some(parts.join(" · "))
                    }
                    (Some(_), None) => Some("Not found on TMDB. Use Fix match to search for it by name.".into()),
                    (None, None) => None,
                };
                let still = Some(spine_core::tv::special_still(media, &g.id, &file)).filter(|p| p.is_file()).and_then(|p| rel(&p));
                return EpisodeView {
                    group: e.group.clone(),
                    season: None,
                    number: None,
                    title: found.and_then(|x| x.name.clone()).or_else(|| e.title.clone()),
                    overview: found.and_then(|x| x.overview.clone()),
                    still,
                    note,
                    query: sp.and_then(|x| x.query.clone()).or_else(|| spine_core::tv::special_queries(e).into_iter().next()),
                    file,
                };
            }
            EpisodeView {
                group: e.group.clone(),
                season: e.season,
                number: e.number,
                title: t.and_then(|t| t.name.clone()).or_else(|| e.title.clone()),
                overview: t.and_then(|t| t.overview.clone()),
                still,
                file,
                note: None,
                query: None,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn boot(app: State<App>) -> Boot {
    view(&app)
}

#[tauri::command]
async fn pick_folder(handle: AppHandle) -> Option<String> {
    handle.dialog().file().set_title("Choose your ROM folder").blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().to_string())
}

/// Scan a folder (fast: no network). Returns the new view.
#[tauri::command]
async fn scan(root: String, handle: AppHandle) -> Result<Boot, String> {
    let app = handle.state::<App>();
    let root_path = PathBuf::from(&root);
    fs::read_dir(&root_path).map_err(|e| format!("Couldn't read {root}: {e}"))?;
    {
        let mut s = app.saved.lock().unwrap();
        s.extra_roots.retain(|r| r != &root);
        s.rom_root = Some(root);
    }
    app.save();
    reload(&app);
    Ok(view(&app))
}

/// Add another ROM folder (another drive, another folder).
#[tauri::command]
async fn add_root(handle: AppHandle) -> Result<Option<Boot>, String> {
    let Some(p) = handle.dialog().file().set_title("Add another ROM folder").blocking_pick_folder().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let app = handle.state::<App>();
    let path = p.to_string_lossy().to_string();
    {
        let mut s = app.saved.lock().unwrap();
        if s.roots().iter().any(|r| r == &p) {
            return Err("That folder is already in your library.".into());
        }
        if s.rom_root.is_none() {
            s.rom_root = Some(path);
        } else {
            s.extra_roots.push(path);
        }
    }
    app.save();
    reload(&app);
    Ok(Some(view(&app)))
}

/// Stop reading a ROM folder (nothing on disk is touched).
#[tauri::command]
fn remove_root(path: String, app: State<App>) -> Boot {
    {
        let mut s = app.saved.lock().unwrap();
        if s.rom_root.as_deref() == Some(path.as_str()) {
            s.rom_root = if s.extra_roots.is_empty() { None } else { Some(s.extra_roots.remove(0)) };
        } else {
            s.extra_roots.retain(|r| r != &path);
        }
    }
    app.save();
    reload(&app);
    view(&app)
}

/// Add a folder with movies in it.
#[tauri::command]
async fn add_movie_root(handle: AppHandle) -> Result<Option<Boot>, String> {
    let Some(p) = handle.dialog().file().set_title("Choose a folder with movies").blocking_pick_folder().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let app = handle.state::<App>();
    {
        let mut s = app.saved.lock().unwrap();
        if let Some(w) = s.uses(&p) {
            return Err(format!("That folder is already {w}."));
        }
        s.movie_roots.push(p.to_string_lossy().to_string());
    }
    app.save();
    reload(&app);
    Ok(Some(view(&app)))
}

/// Stop reading a movie folder (nothing on disk is touched).
#[tauri::command]
fn remove_movie_root(path: String, app: State<App>) -> Boot {
    app.saved.lock().unwrap().movie_roots.retain(|r| r != &path);
    app.save();
    reload(&app);
    view(&app)
}

/// Add a folder with TV shows in it (one show per folder inside).
#[tauri::command]
async fn add_tv_root(handle: AppHandle) -> Result<Option<Boot>, String> {
    let Some(p) = handle.dialog().file().set_title("Choose a folder with TV shows").blocking_pick_folder().and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let app = handle.state::<App>();
    {
        let mut s = app.saved.lock().unwrap();
        if let Some(w) = s.uses(&p) {
            return Err(format!("That folder is already {w}."));
        }
        s.tv_roots.push(p.to_string_lossy().to_string());
    }
    app.save();
    reload(&app);
    Ok(Some(view(&app)))
}

/// Stop reading a TV folder (nothing on disk is touched).
#[tauri::command]
fn remove_tv_root(path: String, app: State<App>) -> Boot {
    app.saved.lock().unwrap().tv_roots.retain(|r| r != &path);
    app.save();
    reload(&app);
    view(&app)
}

/// "Fix match" for a movie in a show's folder: search TMDB's movies for this name next time.
#[tauri::command]
fn set_special_query(id: String, file: String, query: String, app: State<App>) -> Boot {
    let q = query.trim().to_string();
    {
        let mut s = app.saved.lock().unwrap();
        let k = format!("{id}|{file}");
        if q.is_empty() {
            s.special_queries.remove(&k);
        } else {
            s.special_queries.insert(k, q);
        }
    }
    app.save();
    spine_core::tv::forget_special(&app.media(), &id, &file);
    view(&app)
}

/// Say which TMDB season an arc folder is (None: forget the match).
#[tauri::command]
fn set_arc_season(id: String, group: String, season: Option<u32>, start: Option<u32>, app: State<App>) -> Boot {
    {
        let mut s = app.saved.lock().unwrap();
        let k = format!("{id}|{group}");
        match season {
            Some(season) => {
                s.arc_seasons.insert(k, spine_core::tv::ArcMap { season, start: start.unwrap_or(1).max(1) });
            }
            None => {
                s.arc_seasons.remove(&k);
            }
        }
    }
    app.save();
    view(&app)
}

/// Download missing covers / details in the background. Emits
/// "fetch-progress" {stage, done, total, title} and "library-updated" at the end.
#[tauri::command]
fn fetch_media(handle: AppHandle) {
    {
        let app = handle.state::<App>();
        let mut f = app.fetching.lock().unwrap();
        if *f {
            return;
        }
        *f = true;
    }
    std::thread::spawn(move || {
        let app = handle.state::<App>();
        let media = app.media();
        let Some(mut lib) = app.library.lock().unwrap().clone() else {
            *app.fetching.lock().unwrap() = false;
            return;
        };
        let creds = Credentials::load(&app.dir.join("credentials.txt"));
        let emit = |stage: &str, done: usize, total: usize, title: &str| {
            let _ = handle.emit("fetch-progress", serde_json::json!({"stage": stage, "done": done, "total": total, "title": title}));
        };
        // Movies get their art from TMDB only, never from the game art sources.
        let is_video_shelf = |id: &str| id == spine_core::movies::SYSTEM_ID || id == spine_core::tv::SYSTEM_ID;
        let videos = Library { root: lib.root.clone(), systems: lib.systems.iter().filter(|s| is_video_shelf(s.id)).cloned().collect(), notes: vec![], game_count: 0 };
        lib.systems.retain(|s| !is_video_shelf(s.id));
        let mut movie_error = None;
        if let Some(key) = &creds.tmdb_key {
            let r = spine_core::movies::fetch_details(&videos, &media, key, &|d, t, title| emit("movies", d, t, title))
                .and_then(|_| {
                    let (arcs, queries) = {
                        let s = app.saved.lock().unwrap();
                        (s.arc_seasons.clone(), s.special_queries.clone())
                    };
                    spine_core::tv::fetch_details(&videos, &media, key, &|id, group| arcs.get(&format!("{id}|{group}")).copied(), &|id, file| queries.get(&format!("{id}|{file}")).cloned(), &|d, t, title| emit("tv", d, t, title))
                });
            if let Err(e) = r {
                movie_error = Some(e);
            }
        }
        covers::fetch_covers(&mut lib, &media, &|d, t, title| emit("covers", d, t, title));
        covers::fetch_steam_videos(&mut lib, &media, &|d, t, title| emit("videos", d, t, title));
        spine_core::extras::fetch_extras(&lib, &media, creds.steamgriddb_key.as_deref(), &|d, t, title| emit("art", d, t, title));
        let metas = spine_core::meta::build(&lib, &media, None, &|d, t, title| emit("categories", d, t, title));
        spine_core::meta::save(&media, &metas);
        spine_core::meta::fetch_pc_details(&lib, &media, &|d, t, title| emit("details", d, t, title));
        if creds.screenscraper_ready() {
            screenscraper::fetch_details(&lib, &media, &creds, &|d, t, title| emit("details", d, t, title));
        }
        if let Some(key) = &creds.steamgriddb_key {
            covers::fetch_sgdb_covers(&mut lib, &media, key, &|d, t, title| emit("covers", d, t, title));
        }
        // Re-read from disk rather than keep this copy: folders may have changed meanwhile.
        drop(lib);
        reload(&app);
        *app.fetching.lock().unwrap() = false;
        if let Some(e) = movie_error {
            let _ = handle.emit("fetch-error", e);
        }
        let _ = handle.emit("library-updated", view(&app));
    });
}

#[tauri::command]
fn toggle_favorite(id: String, app: State<App>) -> bool {
    let fav = {
        let mut s = app.saved.lock().unwrap();
        if !s.favorites.remove(&id) {
            s.favorites.insert(id);
            true
        } else {
            false
        }
    };
    app.save();
    fav
}

#[tauri::command]
fn set_setting(key: String, value: serde_json::Value, app: State<App>) {
    app.saved.lock().unwrap().settings.insert(key, value);
    app.save();
}

/// Start a game. Emits "game-exited" {id, seconds} when the emulator closes.
#[tauri::command]
fn play(id: String, episode: Option<usize>, handle: AppHandle) -> Result<String, String> {
    let app = handle.state::<App>();
    if let Some(r) = app.running.lock().unwrap().as_ref() {
        return Err(format!("{r} is still running."));
    }
    let is_movie = id.starts_with("movies/") || id.starts_with("tv/");
    let (plan, title) = {
        let lib = app.library.lock().unwrap();
        let lib = lib.as_ref().ok_or("No library yet.")?;
        let (sys, game) = lib.systems.iter().find_map(|s| s.games.iter().find(|g| g.id == id).map(|g| (s, g))).ok_or("Game not found.")?;
        if let Some(root) = app.offline.lock().unwrap().iter().find(|r| game.path.starts_with(r)) {
            let what = if is_movie { "movie" } else { "game" };
            return Err(format!("This {what} is on {}, which isn't connected. Plug in that drive and try again.", root.display()));
        }
        let def = systems::system_by_id(sys.id).unwrap();
        let pref = app.saved.lock().unwrap().overrides.get(&id).cloned();
        // A TV episode plays like its show, with the episode's file.
        let mut game = game.clone();
        if let Some(i) = episode {
            game.path = game.episodes.get(i).ok_or("Episode not found.")?.path.clone();
        }
        (launch::plan_preferred(def, &game, &app.installed.lock().unwrap(), pref.as_deref()), game.title.clone())
    };
    if let Some(why) = &plan.blocked {
        return Err(why.clone());
    }
    // The emulator's own messages go to logs/last-game.log, so a game that closes
    // right away can say why.
    let log = app.dir.join("logs").join("last-game.log");
    let _ = fs::create_dir_all(log.parent().unwrap());
    let logged = (plan.emulator != "system").then_some(log.as_path());
    let mut child = launch::launch_logged(&plan, logged).map_err(|e| format!("Couldn't start {}: {e}", plan.emulator))?;
    *app.running.lock().unwrap() = Some(title);
    {
        let mut s = app.saved.lock().unwrap();
        s.last_played.insert(id.clone(), now());
    }
    app.save();
    let emulator = plan.emulator.clone();
    let h = handle.clone();
    // Hold Select + Start on the controller to close the game and come back.
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let own_exe = plan.program.to_string_lossy().to_lowercase().ends_with(".exe");
    let combo_on = app.saved.lock().unwrap().settings.get("exitCombo").and_then(|v| v.as_bool()) != Some(false);
    if combo_on && (plan.emulator != "system" || own_exe) {
        let pid = child.id();
        let (d, hh) = (done.clone(), handle.clone());
        pad::watch(done.clone(), move || {
            let _ = hh.emit("game-closing", ());
            pad::close_game(pid, &|| !d.load(std::sync::atomic::Ordering::Relaxed));
        });
    }
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let status = child.wait();
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        let secs = started.elapsed().as_secs();
        // Closed within a few seconds without the player quitting: tell them why.
        let quick = plan.emulator != "system" && started.elapsed() < std::time::Duration::from_secs(6);
        let failed = quick && status.as_ref().map_or(true, |s| !s.success());
        let error = (quick && (failed || secs < 3)).then(|| {
            let reason = fs::read_to_string(&log).ok().and_then(|t| launch::log_reason(&t));
            let what = if is_movie { "movie" } else { "game" };
            let mut msg = format!("{} closed right away, so the {what} didn't start.", plan.emulator);
            if let Some(r) = reason {
                msg.push_str("\n\nWhat it said:\n");
                msg.push_str(&r);
            }
            msg.push_str(&format!("\n\nFull details: {}", log.display()));
            msg
        });
        let app = h.state::<App>();
        // Shell-opened PC games return immediately; don't count those seconds.
        if plan.emulator != "system" || own_exe {
            *app.saved.lock().unwrap().playtime.entry(id.clone()).or_insert(0) += secs;
            app.save();
        }
        *app.running.lock().unwrap() = None;
        let _ = h.emit("game-exited", serde_json::json!({"id": id, "seconds": secs, "error": error}));
        if let Some(w) = h.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    });
    Ok(emulator)
}

/// Give a game a different name (None = go back to the name from the file).
#[tauri::command]
fn rename_game(id: String, title: Option<String>, app: State<App>) -> Result<Boot, String> {
    {
        let mut s = app.saved.lock().unwrap();
        match title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            Some(t) if t.chars().count() > 200 => return Err("That name is too long.".into()),
            Some(t) => {
                s.renames.insert(id, t);
            }
            None => {
                s.renames.remove(&id);
            }
        }
    }
    app.save();
    reload(&app);
    Ok(view(&app))
}

/// Hide a game from the library, or bring it back.
#[tauri::command]
fn set_hidden(id: String, hidden: bool, app: State<App>) -> Boot {
    {
        let mut s = app.saved.lock().unwrap();
        if hidden {
            s.hidden.insert(id.clone());
            s.favorites.remove(&id);
        } else {
            s.hidden.remove(&id);
        }
    }
    app.save();
    view(&app)
}

/// Covers from every source for the "Find the right cover" picker.
#[tauri::command]
async fn cover_choices(id: String, query: String, handle: AppHandle) -> Result<Vec<covers::CoverChoice>, String> {
    let app = handle.state::<App>();
    let system = {
        let lib = app.library.lock().unwrap();
        lib.as_ref().and_then(|l| find(l, &id)).map(|(s, _)| s.id).ok_or("Game not found.")?
    };
    let key = Credentials::load(&app.dir.join("credentials.txt")).steamgriddb_key;
    let media = app.media();
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    tauri::async_runtime::spawn_blocking(move || covers::cover_choices(system, &q, &media, key.as_deref())).await.map_err(|e| e.to_string())
}

/// Use one of the covers from the picker.
#[tauri::command]
async fn use_cover(id: String, url: String, handle: AppHandle) -> Result<Boot, String> {
    let app = handle.state::<App>();
    let media = app.media();
    let (m, i, u) = (media.clone(), id.clone(), url.clone());
    tauri::async_runtime::spawn_blocking(move || covers::use_cover_url(&m, &i, &u)).await.map_err(|e| e.to_string())??;
    {
        let saved = app.saved.lock().unwrap().clone();
        if let Some(lib) = app.library.lock().unwrap().as_mut() {
            reattach(lib, &media, &saved);
        }
    }
    Ok(view(&app))
}

/// Put a game in one of the user's lists, or take it out. The list is created if new.
#[tauri::command]
fn set_collection(name: String, id: String, member: bool, app: State<App>) -> Result<Boot, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("List names can be 1 to 40 characters.".into());
    }
    {
        let mut s = app.saved.lock().unwrap();
        let list = s.collections.entry(name).or_default();
        list.retain(|g| g != &id);
        if member {
            list.push(id);
        }
    }
    app.save();
    Ok(view(&app))
}

#[tauri::command]
fn delete_collection(name: String, app: State<App>) -> Boot {
    app.saved.lock().unwrap().collections.remove(&name);
    app.save();
    view(&app)
}

/// RetroAchievements progress for a game (None when not set up or the game has none).
#[tauri::command]
async fn achievements(id: String, handle: AppHandle) -> Result<Option<spine_core::retroachievements::Progress>, String> {
    let app = handle.state::<App>();
    let c = Credentials::load(&app.dir.join("credentials.txt"));
    let (Some(user), Some(key)) = (c.ra_user, c.ra_key) else { return Ok(None) };
    let (system, title) = {
        let lib = app.library.lock().unwrap();
        let (s, g) = lib.as_ref().and_then(|l| find(l, &id)).ok_or("Game not found.")?;
        (s.id, g.title.clone())
    };
    let media = app.media();
    tauri::async_runtime::spawn_blocking(move || spine_core::retroachievements::progress(system, &title, &user, &key, &media)).await.map_err(|e| e.to_string())?
}

/// Open a link in the browser (only RetroAchievements, TMDB's key page and emulator download pages).
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let ok = ["https://retroachievements.org/", "https://www.retroachievements.org/"].iter().any(|p| url.starts_with(p))
        || url == "https://www.themoviedb.org/settings/api" || launch::download_pages().iter().chain(launch::flathub_pages()).any(|(_, u)| *u == url);
    if !ok || url.contains(['"', '\'', ' ', '&', '|', '^', '<', '>']) {
        return Err("Spine only opens links it knows.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).creation_flags(0x0800_0000).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        spine_core::hostenv::command("xdg-open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn check_update(handle: AppHandle) -> Result<update::UpdateInfo, String> {
    let dir = handle.state::<App>().dir.clone();
    tauri::async_runtime::spawn_blocking(move || update::check(&dir)).await.map_err(|e| e.to_string())?
}

/// Download and install the newest Spine, then restart into it.
#[tauri::command]
async fn install_update(handle: AppHandle) -> Result<(), String> {
    let app = handle.state::<App>();
    if app.running.lock().unwrap().is_some() {
        return Err("Close your game first.".into());
    }
    let dir = app.dir.clone();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let new_exe = tauri::async_runtime::spawn_blocking(move || update::install(&dir, &exe)).await.map_err(|e| e.to_string())??;
    app.save();
    spine_core::hostenv::command(&new_exe).current_dir(&app.dir).spawn().map_err(|e| format!("Updated, but couldn't restart Spine ({e}). Start it again yourself."))?;
    // Leave right away so only the new Spine is open (everything is already saved).
    handle.exit(0);
    std::process::exit(0);
    #[allow(unreachable_code)]
    Ok(())
}

#[tauri::command]
fn app_version() -> &'static str {
    update::CURRENT
}

/// Open a folder Spine told the user to put a file in (created if needed).
#[tauri::command]
fn open_bios_folder(system: String, app: State<App>) -> Result<(), String> {
    let folder = {
        let lib = app.library.lock().unwrap();
        let s = lib.as_ref().and_then(|l| l.systems.iter().find(|s| s.id == system)).ok_or("Console not found.")?;
        let def = systems::system_by_id(s.id).unwrap();
        let plan = launch::plan(def, &s.games[0], &app.installed.lock().unwrap());
        spine_core::bios::check(s.id, &plan).map(|i| PathBuf::from(i.folder)).ok_or("Nothing is missing for this console.")?
    };
    let _ = fs::create_dir_all(&folder);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer").arg(&folder).creation_flags(0x0800_0000).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        spine_core::hostenv::command("xdg-open").arg(&folder).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_path(which: String, app: State<App>) -> Result<(), String> {
    let p = match which.as_str() {
        "credentials" => app.dir.join("credentials.txt"),
        "folder" => app.dir.clone(),
        _ => return Err("unknown".into()),
    };
    if which == "credentials" {
        Credentials::ensure_template(&p, "");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer").arg(&p).creation_flags(0x0800_0000).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        spine_core::hostenv::command("xdg-open").arg(&p).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find<'a>(lib: &'a Library, id: &str) -> Option<(&'a spine_core::SystemLibrary, &'a spine_core::Game)> {
    lib.systems.iter().find_map(|s| s.games.iter().find(|g| g.id == id).map(|g| (s, g)))
}

#[derive(Serialize)]
struct LaunchChoices {
    current: Option<String>,
    options: Vec<launch::LaunchOption>,
}

/// The emulators a game can use, for the "Change emulator" menu.
#[tauri::command]
fn launch_options(id: String, app: State<App>) -> Result<LaunchChoices, String> {
    let lib = app.library.lock().unwrap();
    let (sys, game) = lib.as_ref().and_then(|l| find(l, &id)).ok_or("Game not found.")?;
    let def = systems::system_by_id(sys.id).unwrap();
    Ok(LaunchChoices {
        current: app.saved.lock().unwrap().overrides.get(&id).cloned(),
        options: launch::options(def, game, &app.installed.lock().unwrap()),
    })
}

/// Choose an emulator for one game (None = use the console's default).
#[tauri::command]
fn set_launcher(id: String, key: Option<String>, app: State<App>) -> Boot {
    {
        let mut s = app.saved.lock().unwrap();
        match key {
            Some(k) => s.overrides.insert(id, k),
            None => s.overrides.remove(&id),
        };
    }
    app.save();
    view(&app)
}

/// Let the user pick their own cover image for a game.
#[tauri::command]
async fn pick_cover(id: String, handle: AppHandle) -> Result<Option<Boot>, String> {
    let file = handle
        .dialog()
        .file()
        .set_title("Choose a cover image")
        .add_filter("Images", covers::CUSTOM_EXTS)
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    let Some(src) = file else { return Ok(None) };
    let app = handle.state::<App>();
    let media = app.media();
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| covers::CUSTOM_EXTS.contains(&e.as_str())).ok_or("That file isn't a png, jpg, webp or gif image.")?;
    let base = covers::custom_base(&media, &id);
    for e in covers::CUSTOM_EXTS {
        let _ = fs::remove_file(base.with_extension(e));
    }
    if let Some(d) = base.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    fs::copy(&src, base.with_extension(&ext)).map_err(|e| format!("Couldn't copy the image: {e}"))?;
    {
        let saved = app.saved.lock().unwrap().clone();
        if let Some(lib) = app.library.lock().unwrap().as_mut() {
            reattach(lib, &media, &saved);
        }
    }
    Ok(Some(view(&app)))
}

/// Let the user pick their own video for a game.
#[tauri::command]
async fn pick_video(id: String, handle: AppHandle) -> Result<Option<Boot>, String> {
    let file = handle
        .dialog()
        .file()
        .set_title("Choose a video")
        .add_filter("Videos", covers::VIDEO_EXTS)
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    let Some(src) = file else { return Ok(None) };
    let app = handle.state::<App>();
    let media = app.media();
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| covers::VIDEO_EXTS.contains(&e.as_str())).ok_or("That file isn't an mp4 or webm video.")?;
    let base = covers::custom_base(&media, &id);
    for e in covers::VIDEO_EXTS {
        let _ = fs::remove_file(base.with_extension(e));
    }
    if let Some(d) = base.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    fs::copy(&src, base.with_extension(&ext)).map_err(|e| format!("Couldn't copy the video: {e}"))?;
    {
        let saved = app.saved.lock().unwrap().clone();
        if let Some(lib) = app.library.lock().unwrap().as_mut() {
            reattach(lib, &media, &saved);
        }
    }
    Ok(Some(view(&app)))
}

#[tauri::command]
fn reset_video(id: String, app: State<App>) -> Boot {
    let media = app.media();
    let base = covers::custom_base(&media, &id);
    for e in covers::VIDEO_EXTS {
        let _ = fs::remove_file(base.with_extension(e));
    }
    {
        let saved = app.saved.lock().unwrap().clone();
        if let Some(lib) = app.library.lock().unwrap().as_mut() {
            reattach(lib, &media, &saved);
        }
    }
    view(&app)
}

/// Go back to the downloaded cover.
#[tauri::command]
fn reset_cover(id: String, app: State<App>) -> Boot {
    let media = app.media();
    let base = covers::custom_base(&media, &id);
    for e in covers::CUSTOM_EXTS {
        let _ = fs::remove_file(base.with_extension(e));
    }
    {
        let saved = app.saved.lock().unwrap().clone();
        if let Some(lib) = app.library.lock().unwrap().as_mut() {
            reattach(lib, &media, &saved);
        }
    }
    view(&app)
}

/// Scraper accounts for Settings. Secrets are never sent back to the
/// interface, only whether they're set (and the last 4 characters of the key).
#[derive(Serialize)]
struct Accounts {
    steamgriddb_key: Option<String>,
    screenscraper_user: Option<String>,
    screenscraper_password: bool,
    screenscraper_devid: Option<String>,
    screenscraper_devpassword: bool,
    /// This copy of Spine has the developer key built in (official releases).
    screenscraper_builtin: bool,
    retroachievements_user: Option<String>,
    retroachievements_key: Option<String>,
    tmdb_key: Option<String>,
}

fn hint(k: &Option<String>) -> Option<String> {
    k.as_ref().map(|k| {
        let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        format!("••••{tail}")
    })
}

#[tauri::command]
fn get_accounts(app: State<App>) -> Accounts {
    let c = Credentials::load(&app.dir.join("credentials.txt"));
    Accounts {
        steamgriddb_key: hint(&c.steamgriddb_key),
        screenscraper_user: c.ss_user.clone(),
        screenscraper_password: c.ss_password.is_some(),
        screenscraper_devid: c.ss_devid.clone(),
        screenscraper_devpassword: c.ss_devpassword.is_some(),
        screenscraper_builtin: spine_core::config::builtin_dev_key().is_some(),
        retroachievements_user: c.ra_user.clone(),
        retroachievements_key: hint(&c.ra_key),
        tmdb_key: hint(&c.tmdb_key),
    }
}

/// Update scraper accounts. A missing field is left alone; an empty one is cleared.
#[derive(Deserialize)]
struct AccountChanges {
    steamgriddb_key: Option<String>,
    screenscraper_user: Option<String>,
    screenscraper_password: Option<String>,
    screenscraper_devid: Option<String>,
    screenscraper_devpassword: Option<String>,
    retroachievements_user: Option<String>,
    retroachievements_key: Option<String>,
    #[serde(default)]
    tmdb_key: Option<String>,
}

#[tauri::command]
fn set_accounts(changes: AccountChanges, app: State<App>) -> Result<Accounts, String> {
    let path = app.dir.join("credentials.txt");
    let mut c = Credentials::load(&path);
    let apply = |field: &mut Option<String>, v: Option<String>| {
        if let Some(v) = v {
            let v = v.trim().to_string();
            *field = (!v.is_empty()).then_some(v);
        }
    };
    apply(&mut c.steamgriddb_key, changes.steamgriddb_key);
    apply(&mut c.ss_user, changes.screenscraper_user);
    apply(&mut c.ss_password, changes.screenscraper_password);
    apply(&mut c.ss_devid, changes.screenscraper_devid);
    apply(&mut c.ss_devpassword, changes.screenscraper_devpassword);
    apply(&mut c.ra_user, changes.retroachievements_user);
    apply(&mut c.ra_key, changes.retroachievements_key);
    apply(&mut c.tmdb_key, changes.tmdb_key);
    c.save(&path).map_err(|e| format!("Couldn't save: {e}"))?;
    Ok(get_accounts(app))
}

#[tauri::command]
fn quit(handle: AppHandle) {
    handle.exit(0);
}

#[tauri::command]
fn set_fullscreen(on: bool, handle: AppHandle) {
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.set_fullscreen(on);
    }
}

// ---------------------------------------------------------------------------
// Serving covers to the interface: spine://localhost/covers/gba/x.png
// ---------------------------------------------------------------------------

/// Serve covers and videos to the interface.
///   spine://localhost/covers/gba/x.png   a file under the media folder
///   spine://localhost/video/<game id>    that game's video, wherever it lives
/// Videos honour HTTP Range requests so the player can stream and loop them.
fn serve_media(media: &Path, uri_path: &str, range: Option<&str>, video_of: &dyn Fn(&str) -> Option<PathBuf>) -> tauri::http::Response<Vec<u8>> {
    let rel = percent_decode(uri_path.trim_start_matches('/'));
    let resp = tauri::http::Response::builder().header("Access-Control-Allow-Origin", "*");
    let not_found = || tauri::http::Response::builder().status(404).body(Vec::new()).unwrap();
    let path = if let Some(id) = rel.strip_prefix("video/") {
        match video_of(id) {
            Some(p) => p,
            None => return not_found(),
        }
    } else if let Some(id) = rel.strip_prefix("video-audio/") {
        match video_of(id) {
            Some(p) => p.with_extension("audio.m4a"),
            None => return not_found(),
        }
    } else {
        if rel.split(['/', '\\']).any(|p| p == ".." || p.contains(':')) {
            return not_found();
        }
        media.join(&rel)
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "m4a" => "audio/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    };
    let Ok(meta) = fs::metadata(&path) else { return not_found() };
    let len = meta.len();
    // Range: bytes=start-end
    if let Some((a, b)) = range.and_then(|r| r.strip_prefix("bytes=")).and_then(|r| r.split(',').next()).and_then(|r| r.split_once('-')) {
        let (start, end) = match (a.trim().parse::<u64>().ok(), b.trim().parse::<u64>().ok()) {
            (Some(s), Some(e)) => (s, e.min(len.saturating_sub(1))),
            (Some(s), None) => (s, (s + 4 * 1024 * 1024).min(len).saturating_sub(1)),
            (None, Some(n)) => (len.saturating_sub(n), len.saturating_sub(1)),
            _ => (0, len.saturating_sub(1)),
        };
        if start >= len || start > end {
            return tauri::http::Response::builder().status(416).header("Content-Range", format!("bytes */{len}")).body(Vec::new()).unwrap();
        }
        use std::io::{Read, Seek, SeekFrom};
        let mut f = match fs::File::open(&path) { Ok(f) => f, Err(_) => return not_found() };
        let mut buf = vec![0u8; (end - start + 1) as usize];
        if f.seek(SeekFrom::Start(start)).is_err() || f.read_exact(&mut buf).is_err() {
            return not_found();
        }
        return resp
            .status(206)
            .header("Content-Type", mime)
            .header("Accept-Ranges", "bytes")
            .header("Content-Range", format!("bytes {start}-{end}/{len}"))
            .header("Content-Length", buf.len().to_string())
            .body(buf)
            .unwrap();
    }
    match fs::read(&path) {
        Ok(bytes) => resp.header("Content-Type", mime).header("Accept-Ranges", "bytes").header("Cache-Control", "max-age=31536000").body(bytes).unwrap(),
        Err(_) => not_found(),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Covers, details, and the user's own names, on top of a scanned library.
fn reattach(lib: &mut Library, media: &Path, saved: &Saved) {
    covers::attach_existing(lib, media);
    if saved.renames.is_empty() {
        return;
    }
    for s in &mut lib.systems {
        let mut changed = false;
        for g in &mut s.games {
            if let Some(t) = saved.renames.get(&g.id) {
                g.title = t.clone();
                g.sort_key = spine_core::title::sort_key(t);
                g.letter = spine_core::title::index_letter(&g.sort_key);
                changed = true;
            }
        }
        if changed {
            s.games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
        }
    }
}

/// Scan every ROM folder (plus installed PC store games) into one library.
fn build_library(dir: &Path, saved: &Saved) -> (Library, Vec<PathBuf>, Installed) {
    let roots = saved.roots();
    let (mut lib, mut offline) = spine_core::scan::scan_many(&roots, &dir.join("media").join("cache"));
    add_store_games(&mut lib, saved);
    let movie_roots = saved.movie_roots();
    if !movie_roots.is_empty() {
        let (movies, movies_offline) = spine_core::movies::scan_many(&movie_roots, &dir.join("media").join("cache"));
        spine_core::scan::merge_library(&mut lib, movies);
        offline.extend(movies_offline);
    }
    let tv_roots = saved.tv_roots();
    if !tv_roots.is_empty() {
        let (shows, tv_offline) = spine_core::tv::scan_many(&tv_roots, &dir.join("media").join("cache"));
        spine_core::scan::merge_library(&mut lib, shows);
        offline.extend(tv_offline);
    }
    reattach(&mut lib, &dir.join("media"), saved);
    let mut search: Vec<PathBuf> = vec![];
    for r in roots.iter().chain(movie_roots.iter()).chain(tv_roots.iter()) {
        for p in launch::default_search_paths(r) {
            if !search.contains(&p) {
                search.push(p);
            }
        }
    }
    (lib, offline, Installed::detect(&search))
}

/// Note when games first appear. The very first scan counts as "always had them".
fn stamp_first_seen(lib: &Library, saved: &mut Saved) -> bool {
    let first_run = saved.first_seen.is_empty();
    let t = if first_run { 0 } else { now() };
    let mut changed = false;
    for s in &lib.systems {
        for g in &s.games {
            if !saved.first_seen.contains_key(&g.id) {
                saved.first_seen.insert(g.id.clone(), t);
                changed = true;
            }
        }
    }
    changed
}

/// Rebuild the library from disk and store it.
fn reload(app: &App) {
    let saved = app.saved.lock().unwrap().clone();
    let (lib, offline, installed) = build_library(&app.dir, &saved);
    if stamp_first_seen(&lib, &mut app.saved.lock().unwrap()) {
        app.save();
    }
    *app.installed.lock().unwrap() = installed;
    *app.offline.lock().unwrap() = offline;
    *app.library.lock().unwrap() = Some(lib);
}

/// A cheap fingerprint of the ROM folders: whether each is there, and when it and
/// the folders inside it last changed. Adding or removing a game changes it.
fn folder_signature(saved: &Saved) -> Vec<(PathBuf, bool, Vec<u64>)> {
    let mt = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let mut dirs: Vec<PathBuf> = saved.roots();
    dirs.extend(saved.movie_roots());
    dirs.extend(saved.tv_roots());
    if saved.settings.get("storeGames").and_then(|v| v.as_bool()) != Some(false) {
        dirs.push(spine_core::stores::epic_manifest_dir());
        if let Some(st) = spine_core::stores::steam_root() {
            dirs.push(st.join("steamapps"));
        }
    }
    dirs.into_iter()
        .map(|root| {
            let online = root.is_dir();
            let mut v = vec![mt(&root)];
            if let Ok(rd) = fs::read_dir(&root) {
                let mut subs: Vec<(String, u64)> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| (e.file_name().to_string_lossy().to_string(), mt(&e.path()))).collect();
                subs.sort();
                v.extend(subs.into_iter().map(|(_, t)| t));
            }
            (root, online, v)
        })
        .collect()
}

/// Every 15 seconds: if a ROM folder changed or a drive came or went, rescan and
/// tell the interface; new games get their covers fetched.
fn watch_folders(handle: AppHandle) {
    std::thread::spawn(move || {
        let mut last = {
            let app = handle.state::<App>();
            let s = app.saved.lock().unwrap().clone();
            folder_signature(&s)
        };
        loop {
            std::thread::sleep(std::time::Duration::from_secs(15));
            let app = handle.state::<App>();
            let saved = app.saved.lock().unwrap().clone();
            if saved.empty() || saved.settings.get("autoScan").and_then(|v| v.as_bool()) == Some(false) {
                continue;
            }
            let sig = folder_signature(&saved);
            if sig == last {
                continue;
            }
            last = sig;
            let before: BTreeSet<String> = app.library.lock().unwrap().as_ref().map(|l| l.systems.iter().flat_map(|s| s.games.iter().map(|g| g.id.clone())).collect()).unwrap_or_default();
            let offline_before = app.offline.lock().unwrap().clone();
            reload(&app);
            let after: BTreeSet<String> = app.library.lock().unwrap().as_ref().map(|l| l.systems.iter().flat_map(|s| s.games.iter().map(|g| g.id.clone())).collect()).unwrap_or_default();
            let offline_after = app.offline.lock().unwrap().clone();
            let added = after.difference(&before).count();
            let removed = before.difference(&after).count();
            let reconnected: Vec<String> = offline_before.iter().filter(|r| !offline_after.contains(r)).map(|r| r.display().to_string()).collect();
            let unplugged: Vec<String> = offline_after.iter().filter(|r| !offline_before.contains(r)).map(|r| r.display().to_string()).collect();
            let _ = handle.emit("library-changed", serde_json::json!({ "added": added, "removed": removed, "reconnected": reconnected, "unplugged": unplugged, "boot": view(&app) }));
            if added > 0 {
                fetch_media(handle.clone());
            }
        }
    });
}

/// Steam / Epic / GOG games already installed on this PC (on unless turned off in Settings).
fn add_store_games(lib: &mut Library, saved: &Saved) {
    if saved.settings.get("storeGames").and_then(|v| v.as_bool()) == Some(false) {
        return;
    }
    let n = spine_core::stores::merge(lib, spine_core::stores::installed());
    if n > 0 {
        lib.notes.push(format!("Added {n} installed game(s) from Steam, Epic Games and GOG."));
    }
}

fn main() {
    let dir = app_dir();
    if let Ok(exe) = std::env::current_exe() {
        update::cleanup(&exe);
    }
    #[cfg(target_os = "linux")]
    linux::ensure_menu_entry();
    let saved: Saved = fs::read_to_string(dir.join("spine-state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let media = dir.join("media");
    let mut saved = saved;
    // Scanning is fast (under a second for ~700 games), so just do it at start.
    let (library, offline, installed) = if saved.empty() {
        (None, vec![], Installed::default())
    } else {
        let (l, o, i) = build_library(&dir, &saved);
        stamp_first_seen(&l, &mut saved);
        (Some(l), o, i)
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App {
            dir,
            saved: Mutex::new(saved),
            library: Mutex::new(library),
            installed: Mutex::new(installed),
            running: Mutex::new(None),
            fetching: Mutex::new(false),
            offline: Mutex::new(offline),
        })
        .register_uri_scheme_protocol("spine", move |ctx, req| {
            let range = req.headers().get("range").and_then(|v| v.to_str().ok()).map(String::from);
            let handle = ctx.app_handle().clone();
            let video_of = move |id: &str| -> Option<PathBuf> {
                let app = handle.state::<App>();
                let lib = app.library.lock().unwrap();
                lib.as_ref().and_then(|l| find(l, id)).and_then(|(_, g)| g.video.clone())
            };
            serve_media(&media, req.uri().path(), range.as_deref(), &video_of)
        })
        .setup(|app| {
            watch_folders(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![check_update, install_update, app_version, achievements, open_url, set_collection, delete_collection, open_bios_folder, rename_game, set_hidden, cover_choices, use_cover, boot, pick_folder, scan, add_root, remove_root, fetch_media, toggle_favorite, set_setting, play, open_path, quit, set_fullscreen, launch_options, set_launcher, pick_cover, reset_cover, pick_video, reset_video, get_accounts, set_accounts, add_movie_root, remove_movie_root, add_tv_root, remove_tv_root, set_arc_season, set_special_query])
        .run(tauri::generate_context!())
        .expect("error while running Spine");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_refuses_escapes() {
        assert_eq!(percent_decode("covers/gba/pok%C3%A9mon.png"), "covers/gba/pokémon.png");
        let dir = std::env::temp_dir();
        let none = |_: &str| None;
        assert_eq!(serve_media(&dir, "/../etc/passwd", None, &none).status(), 404);
        assert_eq!(serve_media(&dir, "/C:%5Cwindows", None, &none).status(), 404);
        // Range requests
        let d = std::env::temp_dir().join("spine-range-test");
        let _ = fs::create_dir_all(&d);
        fs::write(d.join("v.mp4"), b"0123456789").unwrap();
        let r = serve_media(&d, "/v.mp4", Some("bytes=2-5"), &none);
        assert_eq!(r.status(), 206);
        assert_eq!(r.body(), b"2345");
        assert_eq!(r.headers()["content-range"], "bytes 2-5/10");
        let r = serve_media(&d, "/v.mp4", Some("bytes=8-"), &none);
        assert_eq!(r.body(), b"89");
        let r = serve_media(&d, "/video/gba/x", None, &|_| Some(d.join("v.mp4")));
        assert_eq!(r.headers()["content-type"], "video/mp4");
    }
}
