//! Intro / credits times for anime episodes that have no chapter marks, from AniSkip
//! (api.aniskip.com: times people have submitted, by MyAnimeList id and episode).
//!
//! AniSkip only knows MyAnimeList ids, so Spine first finds the MAL entry for the show
//! and season in Fribb's anime-lists (a public list linking MAL, Kitsu, IMDb and TMDB ids;
//! cached for a week). The result goes to a small file Spine's player script reads:
//! one line per part, "op <start> <end>", "ed ..." or "recap ...", in seconds.

use crate::covers;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const LIST_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-mini.json";
const API: &str = "https://api.aniskip.com/v2/skip-times";

/// Which episode we're asking about, in whatever ids Spine has for it.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    /// A local show matched on TMDB: TMDB show id, TMDB season and episode.
    Tmdb { tv: u64, season: u32, episode: u32 },
    /// A streamed show with an IMDb id (Cinemeta): season and episode as IMDb/TVDB count them.
    Imdb { id: String, season: u32, episode: u32 },
    Kitsu { id: u64, episode: u32 },
    Mal { id: u64, episode: u32 },
}

/// From a streamed episode's key ("series:tt12343534:1:3", "anime:kitsu:42765:3", "series:mal:40748:3").
pub fn from_stream_key(key: &str) -> Option<Lookup> {
    let parts: Vec<&str> = key.split(':').collect();
    let num = |s: &str| s.parse::<u64>().ok();
    match parts.as_slice() {
        [_, id, s, e] if id.starts_with("tt") => Some(Lookup::Imdb { id: id.to_string(), season: num(s)? as u32, episode: num(e)? as u32 }),
        [_, "kitsu", id, e] => Some(Lookup::Kitsu { id: num(id)?, episode: num(e)? as u32 }),
        [_, "kitsu", id, _, e] => Some(Lookup::Kitsu { id: num(id)?, episode: num(e)? as u32 }),
        [_, "mal", id, e] => Some(Lookup::Mal { id: num(id)?, episode: num(e)? as u32 }),
        [_, "mal", id, _, e] => Some(Lookup::Mal { id: num(id)?, episode: num(e)? as u32 }),
        _ => None,
    }
}

/// (MAL id, MAL episode number) for a lookup, using the anime list.
pub fn mal_episode(list: &[Value], l: &Lookup) -> Option<(u64, u32)> {
    let u = |v: &Value, path: &[&str]| path.iter().try_fold(v, |v, k| v.get(*k)).and_then(|x| x.as_u64());
    match l {
        Lookup::Mal { id, episode } => Some((*id, *episode)),
        Lookup::Kitsu { id, episode } => list.iter().find(|x| u(x, &["kitsu_id"]) == Some(*id)).and_then(|x| u(x, &["mal_id"])).map(|m| (m, *episode)),
        Lookup::Tmdb { tv, season, episode } => {
            let c: Vec<&Value> = list.iter().filter(|x| u(x, &["themoviedb_id", "tv"]) == Some(*tv) && u(x, &["mal_id"]).is_some()).collect();
            pick(&c, *season, *episode, |x| u(x, &["season", "tmdb"]), |x| u(x, &["episode_offset", "tmdb"]))
        }
        Lookup::Imdb { id, season, episode } => {
            let has = |x: &Value| match x.get("imdb_id") {
                Some(Value::String(s)) => s == id,
                Some(Value::Array(a)) => a.iter().any(|s| s.as_str() == Some(id)),
                _ => false,
            };
            let c: Vec<&Value> = list.iter().filter(|x| has(x) && u(x, &["mal_id"]).is_some()).collect();
            pick(&c, *season, *episode, |x| u(x, &["season", "tvdb"]), |x| u(x, &["episode_offset", "tvdb"]))
        }
    }
}

/// Among a show's MAL entries, the one holding this season and episode: same season number,
/// the biggest episode offset still below the episode (TMDB often lists later cours of an
/// anime as more episodes of one long season).
fn pick(c: &[&Value], season: u32, episode: u32, season_of: impl Fn(&Value) -> Option<u64>, offset_of: impl Fn(&Value) -> Option<u64>) -> Option<(u64, u32)> {
    let mal = |x: &Value| x.get("mal_id").and_then(|m| m.as_u64());
    let best = c
        .iter()
        .filter(|x| season_of(x).map_or(season == 1 && c.len() == 1, |s| s == season as u64))
        .map(|x| (x, offset_of(x).unwrap_or(0)))
        .filter(|(_, o)| *o < episode as u64)
        .max_by_key(|(_, o)| *o)?;
    Some((mal(best.0)?, episode - best.1 as u32))
}

