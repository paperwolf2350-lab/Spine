//! Categories and facts for every game, without any account:
//!   * retro consoles: libretro-database "metadat" files (genre, franchise,
//!     developer, release year), matched by the same No-Intro names as covers;
//!   * PC games: SteamSpy (Steam's community tags like "Roguelite", developer).
//!
//! Everything is saved in `<media>/meta.json` (game id -> GameMeta).

use crate::covers::{self, libretro_repos, ArtIndex};
use crate::scan::Library;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GameMeta {
    /// Broad genres ("Platformer", "RPG").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    /// Finer community tags ("Roguelite", "Co-op"), most voted first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub franchise: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
}

impl GameMeta {
    fn is_empty(&self) -> bool {
        *self == GameMeta::default()
    }
}

/// Parse a clrmamepro metadat file into (game name, value) pairs for `field`.
pub fn parse_dat(text: &str, field: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut value: Option<String> = None;
    let quoted = |line: &str, key: &str| -> Option<String> {
        let rest = line.strip_prefix(key)?.trim_start();
        let rest = rest.strip_prefix('"')?;
        Some(rest[..rest.rfind('"')?].to_string())
    };
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("game (") {
            name = None;
            value = None;
        } else if let Some(n) = quoted(l, "comment").or_else(|| quoted(l, "name")) {
            name.get_or_insert(n);
        } else if let Some(v) = quoted(l, field) {
            value = Some(v);
        } else if l == ")" {
            if let (Some(n), Some(v)) = (name.take(), value.take()) {
                out.push((n, v));
            }
        }
    }
    out
}

/// Tidy genre names so the same category isn't split in two.
pub fn normalize_genre(g: &str) -> Vec<String> {
    let g = g.trim();
    let mapped = match g.to_lowercase().as_str() {
        "role-playing (rpg)" | "role-playing" | "role playing" | "rpg" => "RPG",
        "platform" | "platformer" => "Platformer",
        "shoot'em up" | "shoot 'em up" | "shmup" => "Shoot 'em up",
        "beat'em up" | "beat 'em up" => "Beat 'em up",
        "casual game" => "Casual",
        "music / dancing" => "Music",
        "hunting and fishing" => "Hunting & Fishing",
        "various" | "compilation" => "Compilation",
        "thinking" => "Puzzle",
        _ => g,
    };
    if mapped.is_empty() {
        return vec![];
    }
    // "Action, Adventure" / "Action / Adventure"
    if mapped.contains(',') || mapped.contains(" / ") {
        return mapped.split([',', '/']).flat_map(normalize_genre).collect();
    }
    vec![mapped.to_string()]
}

/// Steam tags that describe the store, not the game.
const SKIP_TAGS: &[&str] = &["Indie", "Early Access", "Free to Play", "Great Soundtrack", "Singleplayer", "Steam Achievements", "Full controller support", "Steam Cloud", "Controller", "Masterpiece", "Classic", "Cute", "Funny", "Beautiful", "Addictive", "Difficult", "Family Friendly", "Colorful", "Relaxing", "Atmospheric", "Replay Value", "Moddable", "Sandbox"];

/// Read a SteamSpy appdetails answer into GameMeta (top tags, genres, developer).
pub fn parse_steamspy(json: &[u8]) -> Option<GameMeta> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let mut tags: Vec<(String, i64)> = v["tags"].as_object().map(|o| o.iter().map(|(k, n)| (k.clone(), n.as_i64().unwrap_or(0))).collect()).unwrap_or_default();
    tags.sort_by(|a, b| b.1.cmp(&a.1));
    let tags: Vec<String> = tags.into_iter().map(|(k, _)| k).filter(|t| !SKIP_TAGS.contains(&t.as_str())).take(8).collect();
    let genres: Vec<String> = v["genre"].as_str().map(normalize_genre).unwrap_or_default().into_iter().filter(|g| !SKIP_TAGS.contains(&g.as_str())).collect();
    let developer = v["developer"].as_str().filter(|d| !d.is_empty()).map(String::from);
    let m = GameMeta { genres, tags, franchise: None, developer, year: None };
    (!m.is_empty()).then_some(m)
}

pub fn meta_path(media: &Path) -> std::path::PathBuf {
    media.join("meta.json")
}

