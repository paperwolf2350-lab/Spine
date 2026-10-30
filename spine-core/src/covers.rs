//! Box art: finding the right cover for each game and caching it on disk.
//!
//! Sources (no account needed):
//!   * libretro-thumbnails — community box art named after No-Intro / Redump
//!     dumps, one GitHub repository per console.
//!   * Steam — official portrait covers for PC games, by Steam app id.
//!
//! Covers are stored as `<media>/covers/<system>/<slug>.png|jpg`, so the path
//! follows from the game id and nothing else needs remembering between runs.

use crate::scan::{Game, Library};
use crate::title::{self, ParsedName};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// libretro-thumbnails repositories for a Spine system, best first.
pub fn libretro_repos(system: &str) -> &'static [&'static str] {
    match system {
        "nes" => &["Nintendo_-_Nintendo_Entertainment_System"],
        "fds" => &["Nintendo_-_Family_Computer_Disk_System"],
        "snes" => &["Nintendo_-_Super_Nintendo_Entertainment_System"],
        "n64" => &["Nintendo_-_Nintendo_64"],
        "gb" => &["Nintendo_-_Game_Boy"],
        "gbc" => &["Nintendo_-_Game_Boy_Color", "Nintendo_-_Game_Boy"],
        "gba" => &["Nintendo_-_Game_Boy_Advance"],
        "nds" => &["Nintendo_-_Nintendo_DS"],
        "n3ds" => &["Nintendo_-_Nintendo_3DS"],
        "virtualboy" => &["Nintendo_-_Virtual_Boy"],
        "gc" => &["Nintendo_-_GameCube"],
        "wii" => &["Nintendo_-_Wii"],
        "wiiu" => &["Nintendo_-_Wii_U"],
        "mastersystem" => &["Sega_-_Master_System_-_Mark_III"],
        "genesis" => &["Sega_-_Mega_Drive_-_Genesis"],
        "gamegear" => &["Sega_-_Game_Gear"],
        "segacd" => &["Sega_-_Mega-CD_-_Sega_CD"],
        "sega32x" => &["Sega_-_32X"],
        "saturn" => &["Sega_-_Saturn"],
        "dreamcast" => &["Sega_-_Dreamcast"],
        "psx" => &["Sony_-_PlayStation"],
        "ps2" => &["Sony_-_PlayStation_2"],
        "ps3" => &["Sony_-_PlayStation_3"],
        "psp" => &["Sony_-_PlayStation_Portable"],
        "psvita" => &["Sony_-_PlayStation_Vita"],
        "pcengine" => &["NEC_-_PC_Engine_-_TurboGrafx_16"],
        "pcenginecd" => &["NEC_-_PC_Engine_CD_-_TurboGrafx-CD"],
        "neogeo" => &["SNK_-_Neo_Geo"],
        "ngpc" => &["SNK_-_Neo_Geo_Pocket_Color", "SNK_-_Neo_Geo_Pocket"],
        "arcade" => &["FBNeo_-_Arcade_Games", "MAME"],
        "atari2600" => &["Atari_-_2600"],
        "atarilynx" => &["Atari_-_Lynx"],
        "wonderswan" => &["Bandai_-_WonderSwan_Color", "Bandai_-_WonderSwan"],
        "dos" => &["DOS"],
        "xbox" => &["Microsoft_-_Xbox"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Matching a game to a name in a libretro box-art list
// ---------------------------------------------------------------------------

/// libretro replaces these characters with '_' in file names.
pub fn libretro_safe(name: &str) -> String {
    name.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect()
}

/// One entry of a box-art list, pre-parsed for matching.
#[derive(Debug, Clone)]
pub struct ArtEntry {
    /// File name without ".png", exactly as in the repository.
    pub name: String,
    pub repo: &'static str,
    parsed: ParsedName,
    key: String,
    words: Vec<String>,
}

pub struct ArtIndex {
    entries: Vec<ArtEntry>,
    exact: HashMap<String, usize>,
    by_key: HashMap<String, Vec<usize>>,
}

impl ArtIndex {
    pub fn new(lists: Vec<(&'static str, Vec<String>)>) -> Self {
        let mut entries = Vec::new();
        for (repo, names) in lists {
            for n in names {
                let parsed = title::parse_name(&n, false);
                let key = title::match_key(&parsed.title);
                let words = words_of(&parsed.title);
                entries.push(ArtEntry { name: n, repo, parsed, key, words });
            }
        }
        let mut exact = HashMap::new();
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            exact.entry(e.name.to_lowercase()).or_insert(i);
            by_key.entry(e.key.clone()).or_default().push(i);
        }
        ArtIndex { entries, exact, by_key }
    }

    /// The entry with exactly this file name (for tests and tools).
    pub fn entries_named(&self, name: &str) -> Option<ArtEntry> {
        self.entries.iter().find(|e| e.name == name).cloned()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find the best box art for a game, or None.
    pub fn find(&self, game: &Game) -> Option<&ArtEntry> {
        // 1. A file named exactly like the dump (minus any "01. " ranking).
        for v in &game.versions {
            let stem = v.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let stem = strip_rank(&stem);
            if let Some(&i) = self.exact.get(&libretro_safe(&stem).to_lowercase()) {
                return Some(&self.entries[i]);
            }
        }

        // 2. Same title once tags are ignored; pick the best region/release.
        let mut keys = vec![title::match_key(&game.title)];
        for v in &game.versions {
            let stem = v.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            keys.push(title::match_key(&title::parse_name(&stem, false).title));
        }
        keys.dedup();
        let region = game.region.as_deref().unwrap_or("");
        for k in &keys {
            if let Some(ids) = self.by_key.get(k) {
                return ids.iter().map(|&i| &self.entries[i]).min_by_key(|e| rank_entry(e, region));
            }
        }

        // 3. Close enough: most words shared and every number the same
        //    ("Need for Speed Most Wanted U" ~ "Need for Speed - Most Wanted U - A Criterion Game",
        //     but never "Bayonetta" ~ "Bayonetta 2").
        let words = words_of(&game.title);
        if words.len() < 2 {
            return None;
        }
        let nums: Vec<&String> = words.iter().filter(|w| is_number(w)).collect();
        let mut best: Option<(f32, &ArtEntry)> = None;
        for e in &self.entries {
            let enums: Vec<&String> = e.words.iter().filter(|w| is_number(w)).collect();
            if enums != nums {
                continue;
            }
            let shared = words.iter().filter(|w| e.words.contains(w)).count() as f32;
            let score = shared / (words.len().max(e.words.len()) as f32);
            let covers_all = words.iter().all(|w| e.words.contains(w)) || e.words.iter().all(|w| words.contains(w));
            if score >= 0.6 && covers_all {
                let better = match best {
                    None => true,
                    Some((s, b)) => score > s || (score == s && rank_entry(e, region) < rank_entry(b, region)),
                };
                if better {
                    best = Some((score, e));
                }
            }
        }
        best.map(|(_, e)| e)
    }
}

fn strip_rank(s: &str) -> String {
    let t = s.trim_start();
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=3).contains(&digits) {
        let rest = &t[digits..];
        let rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')')) {
            return r.trim_start().to_string();
        }
    }
    t.to_string()
}

fn words_of(t: &str) -> Vec<String> {
    let t = title::fold_accents(t).to_lowercase().replace('&', " and ");
    t.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an" | "of" | "and"))
        .map(|w| w.to_string())
        .collect()
}

fn is_number(w: &str) -> bool {
    w.chars().all(|c| c.is_ascii_digit()) || (w.len() <= 4 && w.chars().all(|c| matches!(c, 'i' | 'v' | 'x')) && w != "x")
}

/// Lower is better: USA/World first, then no region, Europe, others; no betas; the earlier repo first.
fn rank_entry(e: &ArtEntry, want_region: &str) -> (u8, bool, u8, usize) {
    let r = e.parsed.region.as_deref().unwrap_or("").to_lowercase();
    let want = want_region.to_lowercase();
    let same = !want.is_empty() && r == want;
    let region = if same { 0 } else if r.contains("usa") || r.contains("world") { 1 } else if r.is_empty() { 2 } else if r.contains("europe") { 3 } else { 4 };
    (region, e.parsed.prerelease, e.parsed.revision.is_some() as u8, e.name.len())
}

// ---------------------------------------------------------------------------
// Steam
// ---------------------------------------------------------------------------

/// Steam app id from a ".url" shortcut ("URL=steam://rungameid/1145360").
pub fn steam_app_id(path: &Path) -> Option<u32> {
    if let Some(id) = crate::stores::steam_uri_id(path) {
        return Some(id); // an installed Steam game
    }
    let text = fs::read_to_string(path).ok()?;
    let i = text.find("steam://rungameid/")? + "steam://rungameid/".len();
    let digits: String = text[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// For a Windows shortcut, the name of the folder holding the game's .exe
/// ("D:\\Games\\BALLxPIT\\Balls.exe" -> "BALLxPIT"), which is often a better
/// name than the shortcut's own.
pub fn lnk_folder_name(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() > 1_000_000 {
        return None;
    }
    // Shortcuts store the target path as plain ASCII; find "X:\...\something.exe".
    let text: String = bytes.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '\n' }).collect();
    let line = text.lines().find(|l| l.len() > 6 && l.as_bytes()[1] == b':' && l.to_lowercase().ends_with(".exe"))?;
    let parts: Vec<&str> = line.split('\\').collect();
    (parts.len() >= 3).then(|| parts[parts.len() - 2].to_string())
}

/// Pick the Steam app whose name matches one of `names`, from a store search result.
pub fn pick_steam_result(json: &[u8], names: &[String]) -> Option<u32> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let items = v["items"].as_array()?;
    let candidates: Vec<(u32, String)> = items
        .iter()
        .filter_map(|i| Some((i["id"].as_u64()? as u32, i["name"].as_str()?.to_string())))
        .collect();
    // Exact title first, then a close match that keeps every word and number.
    for n in names {
        let k = title::match_key(n);
        if let Some((id, _)) = candidates.iter().find(|(_, name)| title::match_key(name) == k) {
            return Some(*id);
        }
    }
    for n in names {
        let w = words_of(n);
        if w.is_empty() {
            continue;
        }
        let nums: Vec<&String> = w.iter().filter(|x| is_number(x)).collect();
        for (id, name) in &candidates {
            let cw = words_of(name);
            let cnums: Vec<&String> = cw.iter().filter(|x| is_number(x)).collect();
            if cnums == nums && w.iter().all(|x| cw.contains(x)) && cw.len() <= w.len() + 2 {
                return Some(*id);
            }
        }
    }
    None
}

