//! Walking a ROM folder and turning it into a clean library.

use crate::systems::{self, System};
use crate::title::{self, ContentKind, ParsedName};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Library {
    pub root: PathBuf,
    pub systems: Vec<SystemLibrary>,
    /// Things worth telling the user about (not errors).
    pub notes: Vec<String>,
    pub game_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemLibrary {
    pub id: &'static str,
    pub name: &'static str,
    pub short: &'static str,
    pub color: &'static str,
    /// Folders (relative to the library root) that fed this system.
    pub folders: Vec<String>,
    pub games: Vec<Game>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    /// Stable id: "<system>/<slug>".
    pub id: String,
    pub title: String,
    pub sort_key: String,
    pub letter: char,
    /// The file Spine will launch.
    pub path: PathBuf,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_id: Option<String>,
    /// Every copy found (other formats, other discs). The first is `path`.
    pub versions: Vec<Version>,
    pub updates: usize,
    pub dlc: usize,
    /// Downloaded box art, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<PathBuf>,
    /// A gameplay video or trailer, if one was found or downloaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<PathBuf>,
    /// Genres, tags, franchise, developer, year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<crate::meta::GameMeta>,
    /// Details from ScreenScraper (name, description, year...), if looked up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<crate::screenscraper::GameInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub path: PathBuf,
    pub format: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc: Option<u32>,
}

/// One file (or extracted folder) that looks like a game, before grouping.
#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    ext: String,
    parsed: ParsedName,
    size: u64,
    /// Found inside an "update"/"dlc" folder, or tagged as one.
    extra: Option<ContentKind>,
    warnings: Vec<String>,
}

/// Folder names that never hold games (artwork, saves, emulator data).
const SKIP_DIRS: &[&str] = &[
    "media", "images", "image", "videos", "video", "manuals", "boxart", "covers", "snap", "snaps",
    "screenshots", "saves", "save", "states", "savestates", "cheats", "bios", "downloaded_images",
    "downloaded_media", "gamelists", "themes", "$recycle.bin", "system volume information",
];
const EXTRA_DIRS_UPDATE: &[&str] = &["update", "updates", "patch", "patches"];
const EXTRA_DIRS_DLC: &[&str] = &["dlc", "dlcs", "addon", "addons", "add-ons"];
const IGNORE_FILES: &[&str] = &["systeminfo.txt", "systems.txt", "desktop.ini", "thumbs.db", ".ds_store"];
const ARCHIVES: &[&str] = &["rar", "zip", "7z"];
const MAX_DEPTH: usize = 4;

pub fn scan(root: &Path) -> std::io::Result<Library> {
    let mut by_system: BTreeMap<&'static str, (Vec<String>, Vec<Candidate>)> = BTreeMap::new();
    let mut notes = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(root)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_dir() {
            continue;
        }
        if SKIP_DIRS.contains(&name.to_lowercase().as_str()) || name.starts_with('.') {
            continue;
        }
        match systems::system_for_folder(&name) {
            Some(sys) => {
                let mut found = Vec::new();
                walk_system(sys, &path, 0, None, &mut found, &mut notes);
                let slot = by_system.entry(sys.id).or_default();
                slot.0.push(name.clone());
                slot.1.extend(found);
            }
            None => {
                if folder_has_files(&path) {
                    notes.push(format!("Skipped folder \"{name}\": couldn't tell which console it's for. Rename it or pick the console in settings."));
                }
            }
        }
    }

    let mut systems_out = Vec::new();
    let mut game_count = 0;
    for (id, (folders, cands)) in by_system {
        let sys = systems::system_by_id(id).unwrap();
        let games = build_games(sys, cands, &mut notes);
        if games.is_empty() {
            continue; // ES-DE placeholder folders: hide empty systems.
        }
        game_count += games.len();
        systems_out.push(SystemLibrary { id: sys.id, name: sys.name, short: sys.short, color: sys.color, folders, games });
    }
    systems_out.sort_by_key(|s| systems::SYSTEMS.iter().position(|x| x.id == s.id).unwrap_or(usize::MAX));

    Ok(Library { root: root.to_path_buf(), systems: systems_out, notes, game_count })
}