/// One part to skip, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Skip {
    /// "op", "ed" or "recap".
    pub kind: String,
    pub start: f64,
    pub end: f64,
}

/// AniSkip's answer, keeping for each kind the submission whose episode length is closest
/// to ours. Versions of an episode often differ in length (a studio logo, longer credits):
/// openings and recaps stay counted from the start, endings are moved to stay the same
/// distance from the end. More than 2 minutes apart is likely a different cut: left out.
pub fn parse_skips(json: &str, len: f64) -> Vec<Skip> {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let mut best: Vec<(Skip, f64)> = Vec::new();
    for r in v.get("results").and_then(|r| r.as_array()).into_iter().flatten() {
        let kind = r.get("skipType").and_then(|k| k.as_str()).unwrap_or("");
        if !matches!(kind, "op" | "ed" | "recap") {
            continue; // "mixed-op"/"mixed-ed" play over the story: skipping them would skip scenes
        }
        let iv = r.get("interval");
        let (Some(start), Some(end)) = (iv.and_then(|i| i.get("startTime")).and_then(|x| x.as_f64()), iv.and_then(|i| i.get("endTime")).and_then(|x| x.as_f64())) else { continue };
        let their = r.get("episodeLength").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let shift = if len > 0.0 && their > 0.0 { len - their } else { 0.0 };
        let off = shift.abs();
        if off > 120.0 || end <= start {
            continue;
        }
        let (start, end) = if kind == "ed" { (start + shift, (end + shift).min(if len > 0.0 { len } else { f64::MAX })) } else { (start, end) };
        if start < 0.0 || end <= start {
            continue;
        }
        let s = Skip { kind: kind.to_string(), start, end };
        match best.iter_mut().find(|(b, _)| b.kind == s.kind) {
            Some(b) if off < b.1 => *b = (s, off),
            Some(_) => {}
            None => best.push((s, off)),
        }
    }
    best.into_iter().map(|(s, _)| s).collect()
}

/// The file Spine's player script reads.
pub fn to_text(skips: &[Skip]) -> String {
    skips.iter().map(|s| format!("{} {:.3} {:.3}\n", s.kind, s.start, s.end)).collect()
}