/// Find a Steam app id by game name (shared with the metadata lookup).
pub fn steam_search_pub(agent: &Agent, names: &[String]) -> Result<u32, String> {
    steam_search(agent, names)
}

pub(crate) fn steam_search(agent: &Agent, names: &[String]) -> Result<u32, String> {
    for n in names {
        let url = format!("https://store.steampowered.com/api/storesearch/?cc=us&l=english&term={}", utf8_percent_encode(n, PATH_SEG));
        let json = get_bytes(agent, &url)?;
        if let Some(id) = pick_steam_result(&json, names) {
            return Ok(id);
        }
    }
    Err("not found on Steam".into())
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// Downloads go through the system `curl` (built into Windows 10+, macOS and
/// Linux), so Spine needs no TLS library or C compiler to build.
#[derive(Clone, Copy)]
pub struct Agent;

pub fn agent() -> Agent {
    Agent
}

pub(crate) const PATH_SEG: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

pub fn curl(url: &str, fail_on_http_error: bool, headers: &[String]) -> Result<Vec<u8>, String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sSL", "--connect-timeout", "15", "--max-time", "90", "--retry", "2", "-A", "Spine/0.1 (game library frontend)"]);
    if fail_on_http_error {
        cmd.arg("--fail");
    }
    for h in headers {
        cmd.arg("-H").arg(h);
    }
    cmd.arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashes
    }
    let out = match cmd.output() {
        Ok(o) => o,
        // Some Linux systems have no curl: use the built-in downloader instead.
        #[cfg(target_os = "linux")]
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return builtin_get(url, fail_on_http_error, headers),
        Err(e) => return Err(format!("couldn't run curl: {e}")),
    };
    if out.status.success() {
        Ok(out.stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { format!("curl exit {}", out.status) } else { err })
    }
}