fn folder_has_files(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                e.path().is_dir() || !(IGNORE_FILES.contains(&n.as_str()) || n.ends_with(".txt"))
            })
        })
        .unwrap_or(false)
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn walk_system(sys: &System, dir: &Path, depth: usize, extra: Option<ContentKind>, out: &mut Vec<Candidate>, notes: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    // Wii U extracted title: a folder with code/*.rpx is one game, not a tree to explore.
    if sys.id == "wiiu" && depth > 0 {
        if let Some(rpx) = find_rpx(dir) {
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            let mut parsed = title::parse_name(&name, false);
            if let Some(long) = read_wiiu_longname(dir) {
                parsed.title = long;
            }
            let kind = if parsed.kind != ContentKind::Game { Some(parsed.kind) } else { extra };
            out.push(Candidate { path: rpx, ext: "rpx".into(), parsed, size: 0, extra: kind, warnings: vec![] });
            return;
        }
        if dir.join("meta").join("meta.xml").exists() {
            // Extracted update / DLC folders have meta but no runnable code.
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            let parsed = title::parse_name(&name, false);
            let kind = if parsed.kind != ContentKind::Game { parsed.kind } else { ContentKind::Update };
            out.push(Candidate { path: dir.to_path_buf(), ext: "folder".into(), parsed, size: 0, extra: Some(kind), warnings: vec![] });
            return;
        }
    }

    // Files referenced by a .cue/.gdi/.m3u in this folder are parts, not games.
    let mut referenced: HashSet<String> = HashSet::new();
    for e in &entries {
        let p = e.path();
        let ext = ext_of(&p);
        if matches!(ext.as_str(), "cue" | "gdi" | "m3u" | "ccd") && sys.exts.contains(&ext.as_str()) {
            let refs = referenced_files(&p);
            if refs.is_empty() && ext != "m3u" {
                // Unreadable or empty sheet: fall back to same-name track files.
                let stem = p.file_stem().unwrap().to_string_lossy().to_lowercase();
                for other in &entries {
                    let op = other.path();
                    let oe = ext_of(&op);
                    if matches!(oe.as_str(), "bin" | "img" | "sub" | "raw") && op.file_stem().map(|s| s.to_string_lossy().to_lowercase()) == Some(stem.clone()) {
                        referenced.insert(other.file_name().to_string_lossy().to_lowercase());
                    }
                }
            }
            referenced.extend(refs);
        }
    }

    // Vita: folders holding .pkg installers often carry the nicer name ("gravity rush").
    let pkg_dir_titles: Vec<String> = if sys.id == "psvita" {
        entries.iter().filter(|e| e.path().is_dir() && dir_contains_ext(&e.path(), "pkg"))
            .map(|e| title::parse_name(&e.file_name().to_string_lossy(), false).title).collect()
    } else { vec![] };

    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let lname = name.to_lowercase();
        if name.starts_with('.') || name.starts_with("._") {
            continue;
        }
        if p.is_dir() {
            if SKIP_DIRS.contains(&lname.as_str()) {
                continue;
            }
            if sys.id == "psvita" && dir_contains_ext(&p, "pkg") {
                continue; // Vita .pkg installers: the .psvita file next to them is the game.
            }
            let child_extra = if EXTRA_DIRS_UPDATE.contains(&lname.as_str()) {
                Some(ContentKind::Update)
            } else if EXTRA_DIRS_DLC.contains(&lname.as_str()) {
                Some(ContentKind::Dlc)
            } else if lname == "downloaded" {
                continue;
            } else {
                extra
            };
            walk_system(sys, &p, depth + 1, child_extra, out, notes);
            continue;
        }
        if IGNORE_FILES.contains(&lname.as_str()) || referenced.contains(&lname) {
            continue;
        }
        let ext = ext_of(&p);
        if !sys.exts.contains(&ext.as_str()) {
            if ARCHIVES.contains(&ext.as_str()) {
                notes.push(format!("{}: \"{}\" is a compressed archive this console can't run directly. Extract it.", sys.name, name));
            }
            continue;
        }
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        let mut parsed = title::parse_name(&name, true);
        let mut warnings = Vec::new();

        if sys.id == "psvita" {
            let key = title::match_key(&parsed.title);
            if let Some(better) = pkg_dir_titles.iter().find(|t| title::match_key(t) == key && t.contains(' ')) {
                parsed.title = better.clone();
            }
            match read_small(&p).map(|s| s.trim().to_uppercase()) {
                Some(id) if is_vita_id(&id) => parsed.title_id = Some(id),
                _ => warnings.push("Vita3K needs the game's title id (like PCSA00134) inside this .psvita file. Install the .pkg in Vita3K once, then Spine can fill it in.".into()),
            }
        }
        if ext == "bin" && matches!(sys.id, "psx" | "ps2" | "segacd" | "saturn" | "pcenginecd") {
            warnings.push("Disc image without a .cue sheet. It may not boot; a .cue or .chd version works best.".into());
        }

        let kind = if parsed.kind != ContentKind::Game { Some(parsed.kind) } else { extra };
        out.push(Candidate { path: p, ext, parsed, size, extra: kind, warnings });
    }
}

