//! Movies: find films in the user's movie folders, clean up their names, and get
//! posters, banners, logos and details from TMDB (themoviedb.org).
//!
//! Movies show up as one more shelf beside the consoles (system id "movies"), so
//! the grid, A–Z rail, search and detail page work for them unchanged. They play
//! in mpv or VLC, which Spine finds the same way it finds emulators.
//!
//! Folder layouts understood:
//!   Movie Name (2010).mkv
//!   Movie Name (2010)/Movie Name (2010).mkv
//!   Movie.Name.2010.1080p.BluRay.x264-GROUP.mkv
//! TV episodes (S01E02, 1x02) and "Season 01" folders are skipped for now.

use crate::covers::{self, PATH_SEG};
use crate::scan::{Game, Library, SystemLibrary, Version};
use crate::screenscraper::{info_path, load_info, GameInfo};
use crate::title;
use percent_encoding::utf8_percent_encode;
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SYSTEM_ID: &str = "movies";

pub const VIDEO_EXTS: &[&str] = &["mkv", "mp4", "m4v", "avi", "mov", "wmv", "webm", "ts", "m2ts", "mpg", "mpeg", "divx", "ogm", "flv"];

/// Release-name words that come after the title ("Movie.Name.1080p.BluRay.x264").
const JUNK: &[&str] = &[
    "1080p", "1080i", "720p", "480p", "576p", "2160p", "4k", "uhd", "bluray", "blu", "brrip", "bdrip", "bdremux", "remux", "webrip", "web", "webdl", "hdtv",
    "hdrip", "dvdrip", "dvdscr", "dvd", "x264", "x265", "h264", "h265", "hevc", "avc", "xvid", "divx", "hdr", "hdr10", "dv", "10bit", "8bit", "aac", "ac3",
    "dts", "ddp", "dd5", "atmos", "truehd", "proper", "repack", "extended", "unrated", "remastered", "imax", "multi", "subbed", "dubbed", "yify", "yts", "rarbg",
];

/// Folders whose videos aren't the movie itself.
const SKIP_DIRS: &[&str] = &["extras", "featurettes", "behind the scenes", "deleted scenes", "trailers", "trailer", "samples", "sample", "subs", "subtitles", "interviews", "shorts"];

fn year_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[\(\[]?\b((?:19|20)\d{2})\b[\)\]]?").unwrap())
}

fn episode_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\bs\d{1,2}[ ._-]?e\d{1,3}\b|\b\d{1,2}x\d{2,3}\b|^season[ ._-]*\d+$").unwrap())
}

fn latest_year() -> u32 {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    1970 + (secs / 31_556_952) as u32 + 1
}

/// Looks like a TV episode or season folder (kept out of Movies).
pub fn looks_like_episode(name: &str) -> bool {
    episode_re().is_match(name)
}

/// "Movie.Name.2010.1080p.BluRay.x264-GROUP" -> ("Movie Name", Some("2010")).
pub fn parse_name(raw: &str) -> (String, Option<String>) {
    // Group tags in brackets: "[YTS.MX]", "{tmdb-123}".
    let mut s = String::new();
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    // Dots and underscores stand for spaces in release names ("Mr. Smith" keeps its dot).
    s = if !s.contains(' ') { s.replace(['.', '_'], " ") } else { s.replace('_', " ") };

    // The year: the last one that isn't the very start of the name
    // ("1917 (2019)" -> 2019, "2001 A Space Odyssey (1968)" -> 1968).
    let max = latest_year();
    let year = year_re()
        .captures_iter(&s)
        .filter_map(|c| {
            let whole = c.get(0)?;
            let y = c.get(1)?;
            let n: u32 = y.as_str().parse().ok()?;
            (whole.start() > 0 && (1900..=max).contains(&n)).then(|| (whole.start(), y.as_str().to_string()))
        })
        .last();
    let mut title = match &year {
        Some((at, _)) => s[..*at].to_string(),
        None => {
            // No year: cut at the first release word ("Movie Name 1080p BluRay").
            let words: Vec<&str> = s.split_whitespace().collect();
            let cut = words.iter().enumerate().skip(1).find(|(_, w)| {
                let k: String = w.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                JUNK.contains(&k.as_str()) || k.starts_with("web") && k.len() <= 6
            });
            words[..cut.map_or(words.len(), |(i, _)| i)].join(" ")
        }
    };
    // Leftovers: "Movie Name -", "Movie Name (", trailing release group "-GROUP" was after the year.
    title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = title.trim_matches(|c: char| c.is_whitespace() || "-.([_,".contains(c)).to_string();
    let title = if title.is_empty() { raw.trim().to_string() } else { title };
    (title, year.map(|(_, y)| y))
}