#[cfg(target_os = "linux")]
pub fn builtin_get(url: &str, fail_on_http_error: bool, headers: &[String]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout(Duration::from_secs(300)).user_agent("Spine/0.1 (game library frontend)").build();
    let mut req = agent.get(url);
    for h in headers {
        if let Some((k, v)) = h.split_once(':') {
            req = req.set(k.trim(), v.trim());
        }
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            if fail_on_http_error {
                return Err(format!("The requested URL returned error: {code}"));
            }
            r
        }
        Err(e) => return Err(e.to_string()),
    };
    let mut buf = Vec::new();
    resp.into_reader().take(1 << 30).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

pub(crate) fn get_bytes(_agent: &Agent, url: &str) -> Result<Vec<u8>, String> {
    curl(url, true, &[])
}

/// The body of a response whatever its status (some APIs explain errors in the body).
pub fn get_text_any(_agent: &Agent, url: &str) -> Result<String, String> {
    curl(url, false, &[]).map(|b| String::from_utf8_lossy(&b).to_string())
}

pub(crate) fn is_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8]) {
        Some("jpg")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Download any png/jpg image to `dest` (its extension is corrected to match the data).
pub fn download_image(agent: &Agent, url: &str, dest: &Path) -> Result<PathBuf, String> {
    let b = get_bytes(agent, url)?;
    match is_image(&b) {
        Some(ext @ ("png" | "jpg")) => {
            let dest = dest.with_extension(ext);
            save(&dest, &b)?;
            Ok(dest)
        }
        _ => Err("not a png/jpg image".into()),
    }
}

/// The list of box-art names for one repository, cached for two weeks.
pub fn fetch_list(agent: &Agent, repo: &str, cache_dir: &Path) -> Result<Vec<String>, String> {
    let cache = cache_dir.join(format!("{repo}.txt"));
    if let Ok(meta) = fs::metadata(&cache) {
        let fresh = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).map_or(false, |age| age < Duration::from_secs(14 * 24 * 3600));
        if fresh {
            if let Ok(text) = fs::read_to_string(&cache) {
                return Ok(text.lines().filter(|l| !l.is_empty()).map(String::from).collect());
            }
        }
    }

    let mut names = Vec::new();
    // GitHub's file listing for the repository (one request).
    let api = format!("https://api.github.com/repos/libretro-thumbnails/{repo}/git/trees/master?recursive=1");
    match get_bytes(agent, &api) {
        Ok(bytes) => {
            let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            for item in v["tree"].as_array().into_iter().flatten() {
                if let Some(p) = item["path"].as_str() {
                    if let Some(n) = p.strip_prefix("Named_Boxarts/").and_then(|n| n.strip_suffix(".png")) {
                        names.push(n.to_string());
                    }
                }
            }
        }
        Err(api_err) => {
            // Fallback: the directory page on thumbnails.libretro.com.
            let sys = repo.replace('_', " ");
            let url = format!("https://thumbnails.libretro.com/{}/Named_Boxarts/", utf8_percent_encode(&sys, PATH_SEG));
            let html = String::from_utf8_lossy(&get_bytes(agent, &url).map_err(|e| format!("{api_err}; {e}"))?).to_string();
            for part in html.split("href=\"").skip(1) {
                let href = part.split('"').next().unwrap_or("");
                if let Some(n) = href.strip_suffix(".png") {
                    let decoded = percent_encoding::percent_decode_str(n).decode_utf8_lossy().to_string();
                    names.push(decoded.replace("&amp;", "&"));
                }
            }
        }
    }
    if names.is_empty() {
        return Err("empty list".into());
    }
    let _ = fs::create_dir_all(cache_dir);
    let _ = fs::write(&cache, names.join("\n"));
    Ok(names)
}