fn find_rpx(dir: &Path) -> Option<PathBuf> {
    let code = dir.join("code");
    fs::read_dir(code).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| ext_of(p) == "rpx")
}

fn read_wiiu_longname(dir: &Path) -> Option<String> {
    let xml = fs::read_to_string(dir.join("meta").join("meta.xml")).ok()?;
    let start = xml.find("<longname_en")?;
    let open_end = xml[start..].find('>')? + start + 1;
    let close = xml[open_end..].find("</longname_en>")? + open_end;
    let name = xml[open_end..close].replace(['\n', '\r'], " ");
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(name)
}

fn dir_contains_ext(dir: &Path, ext: &str) -> bool {
    fn inner(d: &Path, ext: &str, depth: usize) -> bool {
        if depth > 2 {
            return false;
        }
        fs::read_dir(d)
            .map(|rd| rd.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                if p.is_dir() { inner(&p, ext, depth + 1) } else { ext_of(&p) == ext }
            }))
            .unwrap_or(false)
    }
    inner(dir, ext, 0)
}

fn read_small(p: &Path) -> Option<String> {
    let meta = fs::metadata(p).ok()?;
    if meta.len() > 4096 {
        return None;
    }
    fs::read_to_string(p).ok()
}

fn is_vita_id(s: &str) -> bool {
    s.len() == 9 && s[..4].chars().all(|c| c.is_ascii_uppercase()) && s[4..].chars().all(|c| c.is_ascii_digit())
}

