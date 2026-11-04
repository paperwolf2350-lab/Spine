//! Actors and directors from TMDB: who someone is and what they've been in, for the
//! clickable names on Streaming's details pages. Titles carry TMDB ids; `imdb_id` turns one
//! into the IMDb id ("tt…") that addons like Cinemeta and source addons understand.

use crate::movies::get_json;
use serde::Serialize;
use serde_json::Value;

const API: &str = "https://api.themoviedb.org/3";

fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Person {
    pub tmdb: u64,
    pub name: String,
    pub photo: Option<String>,
    /// "Acting", "Directing"...
    pub known_for: Option<String>,
    pub born: Option<String>,
    pub place: Option<String>,
    pub bio: Option<String>,
    pub credits: Vec<Credit>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Credit {
    pub tmdb: u64,
    /// "movie" or "series" (Stremio's word for TV).
    #[serde(rename = "type")]
    pub typ: String,
    pub name: String,
    pub year: Option<String>,
    pub poster: Option<String>,
    /// Character played, or the job ("Director").
    pub role: Option<String>,
    pub votes: u64,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty()).map(String::from)
}
fn img(path: Option<String>, size: &str) -> Option<String> {
    path.map(|p| format!("https://image.tmdb.org/t/p/{size}{p}"))
}

/// The search result meant: same name first, then someone known for the right work
/// (acting for cast, directing for directors), then TMDB's most popular.
pub fn pick_person(json: &Value, name: &str, director: bool) -> Option<u64> {
    let want = name.trim().to_lowercase();
    let dept = if director { "Directing" } else { "Acting" };
    let results = json["results"].as_array()?;
    results
        .iter()
        .filter_map(|r| Some((r["id"].as_u64()?, r)))
        .max_by(|(_, a), (_, b)| {
            let score = |r: &Value| {
                let same = s(r, "name").map(|n| n.to_lowercase() == want).unwrap_or(false);
                let job = s(r, "known_for_department").as_deref() == Some(dept);
                (same as u8, job as u8)
            };
            let pop = |r: &Value| r["popularity"].as_f64().unwrap_or(0.0);
            score(a).cmp(&score(b)).then(pop(a).partial_cmp(&pop(b)).unwrap_or(std::cmp::Ordering::Equal)).then(std::cmp::Ordering::Greater)
        })
        .map(|(id, _)| id)
}

/// Talk shows, news, reality and award shows: appearances, not roles.
const NOT_ROLES: [u64; 3] = [10767, 10763, 10764];

fn appearance(c: &Value) -> bool {
    let genres = c["genre_ids"].as_array().map(|g| g.iter().filter_map(|x| x.as_u64()).collect::<Vec<_>>()).unwrap_or_default();
    if genres.iter().any(|g| NOT_ROLES.contains(g)) {
        return true;
    }
    let ch = s(c, "character").unwrap_or_default().to_lowercase();
    ch == "self" || ch.starts_with("self ") || ch.starts_with("self -") || ch.contains("himself") || ch.contains("herself") || (ch.contains("(uncredited)") && genres.contains(&99))
}

/// Someone's titles from `/person/{id}?append_to_response=combined_credits`: roles for cast,
/// directing (and creating, for shows) for directors. One entry per title, best known first.
pub fn parse_person(json: &Value, director: bool) -> Option<Person> {
    let tmdb = json["id"].as_u64()?;
    let cc = &json["combined_credits"];
    let mut out: Vec<Credit> = vec![];
    let list: Vec<&Value> = if director {
        cc["crew"].as_array().map(|a| a.iter().filter(|c| matches!(s(c, "job").as_deref(), Some("Director") | Some("Creator"))).collect()).unwrap_or_default()
    } else {
        cc["cast"].as_array().map(|a| a.iter().filter(|c| !appearance(c)).collect()).unwrap_or_default()
    };
    for c in list {
        let typ = match s(c, "media_type").as_deref() {
            Some("movie") => "movie",
            Some("tv") => "series",
            _ => continue,
        };
        let Some(id) = c["id"].as_u64() else { continue };
        let Some(name) = s(c, "title").or_else(|| s(c, "name")) else { continue };
        let role = if director { s(c, "job") } else { s(c, "character") };
        if let Some(have) = out.iter_mut().find(|x| x.tmdb == id && x.typ == typ) {
            // Same title twice (two characters, or directed and created): keep both words.
            if let Some(r) = role.filter(|r| have.role.as_deref().map_or(true, |h| !h.contains(r.as_str()))) {
                have.role = Some(match &have.role { Some(h) => format!("{h} / {r}"), None => r });
            }
            continue;
        }
        let date = s(c, "release_date").or_else(|| s(c, "first_air_date"));
        out.push(Credit {
            tmdb: id,
            typ: typ.into(),
            name,
            year: date.map(|d| d.chars().take(4).collect()),
            poster: img(s(c, "poster_path"), "w342"),
            role,
            votes: c["vote_count"].as_u64().unwrap_or(0),
        });
    }
    // Best known first; titles nobody has rated and with no poster are usually obscure.
    out.retain(|c| c.votes > 0 || c.poster.is_some());
    out.sort_by(|a, b| b.votes.cmp(&a.votes).then_with(|| b.year.cmp(&a.year)));
    out.truncate(120);
    Some(Person {
        tmdb,
        name: s(json, "name").unwrap_or_default(),
        photo: img(s(json, "profile_path"), "w342"),
        known_for: s(json, "known_for_department"),
        born: s(json, "birthday"),
        place: s(json, "place_of_birth"),
        bio: s(json, "biography"),
        credits: out,
    })
}

