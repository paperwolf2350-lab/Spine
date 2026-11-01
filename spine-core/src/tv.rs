//! TV shows: find shows in the user's TV folders, sort their episode files into
//! seasons, and get posters, details and episode names from TMDB.
//!
//! Each folder directly inside a TV folder is one show. Two folders with the same
//! show name (season 1 and season 2 downloaded separately) become one show.
//! Inside a show, episodes are grouped:
//!   - by season: "Season 2" folders, "S02" in a folder name, S02E05 in the file
//!     name, or "Show II" sub-folders;
//!   - by arc: other sub-folders ("Kimetsu no Yaiba Yuukaku-hen"), named after the folder;
//!   - "Movies & specials": files with no episode number (a movie in its own folder).
//! Openings, endings and extras are skipped.
//!
//! Shows are one more shelf (system id "tv"), so the grid, A–Z and search work
//! unchanged; the show page lists the episodes.

use crate::covers::{self, PATH_SEG};
use crate::movies::{self, art_base, get_json, is_video, needs_lookup, now, slug, write_info, API, IMG};
use crate::scan::{Game, Library, SystemLibrary, Version};
use crate::screenscraper::GameInfo;
use crate::title;
use percent_encoding::utf8_percent_encode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const SYSTEM_ID: &str = "tv";
pub const SPECIALS: &str = "Movies & specials";

/// One episode (or movie / special) of a show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub path: PathBuf,
    /// "Season 1", an arc name, or "Movies & specials".
    pub group: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    /// The title from the file name, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The number written in the file when it counts on across seasons
    /// ("Jujutsu Kaisen - 025" is Season 2 episode 1, and episode 25 overall).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute: Option<u32>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

/// S01E02 / s1e2 / 1x02 -> (season, episode, where the code ends).
fn season_episode(s: &str) -> Option<(u32, u32, usize)> {
    static A: OnceLock<Regex> = OnceLock::new();
    static B: OnceLock<Regex> = OnceLock::new();
    let c = re(&A, r"(?i)(?:^|[^a-z0-9])s(\d{1,2})[ ._-]?e(\d{1,4})(?:[^0-9]|$)").captures(s).or_else(|| re(&B, r"(?:^|[^0-9])(\d{1,2})x(\d{2,3})(?:[^0-9]|$)").captures(s))?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?, c.get(2)?.end()))
}

/// Openings, endings, previews: not episodes.
fn is_extra_file(stem: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)(?:^|[^a-z])(nc ?op|nc ?ed|creditless|op\d{1,2}|ed\d{1,2}|trailer|sample|preview|menu)(?:v?\d+)?(?:[^a-z]|$)").is_match(stem)
}

fn is_extra_dir(name: &str) -> bool {
    let low = name.to_lowercase();
    ["extra", "ncop", "nced", "creditless", "featurette", "bonus", "sample", "trailer", "behind the scenes", "deleted scenes", "subs", "subtitles", "fonts", "scans"].iter().any(|w| low.contains(w))
}

/// Remove [group tags], (notes) and {ids}; underscores and dots become spaces.
fn strip_tags(raw: &str) -> String {
    let mut s = String::new();
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    let s = s.replace('_', " ");
    let s = if !s.contains(' ') { s.replace('.', " ") } else { s };
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A season number written in a folder name: "Season 2", "S02", "2nd Season".
/// "Season 1 & 2" says nothing about one season, so it doesn't count.
fn folder_season(name: &str) -> Option<u32> {
    static A: OnceLock<Regex> = OnceLock::new();
    static MANY: OnceLock<Regex> = OnceLock::new();
    if re(&MANY, r"(?i)seasons?\s*\d+\s*(?:&|and|-|to|\+|,)\s*\d").is_match(name) {
        return None;
    }
    let c = re(&A, r"(?i)(?:^|[^a-z0-9])(?:season[ ._-]*(\d{1,2})|s(\d{1,2})|(\d{1,2})(?:st|nd|rd|th)[ ._-]season)(?:[^a-z0-9]|$)").captures(name)?;
    c.get(1).or(c.get(2)).or(c.get(3))?.as_str().parse().ok()
}

/// "Goblin Slayer II" inside "Goblin Slayer" -> 2.
fn sequel_number(rest: &str) -> Option<u32> {
    match rest.trim().to_lowercase().as_str() {
        "ii" | "2" => Some(2),
        "iii" | "3" => Some(3),
        "iv" | "4" => Some(4),
        "v" | "5" => Some(5),
        "vi" | "6" => Some(6),
        _ => None,
    }
}

/// A show's name and year from its folder:
/// "Heavenly Delusion S01 1080p Dual Audio WEBRip AAC x265-EMBER" -> "Heavenly Delusion".
pub fn show_name(raw: &str) -> (String, Option<String>) {
    // The year in brackets would be lost with the other tags: "Ergo Proxy (2006) [Season 1]".
    static Y: OnceLock<Regex> = OnceLock::new();
    let year = re(&Y, r"\(((?:19|20)\d{2})\)").captures(raw).map(|c| c[1].to_string());
    let s = strip_tags(raw);
    static CUT: OnceLock<Regex> = OnceLock::new();
    let s = match re(&CUT, r"(?i)(?:^|[^a-z0-9])(s\d{1,2}(?:e\d+)?|season\s*\d+|complete|batch)(?:[^a-z0-9]|$)").find(&s) {
        Some(m) if m.start() > 0 => s[..m.start()].to_string(),
        _ => s,
    };
    let (t, y) = movies::parse_name(&s);
    let t = t.trim_end_matches(|c: char| c == '-' || c.is_whitespace()).to_string();
    (t, year.or(y))
}

/// Remove the show's name from the front of a folder or file name.
fn without_show<'a>(name: &'a str, show: &str) -> &'a str {
    let (n, s) = (name.to_lowercase(), show.to_lowercase());
    if n.starts_with(&s) && name.is_char_boundary(s.len()) { name[s.len()..].trim_start_matches(|c: char| c == '-' || c == ':' || c.is_whitespace()) } else { name }
}

