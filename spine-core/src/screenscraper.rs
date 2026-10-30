//! ScreenScraper: proper names, descriptions, release years, genres and box
//! art, looked up by file name and size. Needs a user account *and* a
//! developer key for Spine (see credentials.txt).
//!
//! Results are cached per game in `<media>/info/<system>/<slug>.json`, so each
//! game is only ever looked up once (games ScreenScraper doesn't know are
//! retried after 30 days). Free accounts get one request at a time, so this
//! runs one game after another with a short pause in between.

use crate::config::Credentials;
use crate::covers;
use crate::scan::{Game, Library};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str = "https://api.screenscraper.fr/api2";
const SOFTNAME: &str = "Spine";

/// ScreenScraper's id for each Spine system.
pub fn system_id(system: &str) -> Option<u32> {
    Some(match system {
        "nes" => 3, "fds" => 106, "snes" => 4, "n64" => 14, "gb" => 9, "gbc" => 10, "gba" => 12,
        "nds" => 15, "n3ds" => 17, "virtualboy" => 11, "gc" => 13, "wii" => 16, "wiiu" => 18, "switch" => 225,
        "mastersystem" => 2, "genesis" => 1, "gamegear" => 21, "segacd" => 20, "sega32x" => 19, "saturn" => 22, "dreamcast" => 23,
        "psx" => 57, "ps2" => 58, "ps3" => 59, "psp" => 61, "psvita" => 62,
        "pcengine" => 31, "pcenginecd" => 114, "neogeo" => 142, "ngpc" => 82, "arcade" => 75,
        "atari2600" => 26, "atarilynx" => 28, "wonderswan" => 45, "dos" => 135, "xbox" => 32,
        _ => return None,
    })
}

/// What Spine keeps about a game.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GameInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    /// Box art URL as returned (contains no user password; ScreenScraper media URLs carry dev id only).
    #[serde(default, skip_serializing)]
    pub box_url: Option<String>,
    #[serde(default, skip_serializing)]
    pub box_format: Option<String>,
    /// Gameplay video URL (not saved: ScreenScraper media links carry the developer key).
    #[serde(default, skip_serializing)]
    pub video_url: Option<String>,
    /// ScreenScraper didn't know this game; unix time of the check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_found_at: Option<u64>,
    /// Which version of the lookup produced a "not found" (older ones get retried).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup: Option<u32>,
}

const REGIONS: &[&str] = &["us", "wor", "ss", "eu", "uk", "jp"];
const LANGS: &[&str] = &["en", "fr", "es", "de"];

fn pick_region(list: &Value) -> Option<String> {
    let arr = list.as_array()?;
    for r in REGIONS {
        if let Some(t) = arr.iter().find(|x| x["region"].as_str() == Some(r)).and_then(|x| x["text"].as_str()) {
            return Some(t.to_string());
        }
    }
    arr.first().and_then(|x| x["text"].as_str()).map(String::from)
}

fn pick_lang(list: &Value) -> Option<String> {
    let arr = list.as_array()?;
    for l in LANGS {
        if let Some(t) = arr.iter().find(|x| x["langue"].as_str() == Some(l)).and_then(|x| x["text"].as_str()) {
            return Some(t.to_string());
        }
    }
    None
}