/// The anime list, downloaded at most once a week into `cache`.
pub fn load_list(cache: &Path) -> Result<Vec<Value>, String> {
    let file: PathBuf = cache.join("anime-list-mini.json");
    let fresh = std::fs::metadata(&file).ok().and_then(|m| m.modified().ok()).and_then(|m| SystemTime::now().duration_since(m).ok()).map_or(false, |age| age < Duration::from_secs(7 * 24 * 3600));
    if !fresh {
        if let Ok(b) = covers::get_bytes(&covers::agent(), LIST_URL) {
            if serde_json::from_slice::<Vec<Value>>(&b).is_ok() {
                let _ = std::fs::create_dir_all(cache);
                let _ = std::fs::write(&file, &b);
            }
        }
    }
    let text = std::fs::read_to_string(&file).map_err(|e| format!("anime list: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("anime list: {e}"))
}

/// Intro / credits times for one episode. `len` = the video's length in seconds.
pub fn fetch(cache: &Path, l: &Lookup, len: f64) -> Result<Vec<Skip>, String> {
    let (mal, ep) = match l {
        Lookup::Mal { id, episode } => (*id, *episode),
        _ => mal_episode(&load_list(cache)?, l).ok_or("not in the anime list")?,
    };
    let url = format!("{API}/{mal}/{ep}?types[]=op&types[]=ed&types[]=recap&episodeLength=0");
    let body = covers::get_text_any(&covers::agent(), &url)?;
    Ok(parse_skips(&body, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Vec<Value> {
        serde_json::from_str(r#"[
          {"mal_id":40748,"kitsu_id":42765,"imdb_id":["tt12343534"],"themoviedb_id":{"tv":95479},"season":{"tvdb":1,"tmdb":1}},
          {"mal_id":51009,"kitsu_id":45857,"imdb_id":["tt12343534"],"themoviedb_id":{"tv":95479},"season":{"tvdb":2,"tmdb":2}},
          {"mal_id":57658,"imdb_id":["tt12343534"],"themoviedb_id":{"tv":95479},"season":{"tvdb":3,"tmdb":1},"episode_offset":{"tmdb":47}},
          {"mal_id":1,"imdb_id":"tt0213338","themoviedb_id":{"tv":30991}},
          {"type":"MOVIE","mal_id":2,"themoviedb_id":{"movie":5}}
        ]"#).unwrap()
    }

    #[test]
    fn finds_mal_episodes() {
        let l = list();
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 95479, season: 1, episode: 3 }), Some((40748, 3)));
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 95479, season: 2, episode: 5 }), Some((51009, 5)));
        // TMDB's long season 1: episode 49 is the Culling Game's episode 2.
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 95479, season: 1, episode: 49 }), Some((57658, 2)));
        assert_eq!(mal_episode(&l, &Lookup::Imdb { id: "tt12343534".into(), season: 2, episode: 1 }), Some((51009, 1)));
        // One entry, no season info: season 1 only.
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 30991, season: 1, episode: 7 }), Some((1, 7)));
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 30991, season: 2, episode: 7 }), None);
        assert_eq!(mal_episode(&l, &Lookup::Kitsu { id: 45857, episode: 4 }), Some((51009, 4)));
        assert_eq!(mal_episode(&l, &Lookup::Tmdb { tv: 1, season: 1, episode: 1 }), None);
    }

    #[test]
    fn stream_keys() {
        assert_eq!(from_stream_key("series:tt12343534:1:3"), Some(Lookup::Imdb { id: "tt12343534".into(), season: 1, episode: 3 }));
        assert_eq!(from_stream_key("anime:kitsu:42765:3"), Some(Lookup::Kitsu { id: 42765, episode: 3 }));
        assert_eq!(from_stream_key("series:mal:40748:3"), Some(Lookup::Mal { id: 40748, episode: 3 }));
        assert_eq!(from_stream_key("movie:tt0111161"), None);
    }

    #[test]
    fn skips() {
        let j = r#"{"found":true,"results":[
          {"interval":{"startTime":54.7,"endTime":145.1},"skipType":"op","episodeLength":1435.1},
          {"interval":{"startTime":50.0,"endTime":140.0},"skipType":"op","episodeLength":1500.0},
          {"interval":{"startTime":1338.2,"endTime":1428.2},"skipType":"ed","episodeLength":1435.0},
          {"interval":{"startTime":223.9,"endTime":313.9},"skipType":"mixed-ed","episodeLength":1492.1}]}"#;
        let s = parse_skips(j, 1435.0);
        assert_eq!(s, vec![Skip { kind: "op".into(), start: 54.7, end: 145.1 }, Skip { kind: "ed".into(), start: 1338.2, end: 1428.2 }]);
        assert_eq!(to_text(&s[..1]), "op 54.700 145.100\n");
        // A version 40 s shorter: the opening stays, the ending moves with the end.
        let s = parse_skips(j, 1395.0);
        assert_eq!(s[0], Skip { kind: "op".into(), start: 54.7, end: 145.1 });
        assert!((s[1].start - 1298.2).abs() < 0.01 && (s[1].end - 1388.2).abs() < 0.01);
        // A much longer cut: its times would be off.
        assert!(parse_skips(j, 1700.0).is_empty());
        assert!(parse_skips("{\"found\":false,\"results\":[]}", 1400.0).is_empty());
    }

    /// Against the real anime list and AniSkip: `cargo test -- --ignored live_aniskip`.
    #[test]
    #[ignore]
    fn live_aniskip() {
        let dir = std::env::temp_dir().join("spine-aniskip-test");
        let s = fetch(&dir, &Lookup::Tmdb { tv: 95479, season: 1, episode: 1 }, 1435.0).unwrap();
        assert!(s.iter().any(|x| x.kind == "op"), "{s:?}");
        let s = fetch(&dir, &from_stream_key("series:tt12343534:1:2").unwrap(), 1435.0).unwrap();
        assert!(!s.is_empty(), "{s:?}");
    }
}