/// Episode number and title from a file name with no season code:
/// "Jujutsu Kaisen - 001 - Ryoumen Sukuna", "[DB]Kimetsu no Yaiba_-_01_(...)", "01 - Title", "Ep 5".
fn numbered(stem: &str) -> Option<(u32, Option<String>)> {
    let s = strip_tags(stem);
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static DASH: OnceLock<Regex> = OnceLock::new();
    static EP: OnceLock<Regex> = OnceLock::new();
    let title = |t: Option<regex::Match>| t.map(|m| m.as_str().trim().trim_matches('-').trim().to_string()).filter(|t| !t.is_empty());
    if let Some(c) = re(&LEAD, r"^(\d{1,4})(?:v\d)?(?:\s+-\s+(.+)|\s*$)").captures(&s) {
        return Some((c[1].parse().ok()?, title(c.get(2))));
    }
    if let Some(c) = re(&DASH, r"\s-\s(\d{1,4})(?:v\d)?(?:\s+-\s+(.+?))?\s*-?\s*$").captures(&s) {
        return Some((c[1].parse().ok()?, title(c.get(2))));
    }
    let c = re(&EP, r"(?i)(?:^|\s)(?:ep|episode|e)\.?\s?(\d{1,4})(?:\s|$)").captures(&s)?;
    Some((c[1].parse().ok()?, None))
}

/// The title after an S01E02 code: "S01E01-Heaven and Hell [1151EF50]" -> "Heaven and Hell".
fn title_after(stem: &str, code_end: usize) -> Option<String> {
    let rest = strip_tags(&stem[code_end..]);
    let rest = rest.trim_start_matches(|c: char| c == '-' || c == '.' || c.is_whitespace());
    if rest.is_empty() {
        return None;
    }
    let (t, _) = movies::parse_name(rest);
    let t = t.trim().to_string();
    (!t.is_empty() && t.chars().any(|c| c.is_alphabetic())).then_some(t)
}

fn videos(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
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
            if !is_extra_dir(&name) {
                videos(&p, depth + 1, out);
            }
        } else if is_video(&p) && !p.file_stem().map_or(true, |s| is_extra_file(&s.to_string_lossy())) {
            out.push(p);
        }
    }
}

/// Sort a show folder's video files into episodes.
pub fn episodes_of(show_dir: &Path, show: &str) -> Vec<Episode> {
    let mut files = Vec::new();
    videos(show_dir, 0, &mut files);
    let top_season = show_dir.file_name().and_then(|n| folder_season(&n.to_string_lossy()));
    // (episode, whether its number came from an S01E02 code, size)
    let mut found: Vec<(Episode, bool, u64)> = Vec::new();
    for f in files {
        let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let parent = f.parent().unwrap_or(show_dir).to_path_buf();
        let dirs: Vec<String> = parent.strip_prefix(show_dir).map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect()).unwrap_or_default();
        let size = fs::metadata(&f).map(|m| m.len()).unwrap_or(0);

        if let Some((s, e, end)) = season_episode(&stem) {
            found.push((Episode { path: f, group: format!("Season {s}"), season: Some(s), number: Some(e), title: title_after(&stem, end), absolute: None }, true, size));
            continue;
        }
        let num = numbered(&stem);
        if num.is_none() {
            let clean = strip_tags(&stem);
            let clean = clean.trim_end_matches(|c: char| c == '-' || c.is_whitespace());
            let rest = without_show(clean, show);
            let t = if rest.chars().filter(|c| c.is_alphabetic()).count() >= 3 { rest } else { clean };
            found.push((Episode { path: f, group: SPECIALS.into(), season: None, number: None, title: Some(t.to_string()), absolute: None }, false, size));
            continue;
        }
        let (n, t) = num.unwrap();
        // The season: the nearest folder that names one, then the show folder,
        // then "Show II" folders; a folder named just like the show is season 1.
        let mut season = dirs.iter().rev().find_map(|d| folder_season(d)).or(top_season);
        let mut group = None;
        if season.is_none() {
            match dirs.last() {
                None => season = Some(1),
                Some(d) => {
                    let (dn, _) = show_name(d);
                    let rest = without_show(&dn, show);
                    if title::match_key(&dn) == title::match_key(show) || rest.is_empty() {
                        season = Some(1);
                    } else if let Some(k) = sequel_number(rest) {
                        season = Some(k);
                    } else {
                        group = Some(rest.to_string());
                    }
                }
            }
        }
        let group = group.unwrap_or_else(|| format!("Season {}", season.unwrap_or(1)));
        let t = t.map(|t| without_show(&t, show).to_string()).filter(|t| !t.is_empty());
        found.push((Episode { path: f, group, season, number: Some(n), title: t, absolute: None }, false, size));
    }

    // Seasons numbered on from the last one ("Season 2" holding 025-047) start at 1.
    let mut lowest: BTreeMap<String, u32> = BTreeMap::new();
    for (e, coded, _) in &found {
        if let (Some(n), false) = (e.number, coded) {
            let l = lowest.entry(e.group.clone()).or_insert(n);
            *l = (*l).min(n);
        }
    }
    for (e, coded, _) in &mut found {
        if let (Some(n), false, Some(l)) = (e.number, *coded, lowest.get(&e.group)) {
            if *l > 1 && e.season.is_some() {
                e.number = Some(n - l + 1);
                e.absolute = Some(n);
            }
        }
    }

    // Two copies of one episode: keep the bigger file.
    found.sort_by(|a, b| b.2.cmp(&a.2));
    let mut seen = std::collections::BTreeSet::new();
    found.retain(|(e, _, _)| e.number.is_none() || seen.insert((e.group.clone(), e.number)));
    let mut eps: Vec<Episode> = found.into_iter().map(|(e, _, _)| e).collect();
    eps.sort_by(|a, b| order(a).cmp(&order(b)));
    eps
}

/// Seasons in number order, then arcs A–Z, then movies & specials.
fn order(e: &Episode) -> (u8, u32, String, u32, String) {
    let kind = if e.season.is_some() { 0 } else if e.group == SPECIALS { 2 } else { 1 };
    let t = e.title.clone().unwrap_or_default();
    (kind, e.season.unwrap_or(0), if kind == 1 { e.group.to_lowercase() } else { String::new() }, e.number.unwrap_or(0), t.to_lowercase())
}