pub(crate) fn slug(s: &str) -> String {
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

pub(crate) fn is_video(p: &Path) -> bool {
    p.extension().map_or(false, |e| VIDEO_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

fn visit(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            let low = name.to_lowercase();
            if !SKIP_DIRS.contains(&low.as_str()) && !looks_like_episode(&name) {
                visit(&p, depth + 1, out);
            }
        } else if is_video(&p) {
            out.push(p);
        }
    }
}

/// The movie a video file is, or None for samples, trailers and TV episodes.
fn movie_of(root: &Path, file: &Path) -> Option<(String, Option<String>)> {
    let stem = file.file_stem()?.to_string_lossy().to_string();
    let low = stem.to_lowercase();
    if looks_like_episode(&stem) || low == "sample" || low.ends_with("-sample") || low.ends_with(" sample") || low.ends_with(".sample") || low.contains("trailer") {
        return None;
    }
    let (t, y) = parse_name(&stem);
    // "Movie Name (2010)/movie.mkv": the folder knows better when the file has no year.
    if y.is_none() {
        if let Some(parent) = file.parent().filter(|p| *p != root) {
            let (pt, py) = parse_name(&parent.file_name()?.to_string_lossy());
            if py.is_some() {
                return Some((pt, py));
            }
        }
    }
    Some((t, y))
}

/// Find every movie under a folder. None when the folder can't be read (unplugged drive).
pub fn scan_root(root: &Path) -> Option<Library> {
    if !root.is_dir() {
        return None;
    }
    let mut files = Vec::new();
    visit(root, 0, &mut files);
    let mut by_id: BTreeMap<String, (Game, u64)> = BTreeMap::new();
    for f in files {
        let Some((t, y)) = movie_of(root, &f) else { continue };
        let size = fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
        let sort_key = title::sort_key(&t);
        let id = format!("{SYSTEM_ID}/{}", slug(&format!("{sort_key} {}", y.clone().unwrap_or_default())));
        let format = f.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let version = Version { path: f.clone(), format: format.clone(), size, disc: None };
        match by_id.get_mut(&id) {
            // Two copies of one movie: the bigger file plays, both are counted.
            Some((g, best)) => {
                g.versions.push(version);
                if size > *best {
                    *best = size;
                    g.path = f;
                    g.format = format;
                }
            }
            None => {
                let g = Game {
                    id: id.clone(),
                    letter: title::index_letter(&sort_key),
                    sort_key,
                    title: t,
                    path: f,
                    format,
                    region: None,
                    // The year from the file name, used to look the movie up.
                    revision: y,
                    title_id: None,
                    versions: vec![version],
                    updates: 0,
                    dlc: 0,
                    cover: None,
                    video: None,
                    meta: None,
                    info: None,
                    warnings: vec![],
                    episodes: vec![],
                };
                by_id.insert(id, (g, size));
            }
        }
    }
    let mut games: Vec<Game> = by_id.into_values().map(|(g, _)| g).collect();
    games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    let sys = crate::systems::system_by_id(SYSTEM_ID).expect("movies system");
    let game_count = games.len();
    let systems = if games.is_empty() { vec![] } else { vec![SystemLibrary { id: sys.id, name: sys.name, short: sys.short, color: sys.color, folders: vec![], games }] };
    Some(Library { root: root.to_path_buf(), systems, notes: vec![], game_count })
}

/// Every movie folder, with unplugged ones keeping what they held last time.
pub fn scan_many(roots: &[PathBuf], cache: &Path) -> (Library, Vec<PathBuf>) {
    crate::scan::scan_many_with(roots, &cache.join("movies"), scan_root)
}

// ---------------------------------------------------------------------------
// TMDB
// ---------------------------------------------------------------------------

pub(crate) const API: &str = "https://api.themoviedb.org/3";
pub(crate) const IMG: &str = "https://image.tmdb.org/t/p/";

/// TMDB gives two kinds of key: a short "API key" (sent in the address) or a long
/// "Read Access Token" (sent as a header). Either works.
fn auth(key: &str) -> (String, Vec<String>) {
    let key = key.trim();
    if key.len() > 40 {
        (String::new(), vec![format!("Authorization: Bearer {key}"), "Accept: application/json".into()])
    } else {
        (format!("&api_key={key}"), vec!["Accept: application/json".into()])
    }
}

pub(crate) fn get_json(url: &str, key: &str) -> Result<serde_json::Value, String> {
    let (q, headers) = auth(key);
    let full = format!("{url}{q}");
    let bytes = covers::curl(&full, true, &headers).map_err(|e| if e.contains("401") { "TMDB didn't accept the key. Check it in Settings → Scrapers.".to_string() } else { e })?;
    serde_json::from_slice(&bytes).map_err(|e| format!("TMDB sent something unexpected: {e}"))
}

/// The best search result: same name (and same year when known) first, else TMDB's top hit.
pub fn pick_movie(json: &serde_json::Value, wanted: &str, year: Option<&str>) -> Option<u64> {
    let results = json["results"].as_array()?;
    let want = title::match_key(wanted);
    let score = |r: &serde_json::Value| {
        let same_name = [&r["title"], &r["original_title"]].iter().any(|t| t.as_str().map_or(false, |t| title::match_key(t) == want));
        let same_year = year.map_or(false, |y| r["release_date"].as_str().map_or(false, |d| d.starts_with(y)));
        same_name as u8 * 2 + same_year as u8
    };
    let best = results.iter().enumerate().max_by_key(|(i, r)| (score(r), usize::MAX - i))?.1;
    best["id"].as_u64()
}

/// Details and image paths from TMDB's /movie/{id}?append_to_response=images.
#[derive(Debug, Default, PartialEq)]
pub struct MovieDetails {
    pub info: GameInfo,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub logo: Option<String>,
}

pub fn parse_details(json: &serde_json::Value) -> MovieDetails {
    let s = |v: &serde_json::Value| v.as_str().map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    let genres: Vec<String> = json["genres"].as_array().map(|a| a.iter().filter_map(|g| s(&g["name"])).collect()).unwrap_or_default();
    let info = GameInfo {
        name: s(&json["title"]),
        description: s(&json["overview"]),
        year: s(&json["release_date"]).filter(|d| d.len() >= 4).map(|d| d[..4].to_string()),
        genre: (!genres.is_empty()).then(|| genres.join(", ")),
        runtime: json["runtime"].as_u64().filter(|m| *m > 0).map(|m| m as u32),
        ..Default::default()
    };
    // Logo: English first, then one with no text language, PNGs preferred.
    let logo = json["images"]["logos"].as_array().and_then(|logos| {
        let rank = |l: &serde_json::Value| {
            let lang = l["iso_639_1"].as_str();
            let png = l["file_path"].as_str().map_or(false, |p| p.ends_with(".png"));
            (if lang == Some("en") { 0 } else if lang.is_none() { 1 } else { 2 }, !png as u8)
        };
        logos.iter().filter(|l| rank(l).0 < 2).min_by_key(|l| rank(l)).and_then(|l| s(&l["file_path"]))
    });
    MovieDetails { info, poster: s(&json["poster_path"]), backdrop: s(&json["backdrop_path"]), logo }
}

pub(crate) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) fn write_info(media: &Path, id: &str, info: &GameInfo) {
    let p = info_path(media, id);
    let _ = fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
    let _ = fs::write(p, serde_json::to_string_pretty(info).unwrap_or_default());
}

