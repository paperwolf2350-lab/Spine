//! RetroAchievements: which games have achievements, and how far the user is.
//!
//! Uses the user's own username and Web API key (from retroachievements.org,
//! Settings > Authentication). Games are matched by name against RetroAchievements'
//! list for each console, which is cached for a week.

use crate::covers;
use crate::title;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

const API: &str = "https://retroachievements.org/API";
const MEDIA: &str = "https://media.retroachievements.org";

/// RetroAchievements' console number for a Spine console.
pub fn console_id(system: &str) -> Option<u32> {
    Some(match system {
        "genesis" => 1,
        "n64" => 2,
        "snes" => 3,
        "gb" => 4,
        "gba" => 5,
        "gbc" => 6,
        "nes" | "fds" => 7,
        "pcengine" => 8,
        "segacd" => 9,
        "sega32x" => 10,
        "mastersystem" => 11,
        "psx" => 12,
        "atarilynx" => 13,
        "ngpc" => 14,
        "gamegear" => 15,
        "gc" => 16,
        "nds" => 18,
        "wii" => 19,
        "ps2" => 21,
        "atari2600" => 25,
        "arcade" | "neogeo" => 27,
        "virtualboy" => 28,
        "saturn" => 39,
        "dreamcast" => 40,
        "psp" => 41,
        "wonderswan" => 53,
        "pcenginecd" => 76,
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RaGame {
    pub id: u64,
    pub title: String,
    pub achievements: u64,
}

pub fn parse_game_list(json: &[u8]) -> Vec<RaGame> {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else { return vec![] };
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|g| {
            Some(RaGame { id: g["ID"].as_u64()?, title: g["Title"].as_str()?.to_string(), achievements: g["NumAchievements"].as_u64().unwrap_or(0) })
        })
        .collect()
}

/// "Legend of Zelda, The: A Link to the Past" -> the same key as Spine's title.
/// Hacks, homebrew, prototypes ("~Hack~ ...") and subsets ("[Subset - ...]") never match.
pub fn ra_key(title_ra: &str) -> Option<String> {
    let t = title_ra.trim();
    if t.starts_with('~') || t.contains("[Subset") {
        return None;
    }
    // Move a trailing ", The" back to the front of its part.
    let fixed: Vec<String> = t
        .split(':')
        .map(|part| {
            let p = part.trim();
            for art in [", The", ", A", ", An"] {
                if let Some(base) = p.strip_suffix(art) {
                    return format!("{} {base}", &art[2..]);
                }
            }
            p.to_string()
        })
        .collect();
    Some(title::match_key(&fixed.join(" ").replace(" - ", " ").replace('|', " ")))
}

/// The RetroAchievements game for a Spine title, if there is exactly one sensible match.
pub fn find_game<'a>(list: &'a [RaGame], spine_title: &str) -> Option<&'a RaGame> {
    let want = title::match_key(spine_title);
    let hits: Vec<&RaGame> = list.iter().filter(|g| g.achievements > 0 && ra_key(&g.title).as_deref() == Some(want.as_str())).collect();
    hits.into_iter().max_by_key(|g| g.achievements)
}