/// Find someone by name and fetch what they've been in. None = not on TMDB.
pub fn fetch_person(key: &str, name: &str, director: bool) -> Result<Option<Person>, String> {
    let found = get_json(&format!("{API}/search/person?query={}&include_adult=false", enc(name)), key)?;
    let Some(id) = pick_person(&found, name, director) else { return Ok(None) };
    let json = get_json(&format!("{API}/person/{id}?append_to_response=combined_credits"), key)?;
    Ok(parse_person(&json, director))
}

/// The IMDb id of a TMDB movie or show ("movie"/"series"), which addons use. None = TMDB has none.
pub fn imdb_id(key: &str, typ: &str, tmdb: u64) -> Result<Option<String>, String> {
    let kind = if typ == "movie" { "movie" } else { "tv" };
    let json = get_json(&format!("{API}/{kind}/{tmdb}/external_ids?"), key)?;
    Ok(s(&json, "imdb_id").filter(|i| i.starts_with("tt")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn picks_the_right_person() {
        let r = json!({"results": [
            {"id": 1, "name": "Amy Adams", "known_for_department": "Crew", "popularity": 90.0},
            {"id": 2, "name": "Amy Adams", "known_for_department": "Acting", "popularity": 50.0},
            {"id": 3, "name": "Amy Adamson", "known_for_department": "Acting", "popularity": 99.0}]});
        assert_eq!(pick_person(&r, "amy adams", false), Some(2));
        assert_eq!(pick_person(&json!({"results": []}), "x", false), None);
    }

    #[test]
    fn credits_skip_appearances_and_merge() {
        let p = json!({"id": 9, "name": "Amy Adams", "profile_path": "/a.jpg", "known_for_department": "Acting", "combined_credits": {
            "cast": [
                {"id": 1, "media_type": "movie", "title": "Arrival", "release_date": "2016-11-10", "poster_path": "/p.jpg", "character": "Louise Banks", "vote_count": 19000, "genre_ids": [18]},
                {"id": 2, "media_type": "tv", "name": "The Tonight Show", "first_air_date": "2014-02-17", "character": "Self - Guest", "vote_count": 500, "genre_ids": [10767]},
                {"id": 3, "media_type": "movie", "title": "Oscars Doc", "character": "Herself", "vote_count": 10, "genre_ids": [99]},
                {"id": 4, "media_type": "tv", "name": "The Office", "first_air_date": "2005-03-24", "poster_path": "/o.jpg", "character": "Katy", "vote_count": 4000, "genre_ids": [35]},
                {"id": 1, "media_type": "movie", "title": "Arrival", "character": "Narrator", "vote_count": 19000, "genre_ids": [18]},
                {"id": 5, "media_type": "movie", "title": "Unknown short", "vote_count": 0}
            ],
            "crew": [{"id": 7, "media_type": "movie", "title": "Her Film", "job": "Director", "vote_count": 3}]}});
        let person = parse_person(&p, false).unwrap();
        assert_eq!(person.photo.as_deref(), Some("https://image.tmdb.org/t/p/w342/a.jpg"));
        let names: Vec<_> = person.credits.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Arrival", "The Office"]);
        assert_eq!(person.credits[0].role.as_deref(), Some("Louise Banks / Narrator"));
        assert_eq!(person.credits[1].typ, "series");
        assert_eq!(person.credits[0].year.as_deref(), Some("2016"));
        let dir = parse_person(&p, true).unwrap();
        assert_eq!(dir.credits.len(), 1);
        assert_eq!(dir.credits[0].role.as_deref(), Some("Director"));
    }
}
