//! The extra art on game pages: a wide "hero" banner, the game's logo, and screenshots.
//!
//!   * PC games: Steam's own library banner, logo and store screenshots (no account).
//!   * Everything: SteamGridDB banners and logos (with the free key).
//!   * Retro consoles: libretro's title screens and in-game snaps, matched like covers.
//!
//! Files live in media/heroes, media/logos and media/shots/<game>/. When a source
//! has nothing, a small ".none" marker stops Spine asking again for a month.

use crate::covers::{self, curl, get_bytes, is_image, libretro_repos, pick_sgdb_game, save, steam_app_id, steam_search, ArtEntry, ArtIndex, PATH_SEG, SGDB};
use crate::scan::{Game, Library};
use percent_encoding::utf8_percent_encode;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const IMG_EXTS: &[&str] = &["jpg", "png", "webp"];

fn base(media: &Path, kind: &str, game_id: &str) -> PathBuf {
    let mut p = media.join(kind);
    for part in game_id.split('/') {
        p.push(part);
    }
    p
}

fn find_img(base: &Path) -> Option<PathBuf> {
    IMG_EXTS.iter().map(|e| base.with_extension(e)).find(|p| p.is_file())
}

pub fn existing_hero(media: &Path, game_id: &str) -> Option<PathBuf> {
    find_img(&base(media, "heroes", game_id))
}
pub fn existing_logo(media: &Path, game_id: &str) -> Option<PathBuf> {
    find_img(&base(media, "logos", game_id))
}
/// Screenshots in order (title screen first for retro games).
pub fn existing_shots(media: &Path, game_id: &str) -> Vec<PathBuf> {
    let dir = base(media, "shots", game_id);
    let mut v: Vec<PathBuf> = fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| IMG_EXTS.contains(&x.to_string_lossy().to_lowercase().as_str()))).collect()).unwrap_or_default();
    v.sort();
    v
}

/// "Already looked, nothing there" markers, good for 30 days.
fn checked_recently(marker: &Path) -> bool {
    fs::metadata(marker).ok().and_then(|m| m.modified().ok()).and_then(|t| SystemTime::now().duration_since(t).ok()).map_or(false, |a| a < Duration::from_secs(30 * 24 * 3600))
}
fn mark(marker: &Path) {
    let _ = fs::create_dir_all(marker.parent().unwrap_or(Path::new(".")));
    let _ = fs::write(marker, b"");
}

/// Download an image to `base` + its real extension.
fn grab(url: &str, base: &Path, headers: &[String]) -> Result<PathBuf, String> {
    let b = if headers.is_empty() { get_bytes(&covers::agent(), url)? } else { curl(url, true, headers)? };
    let ext = is_image(&b).ok_or("not an image")?;
    let dest = base.with_extension(ext);
    save(&dest, &b)?;
    Ok(dest)
}

fn steam_urls(app: u32, file: &str) -> [String; 2] {
    [
        format!("https://shared.steamstatic.com/store_item_assets/steam/apps/{app}/{file}"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/{file}"),
    ]
}

/// Screenshot links from Steam's store answer (full size, first `max`).
pub fn parse_steam_screenshots(json: &[u8], app: u32, max: usize) -> Vec<String> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else { return vec![] };
    v[app.to_string()]["data"]["screenshots"].as_array().into_iter().flatten().filter_map(|s| s["path_full"].as_str().map(String::from)).take(max).collect()
}

/// First image link in a SteamGridDB list answer.
pub fn pick_sgdb_url(json: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    v["data"].as_array()?.iter().find_map(|g| g["url"].as_str().map(String::from))
}

struct Job {
    game_id: String,
    title: String,
    names: Vec<String>,
    /// Steam app (known or to look up) for PC games.
    pc: bool,
    steam: Option<u32>,
    art: Option<ArtEntry>,
}

/// What a game still needs.
fn wants(media: &Path, id: &str) -> (bool, bool, bool) {
    let hero = existing_hero(media, id).is_none() && !checked_recently(&base(media, "heroes", id).with_extension("none"));
    let logo = existing_logo(media, id).is_none() && !checked_recently(&base(media, "logos", id).with_extension("none"));
    let shots = existing_shots(media, id).is_empty() && !checked_recently(&base(media, "shots", id).join(".none"));
    (hero, logo, shots)
}

fn names_for(g: &Game) -> Vec<String> {
    let mut names = vec![g.title.clone()];
    if g.format == "lnk" {
        if let Some(f) = covers::lnk_folder_name(&g.path) {
            names.insert(0, f);
        }
    }
    names
}