/// Turn a jeuInfos.php JSON response into GameInfo.
pub fn parse_game(json: &Value) -> Option<GameInfo> {
    let jeu = &json["response"]["jeu"];
    if !jeu.is_object() {
        return None;
    }
    let name = pick_region(&jeu["noms"]).map(|n| n.replace(" : ", ": ").trim().to_string());
    if name.as_deref().map_or(false, |n| n.to_uppercase().starts_with("ZZZ(NOTGAME)")) {
        return None;
    }
    let year = pick_region(&jeu["dates"]).map(|d| d.chars().take(4).collect());
    let genre = jeu["genres"].as_array().and_then(|g| g.first()).and_then(|g| pick_lang(&g["noms"]));
    let players = jeu["joueurs"]["text"].as_str().map(String::from);
    let developer = jeu["developpeur"]["text"].as_str().map(String::from);
    let description = pick_lang(&jeu["synopsis"]).map(|d| d.trim().to_string());

    let mut box_url = None;
    let mut box_format = None;
    if let Some(medias) = jeu["medias"].as_array() {
        let boxes: Vec<&Value> = medias.iter().filter(|m| m["type"].as_str() == Some("box-2D")).collect();
        let best = REGIONS.iter().find_map(|r| boxes.iter().find(|m| m["region"].as_str() == Some(r))).or(boxes.first());
        if let Some(m) = best {
            box_url = m["url"].as_str().map(String::from);
            box_format = m["format"].as_str().map(String::from);
        }
    }
    // Gameplay video: the "normalized" one is smaller and plays everywhere.
    let mut video_url = None;
    if let Some(medias) = jeu["medias"].as_array() {
        for kind in ["video-normalized", "video"] {
            if let Some(m) = medias.iter().find(|m| m["type"].as_str() == Some(kind)) {
                video_url = m["url"].as_str().map(String::from);
                break;
            }
        }
    }
    Some(GameInfo { name, description, year, genre, players, developer, box_url, box_format, video_url, not_found_at: None, lookup: None })
}

pub fn info_path(media: &Path, game_id: &str) -> PathBuf {
    let mut p = media.join("info");
    for part in game_id.split('/') {
        p.push(part);
    }
    p.with_extension("json")
}

pub fn load_info(media: &Path, game_id: &str) -> Option<GameInfo> {
    serde_json::from_str(&fs::read_to_string(info_path(media, game_id)).ok()?).ok()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Bumped when lookups get smarter, so earlier "not found" answers are tried again.
/// 2: ranking numbers ("02. ") removed from file names, CRC sent, search by name as a fallback.
pub const LOOKUP_VERSION: u32 = 2;

fn needs_lookup(media: &Path, game: &Game) -> bool {
    match load_info(media, &game.id) {
        None => true,
        Some(i) => match i.not_found_at {
            Some(t) => i.lookup.unwrap_or(1) < LOOKUP_VERSION || now().saturating_sub(t) > 30 * 24 * 3600,
            None => false,
        },
    }
}

/// CRC32 of a file (the checksum ScreenScraper indexes ROMs by). Only for cartridge-sized
/// files: disc images are matched by name and size instead.
pub fn file_crc32(path: &Path) -> Option<String> {
    use std::io::Read;
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return None;
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "zip" | "7z" | "rar") {
        return None; // the archive's checksum isn't the game's
    }
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut f = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 1 << 16];
    let mut crc = 0xFFFF_FFFFu32;
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
    }
    Some(format!("{:08X}", !crc))
}

/// First game from a jeuRecherche.php (search by name) answer whose name matches.
pub fn parse_search(json: &Value, want_title: &str) -> Option<GameInfo> {
    let want = crate::title::match_key(want_title);
    let games = json["response"]["jeux"].as_array()?;
    for g in games.iter().take(10) {
        let names: Vec<String> = g["noms"].as_array().into_iter().flatten().filter_map(|n| n["text"].as_str().map(String::from)).collect();
        let hit = names.iter().any(|n| {
            let k = crate::title::match_key(n);
            k == want || (k.len() > 6 && want.len() > 6 && (k.starts_with(&want) || want.starts_with(&k)))
        });
        if hit {
            return parse_game(&serde_json::json!({ "response": { "jeu": g } }));
        }
    }
    None
}

/// Whether ScreenScraper's name should replace the name taken from the file.
/// No-Intro/Redump file names are already good; Switch/Vita dumps and
/// abbreviations ("TOTK") are not.
pub fn should_use_name(system: &str, current: &str) -> bool {
    let letters: Vec<char> = current.chars().filter(|c| c.is_alphabetic()).collect();
    let abbreviation = !current.contains(' ') && letters.len() <= 6 && letters.iter().all(|c| c.is_uppercase());
    matches!(system, "switch" | "psvita") || abbreviation
}

#[derive(Debug, Default)]
pub struct DetailsReport {
    pub looked_up: usize,
    pub found: usize,
    pub not_found: Vec<String>,
    pub covers: usize,
    pub videos: usize,
    /// Set when ScreenScraper refused us (bad login, quota, API closed): the run stops early.
    pub stopped: Option<String>,
}