pub(crate) fn needs_lookup(media: &Path, g: &Game) -> bool {
    match load_info(media, &g.id) {
        None => true,
        Some(i) => i.not_found_at.map_or(false, |t| now().saturating_sub(t) > 30 * 24 * 3600),
    }
}

/// The name to search for: what the file said (a later TMDB name doesn't count).
fn search_name(g: &Game) -> String {
    let stem = g.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let (t, _) = parse_name(&stem);
    if g.revision.is_some() || t.is_empty() { g.title.clone() } else { t }
}

/// Look up every movie TMDB hasn't been asked about yet. Returns how many were found.
pub fn fetch_details(lib: &Library, media: &Path, key: &str, progress: &dyn Fn(usize, usize, &str)) -> Result<usize, String> {
    let todo: Vec<&Game> = lib.systems.iter().filter(|s| s.id == SYSTEM_ID).flat_map(|s| s.games.iter()).filter(|g| needs_lookup(media, g)).collect();
    let agent = covers::agent();
    let mut found = 0;
    for (n, g) in todo.iter().enumerate() {
        progress(n, todo.len(), &g.title);
        let name = search_name(g);
        let q = utf8_percent_encode(&name, PATH_SEG).to_string();
        let year = g.revision.as_deref();
        let (mut hit, mut failed) = (None, false);
        for y in [year, None] {
            let url = format!("{API}/search/movie?query={q}&include_adult=false&language=en-US{}", y.map(|y| format!("&year={y}")).unwrap_or_default());
            match get_json(&url, key) {
                Ok(j) => hit = pick_movie(&j, &name, year),
                Err(e) if e.contains("key") => return Err(e),
                Err(_) => {
                    failed = true; // offline or TMDB busy: try again next time
                    break;
                }
            }
            if hit.is_some() || year.is_none() {
                break;
            }
        }
        if failed {
            continue;
        }
        let Some(tmdb) = hit else {
            write_info(media, &g.id, &GameInfo { not_found_at: Some(now()), ..Default::default() });
            continue;
        };
        let j = match get_json(&format!("{API}/movie/{tmdb}?language=en-US&append_to_response=images&include_image_language=en,null"), key) {
            Ok(j) => j,
            Err(e) if e.contains("key") => return Err(e),
            Err(_) => continue,
        };
        let d = parse_details(&j);
        let img = |size: &str, path: &Option<String>, dest: PathBuf| {
            if let Some(p) = path {
                let _ = covers::download_image(&agent, &format!("{IMG}{size}{p}"), &dest.with_extension("jpg"));
            }
        };
        if covers::existing_cover(media, &g.id).is_none() {
            img("w500", &d.poster, covers::cover_base(media, &g.id));
        }
        if crate::extras::existing_hero(media, &g.id).is_none() {
            img("w1280", &d.backdrop, art_base(media, "heroes", &g.id));
        }
        if crate::extras::existing_logo(media, &g.id).is_none() {
            img("w500", &d.logo, art_base(media, "logos", &g.id));
        }
        write_info(media, &g.id, &d.info);
        found += 1;
    }
    progress(todo.len(), todo.len(), "");
    Ok(found)
}