/// Fetch banners, logos and screenshots for every game that's missing them.
/// Returns how many games got something new.
pub fn fetch_extras(lib: &Library, media: &Path, sgdb_key: Option<&str>, progress: &(dyn Fn(usize, usize, &str) + Sync)) -> usize {
    let agent = covers::agent();
    let mut jobs: Vec<Job> = Vec::new();
    for s in &lib.systems {
        let todo: Vec<&Game> = s.games.iter().filter(|g| {
            let (h, l, sh) = wants(media, &g.id);
            h || l || sh
        }).collect();
        if todo.is_empty() {
            continue;
        }
        let index = if s.id == "pc" {
            None
        } else {
            let lists: Vec<_> = libretro_repos(s.id).iter().filter_map(|r| covers::fetch_list(&agent, r, &media.join("lists")).ok().map(|l| (*r, l))).collect();
            (!lists.is_empty()).then(|| ArtIndex::new(lists))
        };
        for g in todo {
            jobs.push(Job {
                game_id: g.id.clone(),
                title: g.title.clone(),
                names: names_for(g),
                pc: s.id == "pc",
                steam: if s.id == "pc" { steam_app_id(&g.path) } else { None },
                art: index.as_ref().and_then(|i| i.find(g).cloned()),
            });
        }
    }
    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let done = Arc::new(Mutex::new(0usize));
    let got = Arc::new(Mutex::new(0usize));
    let key = sgdb_key.map(String::from);
    std::thread::scope(|scope| {
        for _ in 0..6 {
            let (queue, done, got, key) = (queue.clone(), done.clone(), got.clone(), key.clone());
            scope.spawn(move || loop {
                let Some(job) = queue.lock().unwrap().pop() else { break };
                if one_game(&job, media, key.as_deref()) {
                    *got.lock().unwrap() += 1;
                }
                let n = {
                    let mut d = done.lock().unwrap();
                    *d += 1;
                    *d
                };
                progress(n, total, &job.title);
            });
        }
    });
    let n = *got.lock().unwrap();
    n
}