/// Find every show under a TV folder. None when the folder can't be read (unplugged drive).
pub fn scan_root(root: &Path) -> Option<Library> {
    if !root.is_dir() {
        return None;
    }
    let mut shows: BTreeMap<String, Game> = BTreeMap::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for dir in dirs {
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        // Only a folder that's nothing but extras ("Extras", "Featurettes"); a show's own
        // folder can mention them ("Jujutsu Kaisen (Season 1 & 2 + NCOP & NCED)").
        if name.starts_with('.') || is_extra_dir(&name) && show_name(&name).0.split_whitespace().count() <= 1 && name.len() < 16 {
            continue;
        }
        let (show, year) = show_name(&name);
        if show.is_empty() {
            continue;
        }
        let eps = episodes_of(&dir, &show);
        if eps.is_empty() {
            continue;
        }
        let sort_key = title::sort_key(&show);
        let id = format!("{SYSTEM_ID}/{}", slug(&sort_key));
        match shows.get_mut(&id) {
            // "WIND BREAKER - S01" and "Wind Breaker S02": one show.
            Some(g) => {
                g.episodes.extend(eps);
                g.episodes.sort_by(|a, b| order(a).cmp(&order(b)));
                g.path = g.episodes[0].path.clone();
                g.revision = g.revision.take().or(year);
            }
            None => {
                let first = eps[0].path.clone();
                let format = first.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                shows.insert(
                    id.clone(),
                    Game {
                        id,
                        letter: title::index_letter(&sort_key),
                        sort_key,
                        title: show,
                        path: first.clone(),
                        format: format.clone(),
                        region: None,
                        revision: year,
                        title_id: None,
                        versions: vec![Version { path: first, format, size: 0, disc: None }],
                        updates: 0,
                        dlc: 0,
                        cover: None,
                        video: None,
                        meta: None,
                        info: None,
                        warnings: vec![],
                        episodes: eps,
                    },
                );
            }
        }
    }
    let mut games: Vec<Game> = shows.into_values().collect();
    games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    let sys = crate::systems::system_by_id(SYSTEM_ID).expect("tv system");
    let game_count = games.len();
    let systems = if games.is_empty() { vec![] } else { vec![SystemLibrary { id: sys.id, name: sys.name, short: sys.short, color: sys.color, folders: vec![], games }] };
    Some(Library { root: root.to_path_buf(), systems, notes: vec![], game_count })
}

/// Every TV folder, with unplugged ones keeping what they held last time.
pub fn scan_many(roots: &[PathBuf], cache: &Path) -> (Library, Vec<PathBuf>) {
    crate::scan::scan_many_with(roots, &cache.join("tv"), scan_root)
}

// ---------------------------------------------------------------------------
// TMDB
// ---------------------------------------------------------------------------

/// What TMDB says about one episode.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TmdbEpisode {
    pub number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still: Option<String>,
}

/// The show's TMDB id and the seasons fetched so far (saved next to its info).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Seasons {
    pub tmdb: u64,
    #[serde(default)]
    pub seasons: BTreeMap<u32, Vec<TmdbEpisode>>,
    /// Every season TMDB has (not counting "Specials"), for matching arcs.
    #[serde(default)]
    pub list: Vec<TmdbSeason>,
    /// Movies in the show's folder, by file name, as found among TMDB's movies.
    #[serde(default)]
    pub specials: BTreeMap<String, Special>,
}

/// A movie that sits in a show's folder ("Jujutsu Kaisen 0"), looked up as a TMDB movie.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Special {
    /// 0: TMDB had nothing for it.
    #[serde(default)]
    pub tmdb: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<u32>,
    /// The search that found it (or failed), shown when fixing a match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Where a special's picture is kept.
pub fn special_still(media: &Path, id: &str, file: &str) -> PathBuf {
    let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    art_base(media, "stills", id).join(format!("special-{}.jpg", slug(&stem)))
}

/// Forget what TMDB said about one special, so it's looked up again.
pub fn forget_special(media: &Path, id: &str, file: &str) {
    if let Some(mut s) = load_seasons(media, id) {
        if s.specials.remove(file).is_some() {
            let p = seasons_path(media, id);
            let _ = fs::write(p, serde_json::to_string(&s).unwrap_or_default());
        }
    }
    let _ = fs::remove_file(special_still(media, id, file));
}

/// "Wind Breaker (2024)" -> ("Wind Breaker", Some("2024")): a year typed at the
/// end narrows the search instead of being searched for as part of the name.
pub fn split_year(t: &str) -> (String, Option<String>) {
    static Y: OnceLock<Regex> = OnceLock::new();
    match re(&Y, r"^(.*?)[\s(\[]+((?:19|20)\d\d)[)\]]?\s*$").captures(t.trim()) {
        Some(c) if !c[1].trim().is_empty() => (c[1].trim().to_string(), Some(c[2].to_string())),
        _ => (t.trim().to_string(), None),
    }
}

/// Forget everything TMDB said about a whole show (details, poster, banner,
/// logo, episode names and pictures), so the next lookup starts over ("Fix match").
/// The user's own cover stays.
pub fn forget_show(media: &Path, id: &str) {
    let _ = fs::remove_file(seasons_path(media, id));
    let _ = fs::remove_file(crate::screenscraper::info_path(media, id));
    let cover = covers::cover_base(media, id);
    for kind in ["heroes", "logos"] {
        let b = art_base(media, kind, id);
        for e in ["jpg", "png", "webp"] {
            let _ = fs::remove_file(b.with_extension(e));
        }
    }
    for e in ["jpg", "png"] {
        let _ = fs::remove_file(cover.with_extension(e));
    }
    let _ = fs::remove_dir_all(art_base(media, "stills", id));
}