pub(crate) fn art_base(media: &Path, kind: &str, id: &str) -> PathBuf {
    let mut p = media.join(kind);
    for part in id.split('/') {
        p.push(part);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> (String, Option<String>) {
        parse_name(s)
    }

    #[test]
    fn cleans_names() {
        assert_eq!(p("Inception (2010)"), ("Inception".into(), Some("2010".into())));
        assert_eq!(p("The.Dark.Knight.2008.1080p.BluRay.x264-GROUP"), ("The Dark Knight".into(), Some("2008".into())));
        assert_eq!(p("Blade Runner 2049 (2017)"), ("Blade Runner 2049".into(), Some("2017".into())));
        assert_eq!(p("1917 (2019)"), ("1917".into(), Some("2019".into())));
        assert_eq!(p("1917"), ("1917".into(), None));
        assert_eq!(p("2001 A Space Odyssey (1968)"), ("2001 A Space Odyssey".into(), Some("1968".into())));
        assert_eq!(p("Spirited_Away_2001_720p"), ("Spirited Away".into(), Some("2001".into())));
        assert_eq!(p("[YTS.MX] Parasite (2019) [1080p]"), ("Parasite".into(), Some("2019".into())));
        assert_eq!(p("Alien (1979) (Director's Cut)"), ("Alien".into(), Some("1979".into())));
        assert_eq!(p("Mad Max Fury Road 1080p WEB-DL"), ("Mad Max Fury Road".into(), None));
        assert_eq!(p("Mr. Smith Goes to Washington"), ("Mr. Smith Goes to Washington".into(), None));
        assert_eq!(p("Amélie - 2001"), ("Amélie".into(), Some("2001".into())));
    }

    #[test]
    fn spots_episodes() {
        assert!(looks_like_episode("Show Name S01E02 - Pilot"));
        assert!(looks_like_episode("show.name.s01e02.720p"));
        assert!(looks_like_episode("Show 1x02"));
        assert!(looks_like_episode("Season 01"));
        assert!(!looks_like_episode("Inception (2010)"));
        assert!(!looks_like_episode("Movie 1920x1080"));
        assert!(!looks_like_episode("Seven Seasons of Something (2001)"));
    }

    #[test]
    fn scans_a_movie_folder() {
        let base = std::env::temp_dir().join(format!("spine-movies-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("Movies");
        fs::create_dir_all(root.join("Heat (1995)")).unwrap();
        fs::create_dir_all(root.join("Heat (1995)").join("Extras")).unwrap();
        fs::create_dir_all(root.join("Some Show").join("Season 01")).unwrap();
        fs::write(root.join("Heat (1995)").join("heat.mkv"), b"1234").unwrap();
        fs::write(root.join("Heat (1995)").join("Extras").join("Making of.mkv"), b"x").unwrap();
        fs::write(root.join("Heat (1995)").join("heat-sample.mkv"), b"x").unwrap();
        fs::write(root.join("Some Show").join("Season 01").join("Some Show S01E01.mkv"), b"x").unwrap();
        fs::write(root.join("Heat.1995.720p.mp4"), b"12").unwrap();
        fs::write(root.join("The Matrix (1999).mp4"), b"x").unwrap();
        fs::write(root.join("notes.txt"), b"x").unwrap();
        let lib = scan_root(&root).unwrap();
        assert_eq!(lib.systems.len(), 1);
        let games = &lib.systems[0].games;
        let names: Vec<(&str, Option<&str>)> = games.iter().map(|g| (g.title.as_str(), g.revision.as_deref())).collect();
        assert_eq!(names, vec![("Heat", Some("1995")), ("The Matrix", Some("1999"))]);
        assert_eq!(games[0].id, "movies/heat-1995");
        assert_eq!(games[0].versions.len(), 2, "two copies counted");
        assert!(games[0].path.ends_with("heat.mkv"), "the bigger copy plays");
        assert!(scan_root(&base.join("unplugged")).is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn picks_tmdb_results() {
        let j: serde_json::Value = serde_json::from_str(r#"{"results":[
            {"id":1,"title":"Heat Wave","release_date":"1995-01-01"},
            {"id":2,"title":"Heat","release_date":"1986-07-04"},
            {"id":3,"title":"Heat","original_title":"Heat","release_date":"1995-12-15"}]}"#).unwrap();
        assert_eq!(pick_movie(&j, "Heat", Some("1995")), Some(3));
        assert_eq!(pick_movie(&j, "Heat", None), Some(2), "same name, TMDB's order");
        assert_eq!(pick_movie(&j, "Something Else", None), Some(1), "top hit when nothing matches");
        let empty: serde_json::Value = serde_json::from_str(r#"{"results":[]}"#).unwrap();
        assert_eq!(pick_movie(&empty, "Heat", None), None);
    }

    #[test]
    fn reads_tmdb_details() {
        let j: serde_json::Value = serde_json::from_str(r#"{"id":949,"title":"Heat","overview":"Obsessive master thief...",
            "release_date":"1995-12-15","runtime":170,"genres":[{"id":28,"name":"Action"},{"id":80,"name":"Crime"}],
            "poster_path":"/poster.jpg","backdrop_path":"/back.jpg",
            "images":{"logos":[{"file_path":"/de.png","iso_639_1":"de"},{"file_path":"/en.svg","iso_639_1":"en"},{"file_path":"/en.png","iso_639_1":"en"}]}}"#).unwrap();
        let d = parse_details(&j);
        assert_eq!(d.info.name.as_deref(), Some("Heat"));
        assert_eq!(d.info.year.as_deref(), Some("1995"));
        assert_eq!(d.info.genre.as_deref(), Some("Action, Crime"));
        assert_eq!(d.info.runtime, Some(170));
        assert_eq!(d.poster.as_deref(), Some("/poster.jpg"));
        assert_eq!(d.backdrop.as_deref(), Some("/back.jpg"));
        assert_eq!(d.logo.as_deref(), Some("/en.png"));
        assert_eq!(auth("abc").0, "&api_key=abc");
        assert!(auth(&"x".repeat(200)).1[0].starts_with("Authorization: Bearer"));
    }
}