fn one_game(job: &Job, media: &Path, sgdb_key: Option<&str>) -> bool {
    let agent = covers::agent();
    let (want_hero, want_logo, want_shots) = wants(media, &job.game_id);
    let hero_base = base(media, "heroes", &job.game_id);
    let logo_base = base(media, "logos", &job.game_id);
    let shots_dir = base(media, "shots", &job.game_id);
    let (mut hero_ok, mut logo_ok, mut shots_ok) = (!want_hero, !want_logo, !want_shots);

    // PC: Steam has all three, no account needed.
    if job.pc {
        let app = job.steam.or_else(|| steam_search(&agent, &job.names).ok());
        if let Some(app) = app {
            if !hero_ok {
                hero_ok = steam_urls(app, "library_hero.jpg").iter().any(|u| grab(u, &hero_base, &[]).is_ok());
            }
            if !logo_ok {
                logo_ok = steam_urls(app, "logo.png").iter().any(|u| grab(u, &logo_base, &[]).is_ok());
            }
            if !shots_ok {
                let url = format!("https://store.steampowered.com/api/appdetails?appids={app}&filters=screenshots");
                if let Ok(json) = get_bytes(&agent, &url) {
                    for (i, u) in parse_steam_screenshots(&json, app, 6).iter().enumerate() {
                        if grab(u, &shots_dir.join(format!("{:02}", i + 1)), &[]).is_ok() {
                            shots_ok = true;
                        }
                    }
                }
            }
        }
    }

    // Retro: libretro title screen + in-game snap, same names as the box art.
    if let (false, Some(e)) = (shots_ok, &job.art) {
        for (i, folder) in ["Named_Titles", "Named_Snaps"].iter().enumerate() {
            let url = format!("https://raw.githubusercontent.com/libretro-thumbnails/{}/master/{folder}/{}", e.repo, utf8_percent_encode(&format!("{}.png", e.name), PATH_SEG));
            if grab(&url, &shots_dir.join(format!("{:02}", i + 1)), &[]).is_ok() {
                shots_ok = true;
            }
        }
    }

    // Anything still missing: SteamGridDB banners and logos.
    if let (Some(key), false) = (sgdb_key, hero_ok && logo_ok) {
        let auth = vec![format!("Authorization: Bearer {key}")];
        let mut id = None;
        for n in &job.names {
            let url = format!("{SGDB}/search/autocomplete/{}", utf8_percent_encode(n, PATH_SEG));
            if let Ok(body) = curl(&url, true, &auth) {
                if let Some(g) = pick_sgdb_game(&body, &job.names) {
                    id = Some(g);
                    break;
                }
            }
        }
        if let Some(id) = id {
            if !hero_ok {
                let url = format!("{SGDB}/heroes/game/{id}?types=static&nsfw=false&humor=false");
                if let Some(u) = curl(&url, true, &auth).ok().and_then(|b| pick_sgdb_url(&b)) {
                    hero_ok = grab(&u, &hero_base, &[]).is_ok();
                }
            }
            if !logo_ok {
                let url = format!("{SGDB}/logos/game/{id}?types=static&nsfw=false&humor=false");
                if let Some(u) = curl(&url, true, &auth).ok().and_then(|b| pick_sgdb_url(&b)) {
                    logo_ok = grab(&u, &logo_base, &[]).is_ok();
                }
            }
        }
    }

    let before = (!want_hero, !want_logo, !want_shots);
    if !hero_ok {
        mark(&hero_base.with_extension("none"));
    }
    if !logo_ok {
        mark(&logo_base.with_extension("none"));
    }
    if !shots_ok {
        mark(&shots_dir.join(".none"));
    }
    (hero_ok && !before.0) || (logo_ok && !before.1) || (shots_ok && !before.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sources() {
        let j = br#"{"1942280":{"success":true,"data":{"screenshots":[{"id":0,"path_thumbnail":"t0","path_full":"f0"},{"id":1,"path_full":"f1"}]}}}"#;
        assert_eq!(parse_steam_screenshots(j, 1942280, 6), vec!["f0", "f1"]);
        assert_eq!(parse_steam_screenshots(j, 1942280, 1), vec!["f0"]);
        assert!(parse_steam_screenshots(b"{}", 1, 6).is_empty());
        assert_eq!(pick_sgdb_url(br#"{"success":true,"data":[{"id":1,"url":"https://cdn2.steamgriddb.com/hero/a.png"}]}"#).as_deref(), Some("https://cdn2.steamgriddb.com/hero/a.png"));
        assert_eq!(pick_sgdb_url(br#"{"success":true,"data":[]}"#), None);
    }

    #[test]
    fn markers_and_files() {
        let m = std::env::temp_dir().join(format!("spine-extras-{}", std::process::id()));
        let _ = fs::remove_dir_all(&m);
        assert_eq!(wants(&m, "gba/golden-sun"), (true, true, true));
        mark(&base(&m, "heroes", "gba/golden-sun").with_extension("none"));
        fs::create_dir_all(base(&m, "shots", "gba/golden-sun")).unwrap();
        fs::write(base(&m, "shots", "gba/golden-sun").join("02.png"), b"x").unwrap();
        fs::write(base(&m, "shots", "gba/golden-sun").join("01.png"), b"x").unwrap();
        assert_eq!(wants(&m, "gba/golden-sun"), (false, true, false));
        let shots = existing_shots(&m, "gba/golden-sun");
        assert!(shots[0].ends_with("01.png") && shots.len() == 2);
        let _ = fs::remove_dir_all(&m);
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Real downloads (run with: cargo test --lib live -- --ignored).
    #[test]
    #[ignore]
    fn steam_and_libretro() {
        let m = std::env::temp_dir().join("spine-extras-live");
        let _ = fs::remove_dir_all(&m);
        let pc = Job { game_id: "pc/brotato".into(), title: "Brotato".into(), names: vec!["Brotato".into()], pc: true, steam: Some(1942280), art: None };
        // Steam isn't reachable from every network; report rather than fail.
        let _ = one_game(&pc, &m, None);
        println!("pc: hero={:?} logo={:?} shots={}", existing_hero(&m, "pc/brotato"), existing_logo(&m, "pc/brotato"), existing_shots(&m, "pc/brotato").len());
        let names: Vec<String> = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/art_lists/Nintendo_-_Game_Boy_Advance.txt")).unwrap().lines().map(String::from).collect();
        let idx = ArtIndex::new(vec![("Nintendo_-_Game_Boy_Advance", names)]);
        let e = idx.entries_named("Golden Sun (USA, Europe)").expect("entry");
        let gba = Job { game_id: "gba/golden-sun".into(), title: "Golden Sun".into(), names: vec!["Golden Sun".into()], pc: false, steam: None, art: Some(e) };
        assert!(one_game(&gba, &m, None));
        println!("gba shots: {:?}", existing_shots(&m, "gba/golden-sun"));
    }
}