/// Names to search TMDB's movies for: the whole file name without tags
/// ("Goblin Slayer Goblin's Crown"), then the part after the show's name.
/// "Movie" and "Gekijouban" are left out: TMDB titles rarely start that way.
pub fn special_queries(e: &Episode) -> Vec<String> {
    static W: OnceLock<Regex> = OnceLock::new();
    let clean = |t: &str| {
        let t = re(&W, r"(?i)\b(the movie|movie|gekijouban|gekijoban|film)\b").replace_all(t, " ");
        t.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c == '-' || c == ':' || c.is_whitespace()).to_string()
    };
    let stem = e.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut out = vec![clean(&strip_tags(&stem))];
    if let Some(t) = &e.title {
        out.push(clean(t));
    }
    out.retain(|q| q.chars().filter(|c| c.is_alphanumeric()).count() >= 2);
    out.dedup();
    out
}

/// One season as TMDB lists it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TmdbSeason {
    pub number: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub episodes: u32,
}

/// The user's answer to "which season is this arc?": TMDB season, and the
/// TMDB episode the arc's episode 1 is (1 unless it's the second half).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArcMap {
    pub season: u32,
    #[serde(default = "one")]
    pub start: u32,
}

fn one() -> u32 {
    1
}

pub fn parse_season_list(json: &serde_json::Value) -> Vec<TmdbSeason> {
    json["seasons"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let number = x["season_number"].as_u64()? as u32;
                    (number > 0).then(|| TmdbSeason { number, name: x["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(String::from), episodes: x["episode_count"].as_u64().unwrap_or(0) as u32 })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Which TMDB season and episode each of a show's files is (None: unknown).
///   - an arc the user matched: that season, counted from its start;
///   - Season N episode E, when TMDB has it;
///   - Season N of a show TMDB lists as one long season ("Jujutsu Kaisen" 1–59):
///     the number in the file ("025") or the count after the earlier seasons.
pub fn resolve(g: &Game, data: &Seasons, arc: &dyn Fn(&str) -> Option<ArcMap>) -> Vec<Option<(u32, u32)>> {
    let has = |s: u32, n: u32| data.seasons.get(&s).map_or(false, |v| v.iter().any(|e| e.number == n));
    let missing = |s: u32| data.seasons.get(&s).map_or(true, |v| v.is_empty()) && !data.list.iter().any(|x| x.number == s && x.episodes > 0);
    // Highest episode number on disk in each season, to count on from.
    let mut highest: BTreeMap<u32, u32> = BTreeMap::new();
    for e in &g.episodes {
        if let (Some(s), Some(n)) = (e.season, e.number) {
            let h = highest.entry(s).or_default();
            *h = (*h).max(n);
        }
    }
    g.episodes
        .iter()
        .map(|e| {
            let n = e.number?;
            if e.season.is_none() {
                let m = arc(&e.group)?;
                return Some((m.season, m.start + n - 1));
            }
            let s = e.season?;
            if has(s, n) {
                return Some((s, n));
            }
            if s > 1 && missing(s) {
                if let Some(a) = e.absolute.filter(|a| has(1, *a)) {
                    return Some((1, a));
                }
                let before: u32 = highest.range(..s).map(|(_, h)| h).sum();
                return has(1, before + n).then_some((1, before + n));
            }
            None
        })
        .collect()
}

fn seasons_path(media: &Path, id: &str) -> PathBuf {
    art_base(media, "info", id).with_extension("seasons.json")
}

pub fn load_seasons(media: &Path, id: &str) -> Option<Seasons> {
    serde_json::from_str(&fs::read_to_string(seasons_path(media, id)).ok()?).ok()
}

/// Where an episode's thumbnail is kept.
pub fn still_path(media: &Path, id: &str, season: u32, number: u32) -> PathBuf {
    art_base(media, "stills", id).join(format!("s{season}e{number}.jpg"))
}

/// The best search result: same name (and first-aired year when known) first, else TMDB's top hit.
pub fn pick_show(json: &serde_json::Value, wanted: &str, year: Option<&str>) -> Option<u64> {
    let results = json["results"].as_array()?;
    let want = title::match_key(wanted);
    let score = |r: &serde_json::Value| {
        let same_name = [&r["name"], &r["original_name"]].iter().any(|t| t.as_str().map_or(false, |t| title::match_key(t) == want));
        let same_year = year.map_or(false, |y| r["first_air_date"].as_str().map_or(false, |d| d.starts_with(y)));
        same_name as u8 * 2 + same_year as u8
    };
    results.iter().enumerate().max_by_key(|(i, r)| (score(r), usize::MAX - i))?.1["id"].as_u64()
}

pub fn parse_show(json: &serde_json::Value) -> movies::MovieDetails {
    // A show looks like a movie with other field names.
    let mut j = json.clone();
    j["title"] = json["name"].clone();
    j["release_date"] = json["first_air_date"].clone();
    j["runtime"] = serde_json::Value::Null;
    movies::parse_details(&j)
}

pub fn parse_season(json: &serde_json::Value) -> Vec<TmdbEpisode> {
    let s = |v: &serde_json::Value| v.as_str().map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    json["episodes"]
        .as_array()
        .map(|a| a.iter().filter_map(|e| Some(TmdbEpisode { number: e["episode_number"].as_u64()? as u32, name: s(&e["name"]), overview: s(&e["overview"]), still: s(&e["still_path"]) })).collect())
        .unwrap_or_default()
}

/// Look up shows TMDB hasn't been asked about, and seasons not fetched yet.
/// `arc(show id, arc name)` is the season the user matched an arc to.
/// `special_query(show id, file name)` is a name the user typed for a special ("Fix match").
/// `show_query(show id)` is a name the user typed for the whole show ("Fix match").
pub fn fetch_details(lib: &Library, media: &Path, key: &str, arc: &dyn Fn(&str, &str) -> Option<ArcMap>, special_query: &dyn Fn(&str, &str) -> Option<String>, show_query: &dyn Fn(&str) -> Option<String>, progress: &dyn Fn(usize, usize, &str)) -> Result<usize, String> {
    let shows: Vec<&Game> = lib.systems.iter().filter(|s| s.id == SYSTEM_ID).flat_map(|s| s.games.iter()).collect();
    let agent = covers::agent();
    let mut found = 0;
    for (n, g) in shows.iter().enumerate() {
        progress(n, shows.len(), &g.title);
        let mut saved = load_seasons(media, &g.id);
        if needs_lookup(media, g) {
            // A name the user typed wins over the folder name, without the folder's year.
            let typed = show_query(&g.id).map(|t| split_year(&t));
            let wanted = typed.as_ref().map_or(g.title.as_str(), |t| t.0.as_str());
            let q = utf8_percent_encode(wanted, PATH_SEG).to_string();
            let year = match &typed {
                Some((_, y)) => y.as_deref(),
                None => g.revision.as_deref(),
            };
            let mut hit = None;
            let mut failed = false;
            for y in [year, None] {
                let url = format!("{API}/search/tv?query={q}&include_adult=false&language=en-US{}", y.map(|y| format!("&first_air_date_year={y}")).unwrap_or_default());
                match get_json(&url, key) {
                    Ok(j) => hit = pick_show(&j, wanted, year),
                    Err(e) if e.contains("key") => return Err(e),
                    Err(_) => {
                        failed = true;
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
            let j = match get_json(&format!("{API}/tv/{tmdb}?language=en-US&append_to_response=images&include_image_language=en,null"), key) {
                Ok(j) => j,
                Err(e) if e.contains("key") => return Err(e),
                Err(_) => continue,
            };
            let d = parse_show(&j);
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
            saved = Some(Seasons { tmdb, seasons: BTreeMap::new(), list: parse_season_list(&j), specials: BTreeMap::new() });
            found += 1;
        }
        // Episode names and thumbnails for the seasons on disk.
        let Some(mut seasons) = saved else { continue };
        let mut changed = false;
        let has_arcs = g.episodes.iter().any(|e| e.season.is_none() && e.number.is_some());
        // Shows looked up before 0.6.1 don't have TMDB's season list yet.
        if seasons.list.is_empty() && has_arcs {
            match get_json(&format!("{API}/tv/{}?language=en-US", seasons.tmdb), key) {
                Ok(j) => {
                    seasons.list = parse_season_list(&j);
                    changed = !seasons.list.is_empty();
                }
                Err(e) if e.contains("key") => return Err(e),
                Err(_) => {}
            }
        }
        let arc_of = |group: &str| arc(&g.id, group);
        let mut wanted: std::collections::BTreeSet<u32> = g.episodes.iter().filter_map(|e| e.season).collect();
        wanted.extend(g.episodes.iter().filter(|e| e.season.is_none()).filter_map(|e| arc_of(&e.group)).map(|m| m.season));
        // A later season TMDB doesn't have is probably counted on in season 1.
        if wanted.iter().any(|s| *s > 1) {
            wanted.insert(1);
        }
        for s in wanted {
            if seasons.seasons.contains_key(&s) {
                continue;
            }
            let eps = match get_json(&format!("{API}/tv/{}/season/{s}?language=en-US", seasons.tmdb), key) {
                Ok(j) => parse_season(&j),
                Err(e) if e.contains("key") => return Err(e),
                Err(e) if e.contains("404") => vec![], // TMDB has no such season: don't ask again
                Err(_) => continue,
            };
            seasons.seasons.insert(s, eps);
            changed = true;
        }
        // Movies in the show's folder: look each up among TMDB's movies.
        for e in g.episodes.iter().filter(|e| e.number.is_none()) {
            let file = e.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            if seasons.specials.contains_key(&file) {
                continue;
            }
            let queries = match special_query(&g.id, &file) {
                Some(q) => vec![q],
                None => special_queries(e),
            };
            let mut hit = None;
            let mut failed = false;
            for q in &queries {
                let url = format!("{API}/search/movie?query={}&include_adult=false&language=en-US", utf8_percent_encode(q, PATH_SEG));
                match get_json(&url, key) {
                    Ok(j) => {
                        if let Some(id) = movies::pick_movie(&j, q, None) {
                            hit = Some((id, q.clone()));
                            break;
                        }
                    }
                    Err(e) if e.contains("key") => return Err(e),
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                continue;
            }
            let special = match hit {
                None => Special { query: queries.first().cloned(), ..Default::default() },
                Some((id, q)) => {
                    let j = match get_json(&format!("{API}/movie/{id}?language=en-US"), key) {
                        Ok(j) => j,
                        Err(e) if e.contains("key") => return Err(e),
                        Err(_) => continue,
                    };
                    let d = movies::parse_details(&j);
                    if let Some(p) = d.backdrop.as_ref().or(d.poster.as_ref()) {
                        let _ = covers::download_image(&agent, &format!("{IMG}w300{p}"), &special_still(media, &g.id, &file));
                    }
                    Special { tmdb: id, name: d.info.name, overview: d.info.description, year: d.info.year, runtime: d.info.runtime, query: Some(q) }
                }
            };
            seasons.specials.insert(file, special);
            changed = true;
        }
        // Thumbnails for every file Spine can match to a TMDB episode.
        for (s, n) in resolve(g, &seasons, &arc_of).into_iter().flatten() {
            let dest = still_path(media, &g.id, s, n);
            if dest.is_file() {
                continue;
            }
            if let Some(p) = seasons.seasons.get(&s).and_then(|v| v.iter().find(|e| e.number == n)).and_then(|e| e.still.clone()) {
                let _ = covers::download_image(&agent, &format!("{IMG}w300{p}"), &dest);
            }
        }
        if changed {
            let p = seasons_path(media, &g.id);
            let _ = fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
            let _ = fs::write(p, serde_json::to_string(&seasons).unwrap_or_default());
        }
    }
    progress(shows.len(), shows.len(), "");
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_names() {
        let n = |s: &str| show_name(s);
        assert_eq!(n("Heavenly Delusion S01 1080p Dual Audio WEBRip AAC x265-EMBER"), ("Heavenly Delusion".into(), None));
        assert_eq!(n("[Anime Time] Jujutsu Kaisen (Season 1 & 2 + Movie + NCOP & NCED) [BD] [Dual Audio][1080p][HEVC 10bit x265][AAC & AC3][Eng Sub] [Batch]"), ("Jujutsu Kaisen".into(), None));
        assert_eq!(n("[Anime Time] I'm A Spider, So What (Season 1) [Dual Audio][BD][1080p][HEVC 10bit x265][AAC][Eng Sub]"), ("I'm A Spider, So What".into(), None));
        assert_eq!(n("[BDRIP] Redo of Healer [1080P-HEVC,JPN-LPCM-2.0,ENG-ASS]"), ("Redo of Healer".into(), None));
        assert_eq!(n("[Sokudo] Canaan [1080p BD AV1][dual audio]"), ("Canaan".into(), None));
        assert_eq!(n("Chained Soldier S01 1080p Dual Audio BDRip 10 bits DD+ x265-EMBER"), ("Chained Soldier".into(), None));
        assert_eq!(n("Ergo Proxy (2006) [Season 1] [BD 1080p HEVC OPUS] [Dual-Audio]"), ("Ergo Proxy".into(), Some("2006".into())));
        assert_eq!(n("Frieren.Beyond.Journeys.End.S01.1080p.NF.WEB-DL.AAC2.0.H.264.MULTi-VARYG"), ("Frieren Beyond Journeys End".into(), None));
        assert_eq!(n("Godzilla S.P S01 1080p Dual Audio BDRip 10 bits DD+ x265-EMBER"), ("Godzilla S.P".into(), None));
        assert_eq!(n("WIND BREAKER - S01 [English Dub] [CR WEB-RIP 1080p HEVC-10 AAC]"), ("WIND BREAKER".into(), None));
        assert_eq!(n("Wind Breaker S02 1080p Dual Audio WEBRip DD+ x265-EMBER"), ("Wind Breaker".into(), None));
        assert_eq!(n("Kimetsu no Yaiba"), ("Kimetsu no Yaiba".into(), None));
        assert_eq!(n("The Office (US)"), ("The Office".into(), None));
    }

    #[test]
    fn episode_numbers() {
        assert_eq!(season_episode("S01E02-Two Confessions [308EE41A]").map(|x| (x.0, x.1)), Some((1, 2)));
        assert_eq!(season_episode("Frieren.Beyond.Journeys.End.S01E28.It.Would").map(|x| (x.0, x.1)), Some((1, 28)));
        assert_eq!(season_episode("Show 2x05").map(|x| (x.0, x.1)), Some((2, 5)));
        assert_eq!(season_episode("Movie 1920x1080"), None);
        assert_eq!(season_episode("Godzilla S.P - 01"), None);
        assert_eq!(numbered("Jujutsu Kaisen - 047 - Shibuya Incident - Gate, Close"), Some((47, Some("Shibuya Incident - Gate, Close".into()))));
        assert_eq!(numbered("[DB]Kimetsu no Yaiba_-_01_(Dual Audio_10bit_BD1080p_x265)"), Some((1, None)));
        assert_eq!(numbered("I'm a Spider, So What - 24"), Some((24, None)));
        assert_eq!(numbered("01 - The Healer Starts Over!"), Some((1, Some("The Healer Starts Over!".into()))));
        assert_eq!(numbered("[DB]Goblin Slayer Goblin's Crown_-_(Dual Audio_10bit_BD1080p_x265)"), None);
        assert_eq!(numbered("Jujutsu Kaisen 0"), None);
        assert!(is_extra_file("[DB]Kimetsu no Yaiba_-_NCOPv1_(10bit_BD1080p_x265)"));
        assert!(is_extra_file("Jujutsu Kaisen - ED2 - Give It Back"));
        assert!(!is_extra_file("Ed, Edd n Eddy - 01"));
        assert!(!is_extra_file("S01E05-Day of Fate"));
        assert_eq!(folder_season("Season 2"), Some(2));
        assert_eq!(folder_season("Go Go Loser Ranger S02 1080p Dual Audio"), Some(2));
        assert_eq!(folder_season("Jujutsu Kaisen (Season 1 & 2 + Movie)"), None);
        assert_eq!(folder_season("Godzilla S.P"), None);
        assert_eq!(title_after("S01E01-Heaven and Hell [1151EF50]", 6).as_deref(), Some("Heaven and Hell"));
    }

    /// The owner's actual layouts (names from their anime folder, empty files).
    #[test]
    fn real_layouts() {
        let base = std::env::temp_dir().join(format!("spine-tv-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("anime");
        let touch = |rel: &str| {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"x").unwrap();
        };
        let jjk = "[Anime Time] Jujutsu Kaisen (Season 1 & 2 + Movie + NCOP & NCED) [BD] [Dual Audio][1080p][HEVC 10bit x265][AAC & AC3][Eng Sub] [Batch]";
        touch(&format!("{jjk}/Season 1/Jujutsu Kaisen - 001 - Ryoumen Sukuna.mkv"));
        touch(&format!("{jjk}/Season 1/Jujutsu Kaisen - 002 - For Myself.mkv"));
        touch(&format!("{jjk}/Season 2/Jujutsu Kaisen - 025 - Hidden Inventory.mkv"));
        touch(&format!("{jjk}/Season 2/Jujutsu Kaisen - 026 - Hidden Inventory 2.mkv"));
        touch(&format!("{jjk}/Jujutsu Kaisen 0/Jujutsu Kaisen 0.mkv"));
        touch(&format!("{jjk}/NCOP & NCED/Season 1/Jujutsu Kaisen - OP1 - Kaikai Kitan.mkv"));
        touch("Kimetsu no Yaiba/Kimetsu no Yaiba/[DB]Kimetsu no Yaiba_-_01_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Kimetsu no Yaiba/Kimetsu no Yaiba/Extras/[DB]Kimetsu no Yaiba_-_NCOPv1_(10bit_BD1080p_x265).mkv");
        touch("Kimetsu no Yaiba/Kimetsu no Yaiba Yuukaku-hen/[DB]Kimetsu no Yaiba Yuukaku-hen_-_01_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Kimetsu no Yaiba/Kimetsu no Yaiba Movie Mugen Ressha-hen/[DB]Kimetsu no Yaiba Movie Mugen Ressha-hen_-_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Goblin Slayer/Goblin Slayer/[DB]Goblin Slayer_-_01_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Goblin Slayer/Goblin Slayer II/[DB]Goblin Slayer II_-_01_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Goblin Slayer/Goblin Slayer Goblin's Crown/[DB]Goblin Slayer Goblin's Crown_-_(Dual Audio_10bit_BD1080p_x265).mkv");
        touch("Goblin Slayer/Goblin Slayer S01 Extras/Extras/[DB]Goblin Slayer_-_NCED_(10bit_BD1080p_x265).mkv");
        touch("Heavenly Delusion S01 1080p Dual Audio WEBRip AAC x265-EMBER/S01E01-Heaven and Hell [1151EF50].mkv");
        touch("Go Go Loser Ranger S01 1080p Dual Audio WEBRip AAC x265-EMBER/S01E01-We Are Justice! The Dragon Keepers! [0FEAF4EE].mkv");
        touch("Go Go Loser Ranger S01 1080p Dual Audio WEBRip AAC x265-EMBER/Go Go Loser Ranger S02 1080p Dual Audio WEBRip AAC x265-EMBER/S02E01-Look! The Dragon Keepers! [00D2858B].mkv");
        touch("WIND BREAKER - S01 [English Dub] [CR WEB-RIP 1080p HEVC-10 AAC]/[TRC] WIND BREAKER - S01E01 [English Dub] [CR WEB-RIP 1080p HEVC-10 AAC] [8937CCAE].mkv");
        touch("Wind Breaker S02 1080p Dual Audio WEBRip DD+ x265-EMBER/S02E01-Rage [84469F80].mkv");
        touch("[Anime Time] I'm A Spider, So What (Season 1) [Dual Audio][BD][1080p][HEVC 10bit x265][AAC][Eng Sub]/I'm a Spider, So What - 01.mkv");
        touch("[BDRIP] Redo of Healer [1080P-HEVC,JPN-LPCM-2.0,ENG-ASS]/01 - The Healer Starts Over!.mkv");
        touch("Ergo Proxy (2006) [Season 1] [BD 1080p HEVC OPUS] [Dual-Audio]/Season 1/Ergo Proxy (2006) - S01E01 - The Pulse of Awakening Awakening.mkv");
        touch("Ergo Proxy (2006) [Season 1] [BD 1080p HEVC OPUS] [Dual-Audio]/read-me.txt");

        let lib = scan_root(&root).unwrap();
        let games = &lib.systems[0].games;
        let titles: Vec<&str> = games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, vec!["Ergo Proxy", "Go Go Loser Ranger", "Goblin Slayer", "Heavenly Delusion", "I'm A Spider, So What", "Jujutsu Kaisen", "Kimetsu no Yaiba", "Redo of Healer", "WIND BREAKER"]);
        let show = |t: &str| games.iter().find(|g| g.title == t).unwrap();
        let eps = |t: &str| show(t).episodes.iter().map(|e| (e.group.clone(), e.number, e.title.clone())).collect::<Vec<_>>();
        let s = |g: &str, n: Option<u32>, t: Option<&str>| (g.to_string(), n, t.map(String::from));

        assert_eq!(eps("Jujutsu Kaisen"), vec![
            s("Season 1", Some(1), Some("Ryoumen Sukuna")), s("Season 1", Some(2), Some("For Myself")),
            s("Season 2", Some(1), Some("Hidden Inventory")), s("Season 2", Some(2), Some("Hidden Inventory 2")),
            s(SPECIALS, None, Some("Jujutsu Kaisen 0")),
        ]);
        assert_eq!(eps("Kimetsu no Yaiba"), vec![s("Season 1", Some(1), None), s("Yuukaku-hen", Some(1), None), s(SPECIALS, None, Some("Movie Mugen Ressha-hen"))]);
        assert_eq!(eps("Goblin Slayer"), vec![s("Season 1", Some(1), None), s("Season 2", Some(1), None), s(SPECIALS, None, Some("Goblin's Crown"))]);
        assert_eq!(eps("Go Go Loser Ranger"), vec![s("Season 1", Some(1), Some("We Are Justice! The Dragon Keepers!")), s("Season 2", Some(1), Some("Look! The Dragon Keepers!"))]);
        assert_eq!(eps("WIND BREAKER"), vec![s("Season 1", Some(1), None), s("Season 2", Some(1), Some("Rage"))]);
        assert_eq!(eps("I'm A Spider, So What"), vec![s("Season 1", Some(1), None)]);
        assert_eq!(eps("Redo of Healer"), vec![s("Season 1", Some(1), Some("The Healer Starts Over!"))]);
        assert_eq!(eps("Ergo Proxy"), vec![s("Season 1", Some(1), Some("The Pulse of Awakening Awakening"))]);
        assert_eq!(show("Ergo Proxy").revision.as_deref(), Some("2006"));
        assert_eq!(show("Jujutsu Kaisen").id, "tv/jujutsu-kaisen");
        assert!(scan_root(&base.join("unplugged")).is_none());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn matches_long_seasons_and_arcs() {
        let ep = |group: &str, season: Option<u32>, number: u32, absolute: Option<u32>| Episode { path: PathBuf::from("x.mkv"), group: group.into(), season, number: Some(number), title: None, absolute };
        let tm = |n: u32| (1..=n).map(|number| TmdbEpisode { number, ..Default::default() }).collect::<Vec<_>>();
        let mut g = Game { id: "tv/x".into(), title: "X".into(), sort_key: "x".into(), letter: 'X', path: PathBuf::new(), format: "mkv".into(), region: None, revision: None, title_id: None, versions: vec![], updates: 0, dlc: 0, cover: None, video: None, meta: None, info: None, warnings: vec![], episodes: vec![] };
        let none = |_: &str| None;

        // Jujutsu Kaisen: TMDB has one season of 59; files say 001-024 and 025-047.
        g.episodes = vec![ep("Season 1", Some(1), 24, None), ep("Season 2", Some(2), 1, Some(25)), ep("Season 2", Some(2), 23, Some(47))];
        let jjk = Seasons { tmdb: 1, seasons: BTreeMap::from([(1, tm(59)), (2, vec![])]), list: vec![], specials: BTreeMap::new() };
        assert_eq!(resolve(&g, &jjk, &none), vec![Some((1, 24)), Some((1, 25)), Some((1, 47))]);

        // Wind Breaker: S01E01-13 and S02E01-12 on disk, TMDB has one season of 25.
        g.episodes = vec![ep("Season 1", Some(1), 13, None), ep("Season 2", Some(2), 1, None), ep("Season 2", Some(2), 12, None)];
        let wb = Seasons { tmdb: 1, seasons: BTreeMap::from([(1, tm(25)), (2, vec![])]), list: vec![], specials: BTreeMap::new() };
        assert_eq!(resolve(&g, &wb, &none), vec![Some((1, 13)), Some((1, 14)), Some((1, 25))]);

        // A real season 2 on TMDB is used as is.
        let two = Seasons { tmdb: 1, seasons: BTreeMap::from([(1, tm(13)), (2, tm(12))]), list: vec![], specials: BTreeMap::new() };
        assert_eq!(resolve(&g, &two, &none), vec![Some((1, 13)), Some((2, 1)), Some((2, 12))]);

        // Demon Slayer arcs, matched by the user: Mugen Ressha-hen is season 2 from 1,
        // Yuukaku-hen season 2 from episode 8; an unmatched arc stays unknown.
        g.episodes = vec![ep("Season 1", Some(1), 26, None), ep("Mugen Ressha-hen", None, 7, None), ep("Yuukaku-hen", None, 1, None), ep("Hashira Geiko-hen", None, 1, None)];
        let ds = Seasons { tmdb: 1, seasons: BTreeMap::from([(1, tm(26)), (2, tm(18))]), list: vec![], specials: BTreeMap::new() };
        let arcs = |a: &str| match a {
            "Mugen Ressha-hen" => Some(ArcMap { season: 2, start: 1 }),
            "Yuukaku-hen" => Some(ArcMap { season: 2, start: 8 }),
            _ => None,
        };
        assert_eq!(resolve(&g, &ds, &arcs), vec![Some((1, 26)), Some((2, 7)), Some((2, 8)), None]);

        let list: serde_json::Value = serde_json::from_str(r#"{"seasons":[{"season_number":0,"name":"Specials","episode_count":5},{"season_number":2,"name":"Entertainment District Arc","episode_count":18}]}"#).unwrap();
        assert_eq!(parse_season_list(&list), vec![TmdbSeason { number: 2, name: Some("Entertainment District Arc".into()), episodes: 18 }]);
    }

    #[test]
    fn special_names() {
        let sp = |file: &str, title: &str| Episode { path: PathBuf::from(file), group: SPECIALS.into(), season: None, number: None, title: Some(title.into()), absolute: None };
        assert_eq!(split_year("Wind Breaker (2024)"), ("Wind Breaker".to_string(), Some("2024".to_string())));
        assert_eq!(split_year(" Dandadan 2024 "), ("Dandadan".to_string(), Some("2024".to_string())));
        assert_eq!(split_year("Mob Psycho 100"), ("Mob Psycho 100".to_string(), None));
        assert_eq!(split_year("1883"), ("1883".to_string(), None));
        assert_eq!(special_queries(&sp("Jujutsu Kaisen 0.mkv", "Jujutsu Kaisen 0")), vec!["Jujutsu Kaisen 0"]);
        assert_eq!(special_queries(&sp("[DB]Goblin Slayer Goblin's Crown_-_(Dual Audio_10bit_BD1080p_x265).mkv", "Goblin's Crown")), vec!["Goblin Slayer Goblin's Crown", "Goblin's Crown"]);
        assert_eq!(special_queries(&sp("[DB]Kimetsu no Yaiba Movie Mugen Ressha-hen_-_(Dual Audio_10bit_BD1080p_x265).mkv", "Movie Mugen Ressha-hen")), vec!["Kimetsu no Yaiba Mugen Ressha-hen", "Mugen Ressha-hen"]);
    }

    #[test]
    fn reads_tmdb_tv() {
        let search: serde_json::Value = serde_json::from_str(r#"{"results":[{"id":5,"name":"Delusion","first_air_date":"2010-01-01"},{"id":7,"name":"Heavenly Delusion","original_name":"天国大魔境","first_air_date":"2023-04-01"}]}"#).unwrap();
        assert_eq!(pick_show(&search, "Heavenly Delusion", None), Some(7));
        let show: serde_json::Value = serde_json::from_str(r#"{"name":"Heavenly Delusion","overview":"Fifteen years after...","first_air_date":"2023-04-01","genres":[{"name":"Animation"}],"poster_path":"/p.jpg","backdrop_path":"/b.jpg"}"#).unwrap();
        let d = parse_show(&show);
        assert_eq!(d.info.name.as_deref(), Some("Heavenly Delusion"));
        assert_eq!(d.info.year.as_deref(), Some("2023"));
        assert_eq!(d.info.runtime, None);
        assert_eq!(d.poster.as_deref(), Some("/p.jpg"));
        let season: serde_json::Value = serde_json::from_str(r#"{"episodes":[{"episode_number":1,"name":"Heaven and Hell","overview":"Maru and Kiruko...","still_path":"/s1.jpg"},{"episode_number":2,"name":"Two Confessions","still_path":null}]}"#).unwrap();
        let eps = parse_season(&season);
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0], TmdbEpisode { number: 1, name: Some("Heaven and Hell".into()), overview: Some("Maru and Kiruko...".into()), still: Some("/s1.jpg".into()) });
        assert_eq!(eps[1].still, None);
    }

    #[test]
    fn forget_show_clears_tmdb_files_but_keeps_own_cover() {
        let media = std::env::temp_dir().join(format!("spine-forget-{}", std::process::id()));
        let id = "tv/wind-breaker";
        let files = [
            crate::screenscraper::info_path(&media, id),
            seasons_path(&media, id),
            covers::cover_base(&media, id).with_extension("jpg"),
            art_base(&media, "heroes", id).with_extension("jpg"),
            art_base(&media, "logos", id).with_extension("png"),
            still_path(&media, id, 1, 1),
        ];
        let own = covers::custom_base(&media, id).with_extension("png");
        for f in files.iter().chain([&own]) {
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, "x").unwrap();
        }
        forget_show(&media, id);
        for f in &files {
            assert!(!f.exists(), "{} should be gone", f.display());
        }
        assert!(own.exists());
        let _ = fs::remove_dir_all(&media);
    }
}