fn download_art(agent: &Agent, entry: &ArtEntry, dest: &Path) -> Result<(), String> {
    let file = format!("{}.png", entry.name);
    let urls = [
        format!("https://raw.githubusercontent.com/libretro-thumbnails/{}/master/Named_Boxarts/{}", entry.repo, utf8_percent_encode(&file, PATH_SEG)),
        format!("https://thumbnails.libretro.com/{}/Named_Boxarts/{}", utf8_percent_encode(&entry.repo.replace('_', " "), PATH_SEG), utf8_percent_encode(&file, PATH_SEG)),
    ];
    let mut last = String::new();
    for u in &urls {
        match get_bytes(agent, u) {
            Ok(b) if b.starts_with(&[0x89, b'P', b'N', b'G']) => return save(dest, &b),
            Ok(_) => last = "not an image".into(),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn download_steam(agent: &Agent, app: u32, dest: &Path) -> Result<(), String> {
    let urls = [
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/library_600x900.jpg"),
        format!("https://shared.steamstatic.com/store_item_assets/steam/apps/{app}/library_600x900.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/header.jpg"),
    ];
    let mut last = String::new();
    for u in &urls {
        match get_bytes(agent, u) {
            Ok(b) if b.starts_with(&[0xFF, 0xD8]) => return save(dest, &b),
            Ok(_) => last = "not an image".into(),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub(crate) fn save(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = dest.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("part");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Whole-library pass
// ---------------------------------------------------------------------------

/// Where a game's cover lives (whether or not it exists yet).
pub fn cover_base(media: &Path, game_id: &str) -> PathBuf {
    let mut p = media.join("covers");
    for part in game_id.split('/') {
        p.push(part);
    }
    p
}

/// Where a user-chosen cover for a game lives (without extension).
pub fn custom_base(media: &Path, game_id: &str) -> PathBuf {
    let mut p = media.join("custom");
    for part in game_id.split('/') {
        p.push(part);
    }
    p
}

pub const CUSTOM_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

/// The user's own cover for a game, if they picked one.
pub fn custom_cover(media: &Path, game_id: &str) -> Option<PathBuf> {
    let base = custom_base(media, game_id);
    CUSTOM_EXTS.iter().map(|e| base.with_extension(e)).find(|p| p.exists())
}

/// The cover to show for a game: the user's own first, then a downloaded one.
pub fn existing_cover(media: &Path, game_id: &str) -> Option<PathBuf> {
    if let Some(c) = custom_cover(media, game_id) {
        return Some(c);
    }
    let base = cover_base(media, game_id);
    ["png", "jpg"].iter().map(|e| base.with_extension(e)).find(|p| p.exists())
}

// ---------------------------------------------------------------------------
// Videos
// ---------------------------------------------------------------------------

pub const VIDEO_EXTS: &[&str] = &["mp4", "webm", "m4v"];

/// Where Spine saves a downloaded video for a game (without extension).
pub fn video_base(media: &Path, game_id: &str) -> PathBuf {
    let mut p = media.join("videos");
    for part in game_id.split('/') {
        p.push(part);
    }
    p
}

/// The video to play for a game: the user's own, one Spine downloaded, or one
/// Skraper / ES-DE / Batocera already put next to the ROMs.
pub fn existing_video(media: &Path, game: &crate::scan::Game) -> Option<PathBuf> {
    let custom = custom_base(media, &game.id);
    let downloaded = video_base(media, &game.id);
    let mut candidates: Vec<PathBuf> = Vec::new();
    for base in [&custom, &downloaded] {
        candidates.extend(VIDEO_EXTS.iter().map(|e| base.with_extension(e)));
    }
    // Skraper/ES-DE layouts, named after the ROM file.
    for v in &game.versions {
        let (Some(dir), Some(stem)) = (v.path.parent(), v.path.file_stem()) else { continue };
        let stem = stem.to_string_lossy();
        let sys_folder = dir.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let roots = [
            dir.join("media").join("videos"),
            dir.join("videos"),
            dir.join("snap"),
            dir.parent().map(|p| p.join("downloaded_media").join(&sys_folder).join("videos")).unwrap_or_default(),
            dir.parent().and_then(|p| p.parent()).map(|p| p.join("ES-DE").join("downloaded_media").join(&sys_folder).join("videos")).unwrap_or_default(),
        ];
        for r in roots {
            candidates.extend(VIDEO_EXTS.iter().map(|e| r.join(format!("{stem}.{e}"))));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn custom_video(media: &Path, game_id: &str) -> Option<PathBuf> {
    let base = custom_base(media, game_id);
    VIDEO_EXTS.iter().map(|e| base.with_extension(e)).find(|p| p.exists())
}

fn video_ext(b: &[u8]) -> Option<&'static str> {
    if b.len() > 12 && &b[4..8] == b"ftyp" {
        Some("mp4")
    } else if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some("webm")
    } else {
        None
    }
}

/// Download a video to `base` + its real extension.
pub fn download_video(agent: &Agent, url: &str, base: &Path) -> Result<PathBuf, String> {
    let b = curl(url, true, &[])?;
    let ext = video_ext(&b).ok_or("not an mp4/webm video")?;
    let dest = base.with_extension(ext);
    save(&dest, &b)?;
    let _ = agent;
    Ok(dest)
}

/// The best trailer URL from a Steam appdetails response: an HLS playlist
/// (what Steam serves now) or, on older listings, a plain mp4/webm.
pub fn pick_steam_trailer(json: &[u8], app: u32) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let movies = v[app.to_string()]["data"]["movies"].as_array()?;
    let m = movies.iter().find(|m| m["highlight"].as_bool() == Some(true)).or(movies.first())?;
    if let Some(u) = m["hls_h264"].as_str() {
        return Some(u.to_string());
    }
    for (fmt, q) in [("mp4", "480"), ("mp4", "max"), ("webm", "480"), ("webm", "max")] {
        if let Some(u) = m[fmt][q].as_str() {
            return Some(u.replace("http://", "https://"));
        }
    }
    None
}

/// Resolve a playlist entry against the playlist's own URL.
fn hls_join(playlist_url: &str, rel: &str) -> String {
    if rel.starts_with("http://") || rel.starts_with("https://") {
        return rel.to_string();
    }
    let base = playlist_url.split('?').next().unwrap_or(playlist_url);
    let dir = &base[..base.rfind('/').map(|i| i + 1).unwrap_or(0)];
    format!("{dir}{rel}")
}

/// From an HLS master playlist: (video playlist closest to 480p, audio playlist).
pub fn hls_pick_streams(master_url: &str, master: &str) -> Option<(String, Option<String>)> {
    let mut best: Option<(i64, String)> = None;
    let mut audio = None;
    let mut lines = master.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("#EXT-X-MEDIA:") && line.contains("TYPE=AUDIO") {
            if let Some(uri) = line.split("URI=\"").nth(1).and_then(|r| r.split('"').next()) {
                audio.get_or_insert(hls_join(master_url, uri));
            }
        }
        if line.starts_with("#EXT-X-STREAM-INF:") {
            let h: i64 = line.split("RESOLUTION=").nth(1).and_then(|r| r.split([',', '\n']).next()).and_then(|r| r.split('x').nth(1)).and_then(|h| h.trim().parse().ok()).unwrap_or(0);
            if let Some(uri) = lines.peek().filter(|l| !l.starts_with('#')) {
                let score = (h - 480).abs() + if h < 360 { 1000 } else { 0 };
                if best.as_ref().map_or(true, |(s, _)| score < *s) {
                    best = Some((score, hls_join(master_url, uri.trim())));
                }
            }
        }
    }
    best.map(|(_, v)| (v, audio))
}

/// From a media playlist: the init segment and the first `max` segments.
pub fn hls_segments(playlist_url: &str, playlist: &str, max: usize) -> (Option<String>, Vec<String>) {
    let init = playlist.lines().find(|l| l.starts_with("#EXT-X-MAP:")).and_then(|l| l.split("URI=\"").nth(1)).and_then(|r| r.split('"').next()).map(|u| hls_join(playlist_url, u));
    let segs = playlist.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).take(max).map(|u| hls_join(playlist_url, u)).collect();
    (init, segs)
}

/// Download one HLS stream (init + segments) into a single fragmented MP4 file.
fn download_hls_stream(playlist_url: &str, dest: &Path, max_segments: usize) -> Result<(), String> {
    let text = String::from_utf8_lossy(&curl(playlist_url, true, &[])?).to_string();
    let (init, segs) = hls_segments(playlist_url, &text, max_segments);
    if segs.is_empty() {
        return Err("empty playlist".into());
    }
    let mut out = Vec::new();
    if let Some(i) = init {
        out.extend(curl(&i, true, &[])?);
    }
    for s in segs {
        out.extend(curl(&s, true, &[])?);
    }
    save(dest, &out)
}

/// Download a trailer: plain files directly, HLS playlists stitched into
/// `<base>.mp4` (video) plus `<base>.audio.m4a` (sound), first ~60 seconds.
pub fn download_trailer(agent: &Agent, url: &str, base: &Path) -> Result<PathBuf, String> {
    if !url.split('?').next().unwrap_or("").ends_with(".m3u8") {
        return download_video(agent, url, base);
    }
    let master = String::from_utf8_lossy(&curl(url, true, &[])?).to_string();
    let (video, audio) = hls_pick_streams(url, &master).ok_or("no video stream in playlist")?;
    let dest = base.with_extension("mp4");
    download_hls_stream(&video, &dest, 20)?;
    if let Some(a) = audio {
        let _ = download_hls_stream(&a, &base.with_extension("audio.m4a"), 20);
    }
    Ok(dest)
}

/// Steam trailers for PC games that have no video yet.
pub fn fetch_steam_videos(lib: &mut Library, media: &Path, progress: &(dyn Fn(usize, usize, &str) + Sync)) -> usize {
    let agent = agent();
    let Some(pc) = lib.systems.iter_mut().find(|s| s.id == "pc") else { return 0 };
    let todo: Vec<usize> = (0..pc.games.len()).filter(|&i| existing_video(media, &pc.games[i]).is_none()).collect();
    let total = todo.len();
    let mut got = 0;
    for (n, i) in todo.into_iter().enumerate() {
        let g = &pc.games[i];
        progress(n + 1, total, &g.title);
        let app = steam_app_id(&g.path).or_else(|| {
            let mut names = vec![g.title.clone()];
            if g.format == "lnk" {
                if let Some(f) = lnk_folder_name(&g.path) {
                    names.insert(0, f);
                }
            }
            steam_search(&agent, &names).ok()
        });
        let Some(app) = app else { continue };
        let url = format!("https://store.steampowered.com/api/appdetails?appids={app}&filters=movies");
        let Ok(json) = get_bytes(&agent, &url) else { continue };
        if let Some(t) = pick_steam_trailer(&json, app) {
            if let Ok(p) = download_trailer(&agent, &t, &video_base(media, &g.id)) {
                pc.games[i].video = Some(p);
                got += 1;
            }
        }
    }
    got
}

/// Fill in everything already saved on disk: covers, and ScreenScraper
/// details (whose proper names replace abbreviations like "TOTK").
pub fn attach_existing(lib: &mut Library, media: &Path) {
    let metas = crate::meta::load(media);
    for s in &mut lib.systems {
        for g in &mut s.games {
            g.cover = existing_cover(media, &g.id);
            g.video = existing_video(media, g);
            g.meta = metas.get(&g.id).cloned();
            g.info = crate::screenscraper::load_info(media, &g.id).filter(|i| i.not_found_at.is_none());
            if let Some(name) = g.info.as_ref().and_then(|i| i.name.clone()) {
                if crate::screenscraper::should_use_name(s.id, &g.title) {
                    g.title = name;
                    g.sort_key = title::sort_key(&g.title);
                    g.letter = title::index_letter(&g.sort_key);
                }
            }
        }
        s.games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    }
}

// ---------------------------------------------------------------------------
// SteamGridDB (needs a free API key)
// ---------------------------------------------------------------------------

pub(crate) const SGDB: &str = "https://www.steamgriddb.com/api/v2";

/// Pick the SteamGridDB game whose name matches one of `names` from an autocomplete result.
pub fn pick_sgdb_game(json: &[u8], names: &[String]) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let items = v["data"].as_array()?;
    // Reuse the Steam picker: same shape once mapped to {items:[{id,name}]}.
    let mapped = serde_json::json!({ "items": items.iter().map(|i| serde_json::json!({"id": i["id"], "name": i["name"]})).collect::<Vec<_>>() });
    pick_steam_result(mapped.to_string().as_bytes(), names).map(|id| id as u64)
}

/// First portrait grid URL from a grids response (they come best-voted first).
pub fn pick_sgdb_grid(json: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    v["data"].as_array()?.iter().find_map(|g| g["url"].as_str().map(String::from))
}

fn sgdb_cover(key: &str, names: &[String], dest: &Path) -> Result<PathBuf, String> {
    let auth = vec![format!("Authorization: Bearer {key}")];
    let mut game = None;
    for n in names {
        let url = format!("{SGDB}/search/autocomplete/{}", utf8_percent_encode(n, PATH_SEG));
        let body = curl(&url, false, &auth)?;
        if body.starts_with(b"{\"success\":false") || String::from_utf8_lossy(&body).contains("Unauthorized") {
            return Err("SteamGridDB didn't accept the API key in credentials.txt".into());
        }
        if let Some(id) = pick_sgdb_game(&body, names) {
            game = Some(id);
            break;
        }
    }
    let id = game.ok_or("not on SteamGridDB")?;
    let url = format!("{SGDB}/grids/game/{id}?dimensions=600x900,342x482,660x930&types=static&nsfw=false&humor=false");
    let grid = pick_sgdb_grid(&curl(&url, true, &auth)?).ok_or("no portrait cover on SteamGridDB")?;
    download_image(&agent(), &grid, dest)
}

/// Covers from SteamGridDB for every game that still has none.
pub fn fetch_sgdb_covers(lib: &mut Library, media: &Path, key: &str, progress: &(dyn Fn(usize, usize, &str) + Sync)) -> CoverReport {
    let mut report = CoverReport::default();
    let jobs: Vec<(usize, usize, Vec<String>, PathBuf)> = lib
        .systems
        .iter()
        .enumerate()
        .flat_map(|(si, s)| s.games.iter().enumerate().map(move |(gi, g)| (si, gi, s, g)))
        .filter(|(_, _, _, g)| existing_cover(media, &g.id).is_none())
        .map(|(si, gi, s, g)| {
            let mut names = vec![g.title.clone()];
            if let Some(n) = g.info.as_ref().and_then(|i| i.name.clone()) {
                names.insert(0, n);
            }
            if s.id == "pc" && g.format == "lnk" {
                if let Some(f) = lnk_folder_name(&g.path) {
                    names.push(f);
                }
            }
            (si, gi, names, cover_base(media, &g.id))
        })
        .collect();
    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs));
    let results = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(0usize));
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let (queue, results, done) = (Arc::clone(&queue), Arc::clone(&results), Arc::clone(&done));
            scope.spawn(move || loop {
                let Some((si, gi, names, dest)) = queue.lock().unwrap().pop() else { break };
                let r = sgdb_cover(key, &names, &dest);
                let n = {
                    let mut d = done.lock().unwrap();
                    *d += 1;
                    *d
                };
                progress(n, total, &names[0]);
                results.lock().unwrap().push((si, gi, r));
            });
        }
    });
    for (si, gi, r) in Arc::try_unwrap(results).unwrap().into_inner().unwrap() {
        let label = format!("{} ({})", lib.systems[si].games[gi].title, lib.systems[si].short);
        match r {
            Ok(p) => {
                report.downloaded += 1;
                lib.systems[si].games[gi].cover = Some(p);
            }
            Err(e) if e.starts_with("not on") || e.starts_with("no portrait") => report.not_found.push(label),
            Err(e) => report.failed.push(format!("{label}: {e}")),
        }
    }
    report
}