/// Look up every game that has no cached info yet.
pub fn fetch_details(lib: &Library, media: &Path, creds: &Credentials, progress: &dyn Fn(usize, usize, &str)) -> DetailsReport {
    let mut report = DetailsReport::default();
    let agent = covers::agent();
    let todo: Vec<(&str, &Game)> = lib
        .systems
        .iter()
        .filter(|s| system_id(s.id).is_some())
        .flat_map(|s| s.games.iter().map(move |g| (s.id, g)))
        .filter(|(_, g)| needs_lookup(media, g))
        .collect();
    let total = todo.len();
    let enc = |s: &str| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string();
    let (user, pass, devid, devpw) = (
        creds.ss_user.clone().unwrap_or_default(),
        creds.ss_password.clone().unwrap_or_default(),
        creds.dev_key().map(|k| k.0).unwrap_or_default(),
        creds.dev_key().map(|k| k.1).unwrap_or_default(),
    );

    for (n, (sys, game)) in todo.into_iter().enumerate() {
        progress(n + 1, total, &game.title);
        // The file name without a ranking number ("02. Super Mario 64 (USA).z64" -> "Super Mario 64 (USA).z64").
        let file_name = covers::strip_rank(&game.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
        let size = fs::metadata(&game.path).map(|m| m.len()).unwrap_or(0);
        let crc = file_crc32(&game.path).map(|c| format!("&crc={c}")).unwrap_or_default();
        // The user's own account is optional: without one, ScreenScraper still answers, more slowly.
        let account = if user.is_empty() || pass.is_empty() { String::new() } else { format!("&ssid={}&sspassword={}", enc(&user), enc(&pass)) };
        let auth = format!("devid={}&devpassword={}&softname={SOFTNAME}&output=json{account}&systemeid={}", enc(&devid), enc(&devpw), system_id(sys).unwrap());
        let by_file = format!("{API}/jeuInfos.php?{auth}&romtype=rom&romnom={}&romtaille={size}{crc}", enc(&file_name));
        let by_name = format!("{API}/jeuRecherche.php?{auth}&recherche={}", enc(&game.title));
        report.looked_up += 1;
        // Ask by file first; if ScreenScraper doesn't know the file, search by the game's name.
        let mut info = None;
        let mut stop = None;
        for (i, url) in [&by_file, &by_name].into_iter().enumerate() {
            let body = match covers::get_text_any(&agent, url) {
                Ok(b) => b,
                Err(e) => {
                    stop = Some(format!("couldn't reach ScreenScraper ({e})"));
                    break;
                }
            };
            match serde_json::from_str::<Value>(&body) {
                Ok(v) => {
                    info = if i == 0 { parse_game(&v) } else { parse_search(&v, &game.title) };
                }
                Err(_) => {
                    let low = body.trim().to_lowercase();
                    if !(low.contains("non trouv") || low.contains("not found") || low.contains("aucun")) {
                        // Login problems, daily quota, API closed for maintenance...
                        stop = Some(translate_error(body.trim()));
                        break;
                    }
                }
            }
            if info.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
        if let Some(why) = stop {
            report.stopped = Some(why);
            break;
        }
        let path = info_path(media, &game.id);
        if let Some(d) = path.parent() {
            let _ = fs::create_dir_all(d);
        }
        match info {
            Some(info) => {
                report.found += 1;
                // Box art, when the game still has none.
                if covers::existing_cover(media, &game.id).is_none() {
                    if let Some(u) = &info.box_url {
                        let ext = if info.box_format.as_deref() == Some("jpg") { "jpg" } else { "png" };
                        let dest = covers::cover_base(media, &game.id).with_extension(ext);
                        if covers::download_image(&agent, u, &dest).is_ok() {
                            report.covers += 1;
                        }
                    }
                }
                // Gameplay video, when the game has none yet.
                if covers::existing_video(media, game).is_none() {
                    if let Some(u) = &info.video_url {
                        if covers::download_video(&agent, u, &covers::video_base(media, &game.id)).is_ok() {
                            report.videos += 1;
                        }
                    }
                }
                let _ = fs::write(&path, serde_json::to_string_pretty(&info).unwrap_or_default());
            }
            None => {
                report.not_found.push(game.title.clone());
                let miss = GameInfo { not_found_at: Some(now()), lookup: Some(LOOKUP_VERSION), ..Default::default() };
                let _ = fs::write(&path, serde_json::to_string(&miss).unwrap_or_default());
            }
        }
        std::thread::sleep(Duration::from_millis(1200)); // free accounts: one request at a time
    }
    report
}

fn translate_error(msg: &str) -> String {
    let low = msg.to_lowercase();
    let short: String = msg.chars().take(160).collect();
    if low.contains("identifiants d") || low.contains("developer") || low.contains("développeur") {
        format!("ScreenScraper didn't accept the developer key in credentials.txt ({short})")
    } else if low.contains("login") || low.contains("identifiant") || low.contains("password") {
        format!("ScreenScraper didn't accept the username/password in credentials.txt ({short})")
    } else if low.contains("quota") || low.contains("nombre") || low.contains("limit") {
        format!("ScreenScraper's daily limit is used up; the rest will be looked up next time ({short})")
    } else if low.contains("ferm") || low.contains("closed") || low.contains("maintenance") {
        format!("ScreenScraper is closed right now (it does this when overloaded); try again later ({short})")
    } else {
        format!("ScreenScraper answered: {short}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_search() {
        let p = std::env::temp_dir().join(format!("spine-crc-{}.gba", std::process::id()));
        fs::write(&p, b"123456789").unwrap();
        assert_eq!(file_crc32(&p).as_deref(), Some("CBF43926")); // the standard CRC32 check value
        let _ = fs::remove_file(&p);
        let v = serde_json::json!({"response": {"jeux": [
            {"id": "1", "noms": [{"region": "us", "text": "Mario Kart: Super Circuit"}]},
            {"id": "2", "noms": [{"region": "us", "text": "Super Mario 64"}], "synopsis": [{"langue": "en", "text": "Mario's first 3D adventure."}]}
        ]}});
        assert_eq!(parse_search(&v, "Super Mario 64").and_then(|i| i.description).as_deref(), Some("Mario's first 3D adventure."));
        assert!(parse_search(&v, "Golden Sun").is_none());
        assert_eq!(covers::strip_rank("02. Super Mario 64 (USA).z64"), "Super Mario 64 (USA).z64");
    }

    #[test]
    fn parses_a_response() {
        let v: Value = serde_json::from_str(r#"{"response":{"jeu":{
            "id":"123",
            "noms":[{"region":"wor","text":"The Legend of Zelda : Tears of the Kingdom"},{"region":"jp","text":"ゼルダの伝説"}],
            "synopsis":[{"langue":"fr","text":"Bonjour"},{"langue":"en","text":"An epic adventure."}],
            "dates":[{"region":"eu","text":"2023-05-12"},{"region":"us","text":"2023-05-12"}],
            "genres":[{"noms":[{"langue":"en","text":"Action / Adventure"}]}],
            "joueurs":{"text":"1"},
            "developpeur":{"text":"Nintendo"},
            "medias":[{"type":"ss","region":"wor","url":"https://x/ss.png","format":"png"},
                      {"type":"box-2D","region":"jp","url":"https://x/jp.png","format":"png"},
                      {"type":"box-2D","region":"us","url":"https://x/us.jpg","format":"jpg"}]}}}"#).unwrap();
        let i = parse_game(&v).unwrap();
        assert_eq!(i.name.as_deref(), Some("The Legend of Zelda: Tears of the Kingdom"));
        assert_eq!(i.description.as_deref(), Some("An epic adventure."));
        assert_eq!(i.year.as_deref(), Some("2023"));
        assert_eq!(i.genre.as_deref(), Some("Action / Adventure"));
        assert_eq!(i.box_url.as_deref(), Some("https://x/us.jpg"));
        assert_eq!(i.box_format.as_deref(), Some("jpg"));
    }

    #[test]
    fn name_override_rules() {
        assert!(should_use_name("switch", "TOTK"));
        assert!(should_use_name("gba", "TOTK"));
        assert!(!should_use_name("gba", "Golden Sun: The Lost Age"));
        assert!(!should_use_name("n64", "GoldenEye 007"));
        assert!(should_use_name("psvita", "Gravity Rush"));
    }
}
