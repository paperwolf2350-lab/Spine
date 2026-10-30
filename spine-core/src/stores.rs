//! Games already installed through PC stores (Steam, Epic Games, GOG), found
//! without any setup: Spine reads the files those launchers keep on disk.
//!
//!   * Steam: `libraryfolders.vdf` lists every library folder (other drives too),
//!     and each `steamapps/appmanifest_<id>.acf` is one installed game.
//!   * Epic: one JSON `.item` manifest per install under ProgramData.
//!   * GOG: the GOG.com\Games registry key (Galaxy and offline installers both write it).
//!
//! Steam and Epic games start through their launcher (`steam://`, `com.epicgames.launcher://`),
//! GOG games start their own .exe.

use crate::scan::{Game, Library, SystemLibrary, Version};
use crate::{systems, title};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct StoreGame {
    /// "steam", "epic" or "gog".
    pub store: &'static str,
    pub name: String,
    /// What starts it: a launcher URI or an .exe path.
    pub launch: String,
    pub install_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Steam
// ---------------------------------------------------------------------------

/// Steam's own tools that show up as "installed apps" but aren't games.
const STEAM_NOT_GAMES: &[u32] = &[228980, 1070560, 1391110, 1628350, 1493710, 2180100, 250820, 1826330];

fn steam_name_is_tool(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("proton") || n.starts_with("steam linux runtime") || n.contains("redistributable") || n.starts_with("steamvr") || n.contains("dedicated server") || n.ends_with(" sdk")
}

/// Tokens of a Valve KeyValues file: quoted strings and braces.
fn vdf_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            if let Some(n) = chars.next() {
                                s.push(n);
                            }
                        }
                        '"' => break,
                        _ => s.push(c),
                    }
                }
                out.push(s);
            }
            '{' => out.push("{".into()),
            '}' => out.push("}".into()),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// `key value` pairs one level inside the outer block (e.g. AppState's fields).
fn vdf_top_fields(text: &str) -> Vec<(String, String)> {
    let t = vdf_tokens(text);
    let mut out = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    while i < t.len() {
        match t[i].as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            k if depth == 1 && i + 1 < t.len() && t[i + 1] != "{" && t[i + 1] != "}" => {
                out.push((k.to_string(), t[i + 1].clone()));
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Every library folder listed in libraryfolders.vdf.
pub fn parse_library_folders(text: &str) -> Vec<PathBuf> {
    let t = vdf_tokens(text);
    t.windows(2).filter(|w| w[0].eq_ignore_ascii_case("path") && w[1] != "{").map(|w| PathBuf::from(&w[1])).collect()
}

/// One installed game from an appmanifest, or None for tools and half-done downloads.
pub fn parse_app_manifest(text: &str) -> Option<StoreGame> {
    let f = vdf_top_fields(text);
    let get = |k: &str| f.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone());
    let appid: u32 = get("appid")?.parse().ok()?;
    let name = get("name").filter(|n| !n.trim().is_empty())?;
    if STEAM_NOT_GAMES.contains(&appid) || steam_name_is_tool(&name) {
        return None;
    }
    // StateFlags bit 4 = fully installed.
    let flags: u32 = get("StateFlags").and_then(|s| s.parse().ok()).unwrap_or(0);
    if flags & 4 == 0 {
        return None;
    }
    Some(StoreGame { store: "steam", name: tidy_name(&name), launch: format!("steam://rungameid/{appid}"), install_dir: get("installdir").map(PathBuf::from) })
}

/// Where Steam is installed (registry first, then the usual place).
pub fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(out) = reg_query(&["HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"]) {
            if let Some(p) = parse_reg_values(&out).into_iter().find(|(k, _)| k.eq_ignore_ascii_case("SteamPath")).map(|(_, v)| PathBuf::from(v.replace('/', "\\"))) {
                if p.exists() {
                    return Some(p);
                }
            }
        }
        let d = PathBuf::from(r"C:\Program Files (x86)\Steam");
        return d.exists().then_some(d);
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        [".local/share/Steam", ".steam/steam"].iter().map(|p| home.join(p)).find(|p| p.exists())
    }
}