/// File names (lower case) referenced by a cue / gdi / m3u sheet.
fn referenced_files(sheet: &Path) -> Vec<String> {
    let Some(text) = read_small(sheet).or_else(|| fs::read_to_string(sheet).ok()) else { return vec![] };
    let ext = ext_of(sheet);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let name = match ext.as_str() {
            "cue" => line.strip_prefix("FILE ").and_then(|rest| {
                let rest = rest.trim();
                if let Some(r) = rest.strip_prefix('"') { r.split('"').next() } else { rest.split_whitespace().next() }
            }),
            "gdi" => line.split('"').nth(1).or_else(|| line.split_whitespace().nth(4)),
            "m3u" => (!line.is_empty() && !line.starts_with('#')).then_some(line),
            _ => None,
        };
        if let Some(n) = name {
            let n = n.replace('\\', "/");
            out.push(n.rsplit('/').next().unwrap_or(&n).to_lowercase());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Grouping candidates into games
// ---------------------------------------------------------------------------

fn region_rank(r: &Option<String>) -> u8 {
    let r = r.as_deref().unwrap_or("").to_lowercase();
    if r.contains("usa") || r == "us" || r.contains("world") { 0 }
    else if r.is_empty() { 1 }
    else if r.contains("europe") || r == "eu" || r.contains("australia") { 2 }
    else { 3 }
}

fn format_rank(sys: &System, ext: &str) -> usize {
    sys.exts.iter().position(|e| *e == ext).unwrap_or(sys.exts.len())
}

/// The base title id a Switch / Wii U update or DLC belongs to.
fn base_title_id(sys: &System, id: &str) -> Option<String> {
    let v = u64::from_str_radix(id, 16).ok()?;
    match sys.id {
        "switch" if id.len() == 16 => Some(format!("{:016X}", v & !0x1FFF)),
        "wiiu" => Some(format!("{:08X}", v & 0xFFFF_FFFF)), // low half is shared by game/update/dlc
        _ => None,
    }
}

fn build_games(sys: &System, cands: Vec<Candidate>, notes: &mut Vec<String>) -> Vec<Game> {
    let (extras, mut games_c): (Vec<_>, Vec<_>) = cands.into_iter().partition(|c| c.extra.is_some());

    // Multi-disc without .m3u: "(Disc 1)", "(Disc 2)" share a game.
    // Group 1: exact title match (any format).
    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    games_c.sort_by(|a, b| a.path.cmp(&b.path));
    for c in games_c {
        let key = title::match_key(&c.parsed.title);
        if key.is_empty() {
            continue;
        }
        if let Some(&i) = index.get(&key) {
            groups[i].push(c);
        } else {
            index.insert(key, groups.len());
            groups.push(vec![c]);
        }
    }

    // Group 2: the same game dumped in two *different* formats under slightly
    // different names ("The Wind Waker HD (US).wua" vs "Legend of Zelda, The -
    // The Wind Waker HD (USA).wux"). Merge when one name contains the other
    // and the leftover isn't just a number ("Bayonetta" vs "Bayonetta 2").
    // Two files of the same format are never merged this way: those are
    // usually genuinely different games (NSMBU vs NSMBU + Luigi U).
    // Only disc-based systems get re-packed into other formats (iso -> chd,
    // wux -> wua). On cartridge systems different names are different games
    // ("Bubble Bobble" vs "Classic Bubble Bobble").
    let fuzzy = matches!(sys.id, "wiiu" | "wii" | "gc" | "ps2" | "psx" | "psp" | "switch" | "n3ds" | "ps3" | "segacd" | "saturn" | "dreamcast");
    let mut merged = fuzzy;
    while merged {
        merged = false;
        'outer: for i in 0..groups.len() {
            for j in 0..groups.len() {
                if i == j {
                    continue;
                }
                let (ki, kj) = (title::match_key(&groups[i][0].parsed.title), title::match_key(&groups[j][0].parsed.title));
                if ki.len() >= kj.len() || kj.len() < 8 {
                    continue;
                }
                // i is the shorter name.
                let leftover = if kj.starts_with(&ki) { &kj[ki.len()..] } else if kj.ends_with(&ki) { &kj[..kj.len() - ki.len()] } else { continue };
                if leftover.len() < 4 || leftover.chars().all(|c| c.is_ascii_digit()) || is_roman(leftover) {
                    continue;
                }
                let fi: HashSet<&str> = groups[i].iter().map(|c| c.ext.as_str()).collect();
                let fj: HashSet<&str> = groups[j].iter().map(|c| c.ext.as_str()).collect();
                if !fi.is_disjoint(&fj) {
                    continue;
                }
                let moved = groups.remove(i);
                let j = if j > i { j - 1 } else { j };
                groups[j].extend(moved);
                merged = true;
                break 'outer;
            }
        }
    }

    // Attach updates / DLC by title id where possible.
    let mut extra_counts: HashMap<String, (usize, usize)> = HashMap::new();
    let mut orphan_extras = 0;
    for x in &extras {
        let base = x.parsed.title_id.as_deref().and_then(|id| base_title_id(sys, id));
        let key = match base {
            Some(b) => format!("id:{b}"),
            None => format!("name:{}", title::match_key(&x.parsed.title)),
        };
        let slot = extra_counts.entry(key).or_default();
        match x.extra {
            Some(ContentKind::Dlc) => slot.1 += 1,
            _ => slot.0 += 1,
        }
    }

    let mut games: Vec<Game> = groups
        .into_iter()
        .map(|mut g| {
            g.sort_by(|a, b| {
                (a.parsed.disc.unwrap_or(1), a.parsed.prerelease, format_rank(sys, &a.ext), region_rank(&a.parsed.region))
                    .cmp(&(b.parsed.disc.unwrap_or(1), b.parsed.prerelease, format_rank(sys, &b.ext), region_rank(&b.parsed.region)))
            });
            let primary = g[0].clone();
            let best_title = g.iter().map(|c| c.parsed.title.clone()).max_by_key(|t| t.chars().count()).unwrap();
            let title_id = g.iter().find_map(|c| c.parsed.title_id.clone());
            let sort_key = title::sort_key(&best_title);
            let mut warnings: Vec<String> = g.iter().flat_map(|c| c.warnings.clone()).collect();
            warnings.dedup();
            let (mut updates, mut dlc) = (0, 0);
            let mut keys = vec![format!("name:{}", title::match_key(&best_title))];
            keys.extend(g.iter().map(|c| format!("name:{}", title::match_key(&c.parsed.title))));
            if let Some(b) = title_id.as_deref().and_then(|id| base_title_id(sys, id)) {
                keys.push(format!("id:{b}"));
            }
            keys.sort();
            keys.dedup();
            for k in keys {
                if let Some((u, d)) = extra_counts.remove(&k) {
                    updates += u;
                    dlc += d;
                }
            }
            Game {
                id: format!("{}/{}", sys.id, slug(&sort_key)),
                letter: title::index_letter(&sort_key),
                title: best_title,
                sort_key,
                path: primary.path.clone(),
                format: primary.ext.clone(),
                region: primary.parsed.region.clone(),
                revision: primary.parsed.revision.clone(),
                title_id,
                versions: g.iter().map(|c| Version { path: c.path.clone(), format: c.ext.clone(), size: c.size, disc: c.parsed.disc }).collect(),
                updates,
                dlc,
                cover: None,
                video: None,
                meta: None,
                info: None,
                warnings,
            }
        })
        .collect();

    orphan_extras += extra_counts.values().map(|(u, d)| u + d).sum::<usize>();
    if orphan_extras > 0 {
        notes.push(format!("{}: {} update/DLC file(s) didn't match any game in the library.", sys.name, orphan_extras));
    }

    games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    // Make ids unique if two games slug the same.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for g in &mut games {
        let n = seen.entry(g.id.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            g.id = format!("{}-{}", g.id, n);
        }
    }
    games
}