fn cached_list(console: u32, key: &str, media: &Path) -> Result<Vec<RaGame>, String> {
    let file = media.join("lists").join("retroachievements").join(format!("{console}.json"));
    if let Ok(m) = fs::metadata(&file) {
        let fresh = m.modified().ok().and_then(|t| SystemTime::now().duration_since(t).ok()).map_or(false, |a| a < Duration::from_secs(7 * 24 * 3600));
        if fresh {
            if let Ok(b) = fs::read(&file) {
                return Ok(parse_game_list(&b));
            }
        }
    }
    let url = format!("{API}/API_GetGameList.php?i={console}&f=1&y={key}");
    let body = covers::get_text_any(&covers::agent(), &url)?;
    let list = parse_game_list(body.as_bytes());
    if list.is_empty() {
        if body.contains("nauthorized") || body.contains("Invalid API Key") || body.contains("\"error\"") {
            return Err("RetroAchievements didn't accept the Web API key. Check it under Settings → Scrapers.".into());
        }
        return Err("Couldn't get RetroAchievements' game list right now.".into());
    }
    let _ = fs::create_dir_all(file.parent().unwrap());
    let _ = fs::write(&file, body);
    Ok(list)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Achievement {
    pub title: String,
    pub description: String,
    pub points: u64,
    pub badge: String,
    pub earned: bool,
    pub hardcore: bool,
    /// When it was earned ("2024-05-01 18:22:10").
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Progress {
    pub game_id: u64,
    pub title: String,
    pub url: String,
    pub total: usize,
    pub earned: usize,
    pub points: u64,
    pub points_earned: u64,
    pub achievements: Vec<Achievement>,
}

pub fn parse_progress(json: &[u8]) -> Option<Progress> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let id = v["ID"].as_u64()?;
    let mut list: Vec<(u64, Achievement)> = v["Achievements"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, a)| {
            let earned_hc = a["DateEarnedHardcore"].as_str().filter(|s| !s.is_empty());
            let earned = a["DateEarned"].as_str().filter(|s| !s.is_empty()).or(earned_hc);
            let badge = a["BadgeName"].as_str().unwrap_or("00000");
            (
                a["DisplayOrder"].as_u64().unwrap_or(0),
                Achievement {
                    title: a["Title"].as_str().unwrap_or("").to_string(),
                    description: a["Description"].as_str().unwrap_or("").to_string(),
                    points: a["Points"].as_u64().unwrap_or(0),
                    badge: format!("{MEDIA}/Badge/{badge}{}.png", if earned.is_some() { "" } else { "_lock" }),
                    earned: earned.is_some(),
                    hardcore: earned_hc.is_some(),
                    date: earned.map(String::from),
                },
            )
        })
        .collect();
    // Earned first (newest first), then the rest in the game's own order.
    list.sort_by(|(oa, a), (ob, b)| b.earned.cmp(&a.earned).then_with(|| b.date.cmp(&a.date)).then(oa.cmp(ob)));
    let achievements: Vec<Achievement> = list.into_iter().map(|(_, a)| a).collect();
    Some(Progress {
        game_id: id,
        title: v["Title"].as_str().unwrap_or("").to_string(),
        url: format!("https://retroachievements.org/game/{id}"),
        total: achievements.len(),
        earned: achievements.iter().filter(|a| a.earned).count(),
        points: achievements.iter().map(|a| a.points).sum(),
        points_earned: achievements.iter().filter(|a| a.earned).map(|a| a.points).sum(),
        achievements,
    })
}

/// Achievements for one game, or Ok(None) when RetroAchievements has none for it.
pub fn progress(system: &str, spine_title: &str, user: &str, key: &str, media: &Path) -> Result<Option<Progress>, String> {
    let Some(console) = console_id(system) else { return Ok(None) };
    let list = cached_list(console, key, media)?;
    let Some(game) = find_game(&list, spine_title) else { return Ok(None) };
    let enc = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')).collect::<String>();
    let url = format!("{API}/API_GetGameInfoAndUserProgress.php?g={}&u={}&y={}", game.id, enc(user), enc(key));
    let body = covers::get_text_any(&covers::agent(), &url)?;
    parse_progress(body.as_bytes()).map(Some).ok_or_else(|| "RetroAchievements didn't answer as expected. Check your username and key under Settings → Scrapers.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_retroachievements_titles() {
        let list = parse_game_list(br#"[{"ID":1,"Title":"Legend of Zelda, The: A Link to the Past","ConsoleID":3,"NumAchievements":40},
            {"ID":2,"Title":"~Hack~ Legend of Zelda, The: A Link to the Past - Redux","NumAchievements":10},
            {"ID":3,"Title":"Pokemon - Emerald Version","NumAchievements":80},
            {"ID":4,"Title":"Pokemon - Emerald Version [Subset - Bonus]","NumAchievements":5},
            {"ID":5,"Title":"Super Mario World","NumAchievements":0}]"#);
        assert_eq!(list.len(), 5);
        assert_eq!(find_game(&list, "The Legend of Zelda: A Link to the Past").map(|g| g.id), Some(1));
        assert_eq!(find_game(&list, "Pokémon Emerald Version").map(|g| g.id), Some(3));
        assert_eq!(find_game(&list, "Super Mario World"), None); // no achievements
        assert_eq!(console_id("gba"), Some(5));
        assert_eq!(console_id("switch"), None);
    }

    #[test]
    fn reads_progress() {
        let j = br#"{"ID":14402,"Title":"Dragster","Achievements":{
            "1":{"ID":1,"Title":"Novice","Description":"Finish under 6s","Points":5,"BadgeName":"250336","DisplayOrder":2,"DateEarned":"2024-01-02 10:00:00"},
            "2":{"ID":2,"Title":"Expert","Description":"Finish under 5.6s","Points":25,"BadgeName":"250337","DisplayOrder":1}}}"#;
        let p = parse_progress(j).unwrap();
        assert_eq!((p.total, p.earned, p.points, p.points_earned), (2, 1, 30, 5));
        assert!(p.achievements[0].earned);
        assert_eq!(p.achievements[1].badge, "https://media.retroachievements.org/Badge/250337_lock.png");
    }
}