#[derive(Debug, Default, Clone)]
pub struct CoverReport {
    pub already: usize,
    pub downloaded: usize,
    pub not_found: Vec<String>,
    pub failed: Vec<String>,
    pub no_source: Vec<String>,
}

/// Download missing covers. `progress` is called with (done, total, title).
pub fn fetch_covers(lib: &mut Library, media: &Path, progress: &(dyn Fn(usize, usize, &str) + Sync)) -> CoverReport {
    let agent = agent();
    let list_cache = media.join("lists");
    let mut report = CoverReport::default();

    // Work list: (system, game index, what to fetch, destination).
    enum Job {
        Art(ArtEntry),
        Steam(u32),
        SteamSearch(Vec<String>),
    }
    let mut jobs: Vec<(usize, usize, String, Job, PathBuf)> = Vec::new();

    for (si, s) in lib.systems.iter().enumerate() {
        let missing: Vec<usize> = (0..s.games.len()).filter(|&gi| existing_cover(media, &s.games[gi].id).is_none()).collect();
        report.already += s.games.len() - missing.len();
        if missing.is_empty() {
            continue;
        }
        if s.id == "pc" {
            for gi in missing {
                let g = &s.games[gi];
                let dest = cover_base(media, &g.id).with_extension("jpg");
                match steam_app_id(&g.path) {
                    Some(app) => jobs.push((si, gi, g.title.clone(), Job::Steam(app), dest)),
                    None => {
                        // Epic / GOG links and plain shortcuts: look the game up on Steam by name.
                        let mut names = vec![g.title.clone()];
                        if g.format == "lnk" {
                            if let Some(folder) = lnk_folder_name(&g.path) {
                                names.insert(0, folder);
                            }
                        }
                        jobs.push((si, gi, g.title.clone(), Job::SteamSearch(names), dest));
                    }
                }
            }
            continue;
        }
        let repos = libretro_repos(s.id);
        if repos.is_empty() {
            report.no_source.extend(missing.iter().map(|&gi| format!("{} ({})", s.games[gi].title, s.short)));
            continue;
        }
        let mut lists = Vec::new();
        for r in repos {
            match fetch_list(&agent, r, &list_cache) {
                Ok(l) => lists.push((*r, l)),
                Err(e) => report.failed.push(format!("{}: couldn't get the cover list ({e})", s.name)),
            }
        }
        let index = ArtIndex::new(lists);
        if index.is_empty() {
            continue;
        }
        for gi in missing {
            let g = &s.games[gi];
            match index.find(g) {
                Some(e) => jobs.push((si, gi, g.title.clone(), Job::Art(e.clone()), cover_base(media, &g.id).with_extension("png"))),
                None => report.not_found.push(format!("{} ({})", g.title, s.short)),
            }
        }
    }

    // Download in parallel.
    let total = jobs.len();
    let queue = Arc::new(Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>()));
    let results: Arc<Mutex<Vec<(usize, usize, Result<PathBuf, String>, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(0usize));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let done = Arc::clone(&done);
            let agent = agent.clone();
            scope.spawn(move || loop {
                let Some((_, (si, gi, title, job, dest))) = queue.lock().unwrap().pop() else { break };
                let r = match &job {
                    Job::Art(e) => download_art(&agent, e, &dest),
                    Job::Steam(app) => download_steam(&agent, *app, &dest),
                    Job::SteamSearch(names) => steam_search(&agent, names).and_then(|app| download_steam(&agent, app, &dest)),
                };
                let n = {
                    let mut d = done.lock().unwrap();
                    *d += 1;
                    *d
                };
                progress(n, total, &title);
                results.lock().unwrap().push((si, gi, r.map(|_| dest), title));
            });
        }
    });

    for (si, gi, r, title) in Arc::try_unwrap(results).unwrap().into_inner().unwrap() {
        let short = lib.systems[si].short;
        match r {
            Ok(p) => {
                report.downloaded += 1;
                lib.systems[si].games[gi].cover = Some(p);
            }
            Err(e) => report.failed.push(format!("{title} ({short}): {e}")),
        }
    }
    attach_existing(lib, media);
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names_and_ranks() {
        assert_eq!(libretro_safe("Mario Kart: Double Dash!!"), "Mario Kart_ Double Dash!!");
        assert_eq!(strip_rank("03. Pokemon - Emerald Version (USA, Europe)"), "Pokemon - Emerald Version (USA, Europe)");
        assert_eq!(strip_rank("05.The Legend of Zelda"), "The Legend of Zelda");
        assert_eq!(strip_rank("1080 TenEighty Snowboarding (Japan, USA) (En,Ja)"), "1080 TenEighty Snowboarding (Japan, USA) (En,Ja)");
        assert_eq!(strip_rank("007 - The World Is Not Enough (USA)"), "007 - The World Is Not Enough (USA)");
    }

    #[test]
    fn steam_search_picking() {
        let json = br#"{"total":3,"items":[
            {"type":"app","name":"Nexomon: Extinction","id":1196630},
            {"type":"app","name":"Nexomon","id":1252930},
            {"type":"app","name":"LEGO\u00ae City Undercover","id":578330}]}"#;
        assert_eq!(pick_steam_result(json, &["Nexomon".into()]), Some(1252930));
        assert_eq!(pick_steam_result(json, &["Nexomon Extinction".into()]), Some(1196630));
        assert_eq!(pick_steam_result(json, &["Lego City Undercover".into()]), Some(578330));
        assert_eq!(pick_steam_result(json, &["Balls".into()]), None);
    }

    #[test]
    fn steam_trailer_picking() {
        let j = br#"{"413150":{"success":true,"data":{"movies":[{"id":1,"name":"Teaser","webm":{"480":"http://a/w480.webm"},"highlight":false},{"id":2,"name":"Launch","mp4":{"480":"http://a/m480.mp4","max":"http://a/max.mp4"},"highlight":true}]}}}"#;
        assert_eq!(pick_steam_trailer(j, 413150).as_deref(), Some("https://a/m480.mp4"));
        assert_eq!(pick_steam_trailer(br#"{"1":{"success":true,"data":{}}}"#, 1), None);
        // What Steam actually returns now (October 2026): HLS/DASH only.
        let j = br#"{"413150":{"success":true,"data":{"movies":[{"id":256815967,"dash_h264":"https://v/x/dash_h264.mpd?t=1","hls_h264":"https://v/x/hls_264_master.m3u8?t=1","highlight":true}]}}}"#;
        assert_eq!(pick_steam_trailer(j, 413150).as_deref(), Some("https://v/x/hls_264_master.m3u8?t=1"));
        let master = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"Default\",AUTOSELECT=YES,DEFAULT=YES,URI=\"hls_264_4_audio.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=5800000,RESOLUTION=1920x1080,AUDIO=\"audio\"\nhls_264_0_video.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1400000,RESOLUTION=854x480,AUDIO=\"audio\"\nhls_264_2_video.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=640x360\nhls_264_3_video.m3u8\n";
        let (v, a) = hls_pick_streams("https://v/x/hls_264_master.m3u8?t=1", master).unwrap();
        assert_eq!(v, "https://v/x/hls_264_2_video.m3u8");
        assert_eq!(a.as_deref(), Some("https://v/x/hls_264_4_audio.m3u8"));
        let media = "#EXTM3U\n#EXT-X-MAP:URI=\"dash_h264/init-stream2.m4s\"\n#EXTINF:3\ndash_h264/chunk-stream2-00001.m4s\n#EXTINF:3\ndash_h264/chunk-stream2-00002.m4s\n#EXTINF:3\ndash_h264/chunk-stream2-00003.m4s\n";
        let (init, segs) = hls_segments("https://v/x/hls_264_2_video.m3u8", media, 2);
        assert_eq!(init.as_deref(), Some("https://v/x/dash_h264/init-stream2.m4s"));
        assert_eq!(segs, vec!["https://v/x/dash_h264/chunk-stream2-00001.m4s", "https://v/x/dash_h264/chunk-stream2-00002.m4s"]);
        assert_eq!(video_ext(b"\0\0\0\x20ftypisom\0\0\0\0"), Some("mp4"));
    }

    #[test]
    fn steamgriddb_picking() {
        let search = br#"{"success":true,"data":[{"id":5254,"name":"The Legend of Zelda: Breath of the Wild"},{"id":38000,"name":"The Legend of Zelda: Tears of the Kingdom","release_date":1683849600}]}"#;
        assert_eq!(pick_sgdb_game(search, &["The Legend of Zelda: Tears of the Kingdom".into()]), Some(38000));
        assert_eq!(pick_sgdb_game(search, &["Shantae".into()]), None);
        let grids = br#"{"success":true,"data":[{"id":1,"url":"https://cdn2.steamgriddb.com/grid/a.png","width":600,"height":900}]}"#;
        assert_eq!(pick_sgdb_grid(grids).as_deref(), Some("https://cdn2.steamgriddb.com/grid/a.png"));
    }
}

// ---------------------------------------------------------------------------
// "Wrong cover?": let the user pick the right one
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct CoverChoice {
    /// "libretro", "Steam" or "SteamGridDB".
    pub source: String,
    /// The name the source gives it.
    pub label: String,
    /// Full image.
    pub url: String,
    /// Small preview (may be the same as `url`).
    pub thumb: String,
}

/// Names from a box-art list that best match what the user typed, best first.
pub fn rank_names<'a>(names: &'a [String], query: &str, max: usize) -> Vec<&'a String> {
    let want = words_of(query);
    if want.is_empty() {
        return vec![];
    }
    let mut scored: Vec<(usize, usize, &String)> = names
        .iter()
        .filter_map(|n| {
            let parsed = title::parse_name(n, false);
            let have = words_of(&parsed.title);
            let hits = want.iter().filter(|w| have.contains(w)).count();
            // Every word typed must be there (allowing one miss on long queries).
            let need = if want.len() >= 4 { want.len() - 1 } else { want.len() };
            (hits >= need).then_some((hits, have.len().abs_diff(want.len()), n))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.len().cmp(&b.2.len())));
    scored.into_iter().map(|(_, _, n)| n).take(max).collect()
}