fn is_roman(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| matches!(c, 'i' | 'v' | 'x'))
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Several ROM folders, some of them on drives that come and go
// ---------------------------------------------------------------------------

/// What one ROM folder held last time it was seen (for when its drive is unplugged).
#[derive(Serialize, Deserialize)]
struct RootCache {
    systems: Vec<(String, Vec<Game>)>,
}

fn cache_file(cache: &Path, root: &Path) -> PathBuf {
    cache.join(format!("{}.json", slug(&root.to_string_lossy().to_lowercase()).trim_start_matches('-')))
}

/// Add `other`'s games to `lib`; a game id already present is kept as it is.
pub fn merge_library(lib: &mut Library, other: Library) {
    for s in other.systems {
        match lib.systems.iter_mut().find(|x| x.id == s.id) {
            Some(x) => {
                for f in s.folders {
                    if !x.folders.contains(&f) {
                        x.folders.push(f);
                    }
                }
                for g in s.games {
                    if !x.games.iter().any(|e| e.id == g.id) {
                        x.games.push(g);
                    }
                }
                x.games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
            }
            None => lib.systems.push(s),
        }
    }
    lib.notes.extend(other.notes);
    lib.systems.sort_by_key(|s| systems::SYSTEMS.iter().position(|x| x.id == s.id).unwrap_or(usize::MAX));
    lib.game_count = lib.systems.iter().map(|s| s.games.len()).sum();
}

/// Scan every ROM folder. A folder that can't be read right now (its drive is
/// unplugged) contributes what it held last time, and is listed as offline.
pub fn scan_many(roots: &[PathBuf], cache: &Path) -> (Library, Vec<PathBuf>) {
    let mut lib = Library { root: roots.first().cloned().unwrap_or_default(), systems: vec![], notes: vec![], game_count: 0 };
    let mut offline = Vec::new();
    for root in roots {
        let found = if root.is_dir() { scan(root).ok() } else { None };
        match found {
            Some(l) => {
                let c = RootCache { systems: l.systems.iter().map(|s| (s.id.to_string(), s.games.clone())).collect() };
                let _ = fs::create_dir_all(cache);
                let _ = fs::write(cache_file(cache, root), serde_json::to_string(&c).unwrap_or_default());
                merge_library(&mut lib, l);
            }
            None => {
                offline.push(root.clone());
                let Some(c) = fs::read_to_string(cache_file(cache, root)).ok().and_then(|t| serde_json::from_str::<RootCache>(&t).ok()) else { continue };
                let systems_out = c
                    .systems
                    .into_iter()
                    .filter_map(|(id, games)| {
                        let sys = systems::SYSTEMS.iter().find(|s| s.id == id)?;
                        Some(SystemLibrary { id: sys.id, name: sys.name, short: sys.short, color: sys.color, folders: vec![], games })
                    })
                    .collect();
                merge_library(&mut lib, Library { root: root.clone(), systems: systems_out, notes: vec![], game_count: 0 });
            }
        }
    }
    (lib, offline)
}

#[cfg(test)]
mod many_tests {
    use super::*;

    #[test]
    fn unplugged_folder_keeps_its_games() {
        let base = std::env::temp_dir().join(format!("spine-many-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let a = base.join("a");
        let b = base.join("b");
        fs::create_dir_all(a.join("gba")).unwrap();
        fs::create_dir_all(b.join("snes")).unwrap();
        fs::write(a.join("gba").join("Golden Sun (USA).gba"), b"x").unwrap();
        fs::write(b.join("snes").join("Super Metroid (USA).sfc"), b"x").unwrap();
        fs::write(b.join("snes").join("Golden Sun (USA).sfc"), b"x").unwrap();
        let cache = base.join("cache");
        let roots = vec![a.clone(), b.clone()];
        let (lib, off) = scan_many(&roots, &cache);
        assert!(off.is_empty());
        assert_eq!(lib.game_count, 3);
        // Unplug b: its games are still listed, from the cache.
        fs::remove_dir_all(&b).unwrap();
        let (lib, off) = scan_many(&roots, &cache);
        assert_eq!(off, vec![b.clone()]);
        assert_eq!(lib.game_count, 3);
        assert!(lib.systems.iter().any(|s| s.id == "snes"));
        let _ = fs::remove_dir_all(&base);
    }
}