pub fn steam_games(root: &Path) -> Vec<StoreGame> {
    let steamapps = root.join("steamapps");
    let mut libs = fs::read_to_string(steamapps.join("libraryfolders.vdf")).map(|t| parse_library_folders(&t)).unwrap_or_default();
    if !libs.iter().any(|l| same_path(l, root)) {
        libs.insert(0, root.to_path_buf());
    }
    let mut out: Vec<StoreGame> = Vec::new();
    for lib in libs {
        let apps = lib.join("steamapps");
        let Ok(rd) = fs::read_dir(&apps) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(e.path()) else { continue };
            if let Some(mut g) = parse_app_manifest(&text) {
                g.install_dir = g.install_dir.map(|d| apps.join("common").join(d));
                if !out.iter().any(|o| o.launch == g.launch) {
                    out.push(g);
                }
            }
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    n(a) == n(b)
}

// ---------------------------------------------------------------------------
// Epic Games
// ---------------------------------------------------------------------------

pub fn parse_epic_item(json: &[u8]) -> Option<StoreGame> {
    let json = json.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(json);
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let is_game = v["AppCategories"].as_array().map_or(true, |a| a.iter().any(|c| c.as_str() == Some("games")));
    // DLC and add-ons point at their main game; skip those and broken installs.
    if !is_game || v["bIsIncompleteInstall"].as_bool() == Some(true) || !s("MainGameAppName").is_empty() && s("MainGameAppName") != s("AppName") {
        return None;
    }
    let name = s("DisplayName");
    let (ns, item, app) = (s("CatalogNamespace"), s("CatalogItemId"), s("AppName"));
    if name.trim().is_empty() || app.is_empty() {
        return None;
    }
    let launch = if ns.is_empty() || item.is_empty() {
        format!("com.epicgames.launcher://apps/{app}?action=launch&silent=true")
    } else {
        format!("com.epicgames.launcher://apps/{ns}%3A{item}%3A{app}?action=launch&silent=true")
    };
    let dir = s("InstallLocation");
    Some(StoreGame { store: "epic", name: tidy_name(&name), launch, install_dir: (!dir.is_empty()).then(|| PathBuf::from(dir)) })
}

pub fn epic_manifest_dir() -> PathBuf {
    let pd = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    pd.join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests")
}

pub fn epic_games(dir: &Path) -> Vec<StoreGame> {
    let Ok(rd) = fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<StoreGame> = rd
        .flatten()
        .filter(|e| e.path().extension().map_or(false, |x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| fs::read(e.path()).ok().and_then(|b| parse_epic_item(&b)))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---------------------------------------------------------------------------
// GOG
// ---------------------------------------------------------------------------

/// Name/value pairs from `reg query` output, grouped by key.
fn parse_reg_values(out: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        for ty in ["REG_SZ", "REG_EXPAND_SZ", "REG_DWORD"] {
            let sep = format!("    {ty}    ");
            if let Some(i) = l.find(&sep) {
                v.push((l[..i].trim().to_string(), l[i + sep.len()..].trim().to_string()));
                break;
            }
            // A value whose data is empty ends right after the type.
            if let Some(name) = l.strip_suffix(&format!("    {ty}")) {
                v.push((name.trim().to_string(), String::new()));
                break;
            }
        }
    }
    v
}

pub fn parse_gog_registry(out: &str) -> Vec<StoreGame> {
    let mut games = Vec::new();
    for block in out.split("HKEY_").skip(1) {
        let vals = parse_reg_values(block);
        let get = |k: &str| vals.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        if get("dependsOn").is_some() {
            continue; // DLC
        }
        let (Some(name), Some(exe)) = (get("gameName"), get("exe")) else { continue };
        games.push(StoreGame { store: "gog", name: tidy_name(&name), launch: exe, install_dir: get("path").map(PathBuf::from) });
    }
    games
}

pub fn gog_games() -> Vec<StoreGame> {
    #[cfg(windows)]
    {
        let mut all = Vec::new();
        for key in ["HKLM\\SOFTWARE\\WOW6432Node\\GOG.com\\Games", "HKLM\\SOFTWARE\\GOG.com\\Games"] {
            if let Some(out) = reg_query(&[key, "/s"]) {
                for g in parse_gog_registry(&out) {
                    if Path::new(&g.launch).exists() && !all.iter().any(|a: &StoreGame| a.launch.eq_ignore_ascii_case(&g.launch)) {
                        all.push(g);
                    }
                }
            }
        }
        all
    }
    #[cfg(not(windows))]
    {
        vec![]
    }
}

#[cfg(windows)]
fn reg_query(args: &[&str]) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg").arg("query").args(args).creation_flags(0x0800_0000).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

// ---------------------------------------------------------------------------
// Into the library
// ---------------------------------------------------------------------------

/// "Uncrashed : FPV Drone Simulator" -> "Uncrashed: FPV Drone Simulator"; drops ™ and ®.
fn tidy_name(n: &str) -> String {
    let n: String = n.chars().filter(|c| !matches!(c, '™' | '®' | '©')).collect();
    n.replace(" : ", ": ").split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn installed() -> Vec<StoreGame> {
    let mut all = Vec::new();
    if let Some(root) = steam_root() {
        all.extend(steam_games(&root));
    }
    all.extend(epic_games(&epic_manifest_dir()));
    all.extend(gog_games());
    all
}

/// Steam app id of a store game or shortcut path ("steam://rungameid/123").
pub fn steam_uri_id(path: &Path) -> Option<u32> {
    let s = path.to_string_lossy();
    let rest = s.strip_prefix("steam://rungameid/").or_else(|| s.strip_prefix("steam:\\\\rungameid\\"))?;
    rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

/// Add store games to the PC Games console. Games already in the library (a Steam
/// shortcut in the ROM folder for the same app, or the same title) are left as they are,
/// so favorites and playtime stay attached. Returns how many were added.
pub fn merge(lib: &mut Library, games: Vec<StoreGame>) -> usize {
    if games.is_empty() {
        return 0;
    }
    if !lib.systems.iter().any(|s| s.id == "pc") {
        let sys = systems::SYSTEMS.iter().find(|s| s.id == "pc").expect("pc system");
        lib.systems.push(SystemLibrary { id: sys.id, name: sys.name, short: sys.short, color: sys.color, folders: vec![], games: vec![] });
        lib.systems.sort_by_key(|s| systems::SYSTEMS.iter().position(|x| x.id == s.id).unwrap_or(usize::MAX));
    }
    let pc = lib.systems.iter_mut().find(|s| s.id == "pc").unwrap();
    let mut added = 0;
    for sg in games {
        let path = PathBuf::from(&sg.launch);
        let app = steam_uri_id(&path);
        let key = title::match_key(&sg.name);
        let dup = pc.games.iter().any(|g| {
            (app.is_some() && crate::covers::steam_app_id(&g.path) == app) || title::match_key(&g.title) == key
        });
        if dup {
            continue;
        }
        let sort_key = title::sort_key(&sg.name);
        let mut id = format!("pc/{}", slug(&sort_key));
        if pc.games.iter().any(|g| g.id == id) {
            id = format!("{id}-{}", sg.store);
        }
        pc.games.push(Game {
            id,
            letter: title::index_letter(&sort_key),
            title: sg.name.clone(),
            sort_key,
            path: path.clone(),
            format: sg.store.to_string(),
            region: None,
            revision: None,
            title_id: None,
            versions: vec![Version { path, format: sg.store.to_string(), size: 0, disc: None }],
            updates: 0,
            dlc: 0,
            cover: None,
            video: None,
            meta: None,
            info: None,
            warnings: vec![],
        });
        added += 1;
    }
    pc.games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    lib.game_count = lib.systems.iter().map(|s| s.games.len()).sum();
    added
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

#[cfg(test)]
mod tests {
    use super::*;

    const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stores");

    #[test]
    fn steam_real_files() {
        let lf = fs::read_to_string(format!("{FIX}/Steam--steamapps/libraryfolders.vdf")).unwrap();
        assert_eq!(parse_library_folders(&lf), vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"G:\SteamLibrary")]);
        let mut names = Vec::new();
        for e in fs::read_dir(format!("{FIX}/Steam--steamapps")).unwrap().flatten() {
            if e.file_name().to_string_lossy().ends_with(".acf") {
                if let Some(g) = parse_app_manifest(&fs::read_to_string(e.path()).unwrap()) {
                    names.push(g.name);
                }
            }
        }
        names.sort();
        assert_eq!(names.len(), 10, "{names:?}");
        assert!(names.contains(&"Borderlands 3".to_string()));
        assert!(names.contains(&"Uncrashed: FPV Drone Simulator".to_string()));
        assert!(!names.iter().any(|n| n.contains("Redistributable")));
        let bl3 = parse_app_manifest(&fs::read_to_string(format!("{FIX}/Steam--steamapps/appmanifest_397540.acf")).unwrap()).unwrap();
        assert_eq!(bl3.launch, "steam://rungameid/397540");
        assert_eq!(steam_uri_id(Path::new(&bl3.launch)), Some(397540));
    }

    #[test]
    fn steam_half_downloaded_is_skipped() {
        let t = "\"AppState\"\n{\n\t\"appid\"\t\t\"10\"\n\t\"name\"\t\t\"Counter-Strike\"\n\t\"StateFlags\"\t\t\"1026\"\n}\n";
        assert!(parse_app_manifest(t).is_none());
        assert!(parse_app_manifest(&t.replace("1026", "4")).is_some());
    }

    #[test]
    fn epic_real_files() {
        let g = epic_games(Path::new(&format!("{FIX}/Manifests")));
        let names: Vec<_> = g.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["20 Minutes Till Dawn", "Brotato"]);
        assert_eq!(g[1].launch, "com.epicgames.launcher://apps/3b391953ce4b42bb9a28a2fee249154b%3A518490da62794d0a8f3695cbe2fcf0a6%3Ae8bbb84be35640cda646233152ff3428?action=launch&silent=true");
        let dlc = br#"{"DisplayName":"Extra","AppName":"b","MainGameAppName":"a","AppCategories":["addons"]}"#;
        assert!(parse_epic_item(dlc).is_none());
    }

    #[test]
    fn gog_registry() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1207658924\r\n    gameID    REG_SZ    1207658924\r\n    gameName    REG_SZ    Stardew Valley\r\n    exe    REG_SZ    C:\\GOG Games\\Stardew Valley\\Stardew Valley.exe\r\n    path    REG_SZ    C:\\GOG Games\\Stardew Valley\r\n    dependsOn    REG_SZ\r\n\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1453375253\r\n    gameName    REG_SZ    Some DLC\r\n    exe    REG_SZ    C:\\GOG Games\\X\\x.exe\r\n    dependsOn    REG_SZ    1207658924\r\n";
        let g = parse_gog_registry(out);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "Stardew Valley");
        assert_eq!(g[0].launch, r"C:\GOG Games\Stardew Valley\Stardew Valley.exe");
    }

    #[test]
    fn merge_skips_what_is_already_there() {
        let mut lib = Library { root: PathBuf::from("/r"), systems: vec![], notes: vec![], game_count: 0 };
        let sg = |n: &str, l: &str, s| StoreGame { store: s, name: n.into(), launch: l.into(), install_dir: None };
        let n = merge(&mut lib, vec![sg("Brotato", "com.epicgames.launcher://apps/x", "epic"), sg("The Witcher 3", r"C:\g\w3.exe", "gog")]);
        assert_eq!(n, 2);
        assert_eq!(lib.game_count, 2);
        // Same title again from another store: kept once.
        assert_eq!(merge(&mut lib, vec![sg("Brotato", "steam://rungameid/1942280", "steam")]), 0);
        let pc = &lib.systems[0];
        assert_eq!(pc.games[0].title, "Brotato");
        assert_eq!(pc.games[0].format, "epic");
        assert_eq!(pc.games[1].letter, 'W');
    }
}