/// Search every source for covers matching `query` (usually the game's name).
pub fn cover_choices(system: &str, query: &str, media: &Path, sgdb_key: Option<&str>) -> Vec<CoverChoice> {
    let agent = agent();
    let mut out = Vec::new();
    // libretro (retro consoles): exact No-Intro names, so the user can pick the right region / version.
    let cache = media.join("lists");
    for repo in libretro_repos(system) {
        let Ok(names) = fetch_list(&agent, repo, &cache) else { continue };
        for n in rank_names(&names, query, 8) {
            let file = format!("{n}.png");
            let url = format!("https://raw.githubusercontent.com/libretro-thumbnails/{repo}/master/Named_Boxarts/{}", utf8_percent_encode(&file, PATH_SEG));
            out.push(CoverChoice { source: "libretro".into(), label: n.clone(), thumb: url.clone(), url });
        }
    }
    // Steam (PC games, and plenty of ports).
    if system == "pc" {
        let url = format!("https://store.steampowered.com/api/storesearch/?cc=us&l=english&term={}", utf8_percent_encode(query, PATH_SEG));
        if let Ok(json) = get_bytes(&agent, &url) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&json) {
                for it in v["items"].as_array().into_iter().flatten().take(5) {
                    let (Some(id), Some(name)) = (it["id"].as_u64(), it["name"].as_str()) else { continue };
                    let url = format!("https://shared.steamstatic.com/store_item_assets/steam/apps/{id}/library_600x900.jpg");
                    out.push(CoverChoice { source: "Steam".into(), label: name.to_string(), thumb: url.clone(), url });
                }
            }
        }
    }
    // SteamGridDB (anything, with a free key).
    if let Some(key) = sgdb_key {
        let auth = vec![format!("Authorization: Bearer {key}")];
        let url = format!("{SGDB}/search/autocomplete/{}", utf8_percent_encode(query, PATH_SEG));
        if let Ok(body) = curl(&url, true, &auth) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) {
                for g in v["data"].as_array().into_iter().flatten().take(3) {
                    let (Some(id), Some(name)) = (g["id"].as_u64(), g["name"].as_str()) else { continue };
                    let url = format!("{SGDB}/grids/game/{id}?dimensions=600x900,342x482,660x930&types=static&nsfw=false&humor=false");
                    let Ok(grids) = curl(&url, true, &auth) else { continue };
                    let Ok(gv) = serde_json::from_slice::<serde_json::Value>(&grids) else { continue };
                    for gr in gv["data"].as_array().into_iter().flatten().take(4) {
                        let Some(full) = gr["url"].as_str() else { continue };
                        let thumb = gr["thumb"].as_str().unwrap_or(full);
                        out.push(CoverChoice { source: "SteamGridDB".into(), label: name.to_string(), url: full.to_string(), thumb: thumb.to_string() });
                    }
                }
            }
        }
    }
    out
}