pub fn load(media: &Path) -> BTreeMap<String, GameMeta> {
    fs::read_to_string(meta_path(media)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// A metadat file for one console, cached for two weeks.
fn fetch_dat(kind: &str, repo: &str, cache: &Path) -> Option<String> {
    let file = cache.join(kind).join(format!("{repo}.dat"));
    if let Ok(meta) = fs::metadata(&file) {
        let fresh = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).map_or(false, |a| a < Duration::from_secs(14 * 24 * 3600));
        if fresh {
            return fs::read_to_string(&file).ok();
        }
    }
    let name = repo.replace('_', " ");
    let url = format!("https://raw.githubusercontent.com/libretro/libretro-database/master/metadat/{kind}/{}.dat", name.replace(' ', "%20"));
    let bytes = covers::get_text_any(&covers::agent(), &url).ok()?;
    if !bytes.contains("game (") {
        // 404 page or similar: remember "nothing here" so we don't ask again for two weeks.
        let _ = fs::create_dir_all(file.parent().unwrap());
        let _ = fs::write(&file, "");
        return None;
    }
    let _ = fs::create_dir_all(file.parent().unwrap());
    let _ = fs::write(&file, &bytes);
    Some(bytes)
}

/// Look up categories for every game. `dats` lets tests supply files directly.
pub fn build(lib: &Library, media: &Path, dats: Option<&Path>, progress: &dyn Fn(usize, usize, &str)) -> BTreeMap<String, GameMeta> {
    let mut all: BTreeMap<String, GameMeta> = load(media);
    let cache = media.join("lists").join("metadat");
    let total = lib.systems.len();
    for (n, s) in lib.systems.iter().enumerate() {
        progress(n + 1, total, s.name);
        if s.id == "pc" {
            for g in &s.games {
                if all.get(&g.id).map_or(false, |m| !m.tags.is_empty() || !m.genres.is_empty()) {
                    continue; // already known
                }
                if let Some(m) = steamspy_for(g) {
                    all.insert(g.id.clone(), m);
                }
            }
            continue;
        }
        let repos = libretro_repos(s.id);
        if repos.is_empty() {
            continue;
        }
        let mut fields: HashMap<&str, (ArtIndex, HashMap<String, String>)> = HashMap::new();
        for kind in ["genre", "franchise", "developer", "releaseyear"] {
            // A console can span several lists (the GBC folder also holds Game Boy games).
            let mut lists = Vec::new();
            let mut map: HashMap<String, String> = HashMap::new();
            for repo in repos {
                let text = match dats {
                    Some(dir) => fs::read_to_string(dir.join(kind).join(format!("{}.dat", repo.replace('_', " ")))).ok(),
                    None => fetch_dat(kind, repo, &cache),
                };
                let Some(text) = text else { continue };
                let pairs = parse_dat(&text, kind);
                if pairs.is_empty() {
                    continue;
                }
                lists.push((*repo, pairs.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()));
                map.extend(pairs);
            }
            if !lists.is_empty() {
                fields.insert(kind, (ArtIndex::new(lists), map));
            }
        }
        if fields.is_empty() {
            continue;
        }
        for g in &s.games {
            let mut m = all.get(&g.id).cloned().unwrap_or_default();
            let get = |kind: &str| fields.get(kind).and_then(|(idx, map)| idx.find(g).and_then(|e| map.get(&e.name).cloned()));
            if let Some(v) = get("genre") {
                m.genres = normalize_genre(&v);
            }
            if let Some(v) = get("franchise") {
                m.franchise = Some(v);
            }
            if let Some(v) = get("developer") {
                m.developer = Some(v);
            }
            if let Some(v) = get("releaseyear") {
                m.year = Some(v.chars().take(4).collect());
            }
            if !m.is_empty() {
                all.insert(g.id.clone(), m);
            }
        }
    }
    all
}

fn steamspy_for(g: &crate::scan::Game) -> Option<GameMeta> {
    let agent = covers::agent();
    let app = covers::steam_app_id(&g.path).or_else(|| {
        let mut names = vec![g.title.clone()];
        if g.format == "lnk" {
            if let Some(f) = covers::lnk_folder_name(&g.path) {
                names.insert(0, f);
            }
        }
        covers::steam_search_pub(&agent, &names).ok()
    })?;
    let body = covers::get_text_any(&agent, &format!("https://steamspy.com/api.php?request=appdetails&appid={app}")).ok()?;
    std::thread::sleep(Duration::from_millis(1100)); // SteamSpy asks for about one request per second
    parse_steamspy(body.as_bytes())
}

pub fn save(media: &Path, all: &BTreeMap<String, GameMeta>) {
    let _ = fs::create_dir_all(media);
    let _ = fs::write(meta_path(media), serde_json::to_string_pretty(all).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dat_and_genres() {
        let t = "clrmamepro (\n\tname \"x\"\n)\n\ngame (\n\tcomment \"Golden Sun (USA, Europe)\"\n\tgenre \"Role-playing (RPG)\"\n\trom ( crc E1FB68E8 )\n)\n\ngame (\n\tcomment \"007 - GoldenEye (Europe)\"\n\tfranchise \"James Bond 007\"\n\trom (\n\t\tname x crc 1\n\t)\n)\n";
        assert_eq!(parse_dat(t, "genre"), vec![("Golden Sun (USA, Europe)".to_string(), "Role-playing (RPG)".to_string())]);
        assert_eq!(parse_dat(t, "franchise")[0].1, "James Bond 007");
        assert_eq!(normalize_genre("Role-playing (RPG)"), vec!["RPG"]);
        assert_eq!(normalize_genre("Action, Adventure"), vec!["Action", "Adventure"]);
        assert_eq!(normalize_genre("Platform"), vec!["Platformer"]);
    }

    #[test]
    fn steamspy() {
        let j = br#"{"appid":1966720,"developer":"Zeekerss","genre":"Action, Adventure, Indie, Early Access","tags":{"Online Co-Op":2082,"Horror":1941,"Indie":1500,"Roguelite":900,"Funny":1198}}"#;
        let m = parse_steamspy(j).unwrap();
        assert_eq!(m.tags, vec!["Online Co-Op", "Horror", "Roguelite"]);
        assert_eq!(m.genres, vec!["Action", "Adventure"]);
        assert_eq!(m.developer.as_deref(), Some("Zeekerss"));
    }
}