/// Image hosts a picked cover may come from.
pub fn allowed_cover_url(url: &str) -> bool {
    ["https://raw.githubusercontent.com/libretro-thumbnails/", "https://thumbnails.libretro.com/", "https://cdn2.steamgriddb.com/", "https://shared.steamstatic.com/", "https://cdn.cloudflare.steamstatic.com/", "https://steamcdn-a.akamaihd.net/"]
        .iter()
        .any(|p| url.starts_with(p))
}

/// Save the picked cover as the game's own cover.
pub fn use_cover_url(media: &Path, game_id: &str, url: &str) -> Result<PathBuf, String> {
    if !allowed_cover_url(url) {
        return Err("That image isn't from a cover source Spine knows.".into());
    }
    let agent = agent();
    let b = get_bytes(&agent, url)?;
    let ext = is_image(&b).ok_or("That link didn't give an image.")?;
    let base = custom_base(media, game_id);
    for e in CUSTOM_EXTS {
        let _ = fs::remove_file(base.with_extension(e));
    }
    let dest = base.with_extension(ext);
    save(&dest, &b)?;
    Ok(dest)
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    #[test]
    fn ranks_names_for_a_typed_query() {
        let names: Vec<String> = ["Pokemon - Emerald Version (USA, Europe)", "Pokemon - Ruby Version (USA)", "Pokemon Mystery Dungeon - Red Rescue Team (USA)", "Emerald Hill (Japan)", "Pokemon - Emerald (Japan)"].iter().map(|s| s.to_string()).collect();
        let r = rank_names(&names, "Pokemon Emerald", 5);
        assert_eq!(r.len(), 2);
        assert!(r[0].starts_with("Pokemon - Emerald"));
        assert!(rank_names(&names, "", 5).is_empty());
        assert!(allowed_cover_url("https://cdn2.steamgriddb.com/grid/abc.png"));
        assert!(!allowed_cover_url("https://evil.example/x.png"));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod builtin_net {
    /// cargo test --lib builtin_net -- --ignored
    #[test]
    #[ignore]
    fn downloads_without_curl() {
        let b = super::builtin_get("https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Game_Boy_Advance/master/Named_Snaps/Golden%20Sun%20%28USA%2C%20Europe%29.png", true, &[]).unwrap();
        assert!(b.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(super::builtin_get("https://raw.githubusercontent.com/libretro-thumbnails/nope/master/x.png", true, &[]).unwrap_err().contains("404"));
    }
}
