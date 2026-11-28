//! Built-in TMDB streaming catalogs, clean search, and fallback metadata.
//!
//! Uses the user's existing TMDB key (`credentials.txt`) to give the Streaming screen
//! clean search (without IMDb fan-clip / episode-stub noise) and dedicated Western
//! Cartoons and Anime catalogs. Titles carry `tmdb:<id>` until opened, when Spine
//! resolves the IMDb `tt…` id so existing source addons work unchanged.

use crate::addons::{Catalog, Episode, Item, Meta};
use crate::movies::{get_json, API, IMG};
use crate::title;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;

pub const ADDON_KEY: &str = "tmdb";

/// Major Western animation origin countries (OR-joined for TMDB discover).
const WESTERN_COUNTRIES: &str = "US%7CCA%7CGB%7CFR%7CAU%7CIE";

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty()).map(String::from)
}

fn img(path: Option<String>, size: &str) -> Option<String> {
    path.map(|p| format!("{IMG}{size}{p}"))
}

/// Built-in catalogs shown on the Streaming screen when a TMDB key is configured.
pub fn catalogs() -> Vec<Catalog> {
    let tv_genres = |xs: &[&str]| xs.iter().map(|x| (*x).to_string()).collect();
    vec![
        Catalog {
            typ: "series".into(),
            id: "cartoons".into(),
            name: "Cartoons".into(),
            needs_input: false,
            searchable: false,
            genres: tv_genres(&["Action & Adventure", "Comedy", "Sci-Fi & Fantasy", "Kids", "Family", "Mystery", "Drama"]),
            genre_required: false,
        },
        Catalog {
            typ: "series".into(),
            id: "anime".into(),
            name: "Anime".into(),
            needs_input: false,
            searchable: false,
            genres: tv_genres(&["Action & Adventure", "Comedy", "Sci-Fi & Fantasy", "Drama", "Mystery", "Kids"]),
            genre_required: false,
        },
        Catalog {
            typ: "series".into(),
            id: "search".into(),
            name: "Search".into(),
            needs_input: true,
            searchable: true,
            genres: vec![],
            genre_required: false,
        },
        Catalog {
            typ: "movie".into(),
            id: "cartoon-movies".into(),
            name: "Animated".into(),
            needs_input: false,
            searchable: false,
            genres: tv_genres(&["Action", "Adventure", "Comedy", "Family", "Fantasy", "Science Fiction", "Drama"]),
            genre_required: false,
        },
        Catalog {
            typ: "movie".into(),
            id: "anime-movies".into(),
            name: "Anime".into(),
            needs_input: false,
            searchable: false,
            genres: tv_genres(&["Action", "Adventure", "Comedy", "Drama", "Fantasy", "Science Fiction", "Romance"]),
            genre_required: false,
        },
        Catalog {
            typ: "movie".into(),
            id: "search".into(),
            name: "Search".into(),
            needs_input: true,
            searchable: true,
            genres: vec![],
            genre_required: false,
        },
    ]
}

fn tv_genre_id(name: &str) -> Option<u32> {
    match name {
        "Action & Adventure" => Some(10759),
        "Animation" => Some(16),
        "Comedy" => Some(35),
        "Crime" => Some(80),
        "Documentary" => Some(99),
        "Drama" => Some(18),
        "Family" => Some(10751),
        "Kids" => Some(10762),
        "Mystery" => Some(9648),
        "Sci-Fi & Fantasy" => Some(10765),
        _ => None,
    }
}

fn movie_genre_id(name: &str) -> Option<u32> {
    match name {
        "Action" => Some(28),
        "Adventure" => Some(12),
        "Animation" => Some(16),
        "Comedy" => Some(35),
        "Crime" => Some(80),
        "Documentary" => Some(99),
        "Drama" => Some(18),
        "Family" => Some(10751),
        "Fantasy" => Some(14),
        "Horror" => Some(27),
        "Mystery" => Some(9648),
        "Romance" => Some(10749),
        "Science Fiction" => Some(878),
        "Thriller" => Some(53),
        _ => None,
    }
}

fn genre_name(id: u64) -> Option<&'static str> {
    match id {
        12 => Some("Adventure"),
        14 => Some("Fantasy"),
        16 => Some("Animation"),
        18 => Some("Drama"),
        27 => Some("Horror"),
        28 => Some("Action"),
        35 => Some("Comedy"),
        36 => Some("History"),
        37 => Some("Western"),
        53 => Some("Thriller"),
        80 => Some("Crime"),
        99 => Some("Documentary"),
        878 => Some("Science Fiction"),
        9648 => Some("Mystery"),
        10402 => Some("Music"),
        10749 => Some("Romance"),
        10751 => Some("Family"),
        10752 => Some("War"),
        10759 => Some("Action & Adventure"),
        10762 => Some("Kids"),
        10763 => Some("News"),
        10764 => Some("Reality"),
        10765 => Some("Sci-Fi & Fantasy"),
        10766 => Some("Soap"),
        10767 => Some("Talk"),
        10768 => Some("War & Politics"),
        _ => None,
    }
}

fn country_name(code: &str) -> &str {
    match code {
        "JP" => "Japan",
        "US" => "United States",
        "GB" => "United Kingdom",
        "CA" => "Canada",
        "FR" => "France",
        "AU" => "Australia",
        "IE" => "Ireland",
        "DE" => "Germany",
        "ES" => "Spain",
        "KR" => "South Korea",
        "CN" => "China",
        other => other,
    }
}

fn countries_of(v: &Value, fallback: Option<&str>) -> Option<String> {
    if let Some(arr) = v["origin_country"].as_array() {
        let names: Vec<&str> = arr.iter().filter_map(|c| c.as_str()).map(country_name).collect();
        if !names.is_empty() {
            return Some(names.join(", "));
        }
    }
    if let Some(arr) = v["production_countries"].as_array() {
        let names: Vec<String> = arr.iter().filter_map(|c| s(c, "name")).collect();
        if !names.is_empty() {
            return Some(names.join(", "));
        }
    }
    fallback.map(String::from)
}

fn item_of(r: &Value, typ: &str, fallback_country: Option<&str>) -> Option<Item> {
    let id = r["id"].as_u64()?;
    let name = s(r, "title").or_else(|| s(r, "name"))?;
    let date = s(r, "release_date").or_else(|| s(r, "first_air_date"));
    let genres: Vec<String> = r["genre_ids"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_u64().and_then(genre_name)).map(String::from).collect())
        .unwrap_or_default();
    let votes = r["vote_count"].as_u64().unwrap_or(0);
    let rating = r["vote_average"].as_f64().filter(|x| *x > 0.0 && votes >= 5).map(|x| format!("{x:.1}"));
    Some(Item {
        id: format!("tmdb:{id}"),
        typ: typ.to_string(),
        name,
        poster: img(s(r, "poster_path"), "w342"),
        background: img(s(r, "backdrop_path"), "w1280"),
        logo: None,
        year: date.and_then(|d| d.get(..4).map(String::from)),
        description: s(r, "overview"),
        genres,
        rating,
        runtime: None,
        country: countries_of(r, fallback_country),
    })
}

/// Build the TMDB discover URL for one of Spine's built-in animation catalogs.
pub fn catalog_url(typ: &str, id: &str, skip: usize, genre: Option<&str>) -> Option<(String, Option<&'static str>)> {
    let page = if skip == 0 { 1 } else { (skip - 1) / 20 + 2 };
    if page > 500 {
        return None;
    }
    let is_movie = typ == "movie";
    let (endpoint, countries, min_votes, fallback_country) = match (typ, id) {
        ("series", "cartoons") => ("tv", WESTERN_COUNTRIES, 15, None),
        ("series", "anime") => ("tv", "JP", 15, Some("Japan")),
        ("movie", "cartoon-movies") => ("movie", WESTERN_COUNTRIES, 25, None),
        ("movie", "anime-movies") => ("movie", "JP", 25, Some("Japan")),
        _ => return None,
    };
    let extra_genre = genre.and_then(|g| if is_movie { movie_genre_id(g) } else { tv_genre_id(g) }).filter(|g| *g != 16);
    let genres_param = match extra_genre {
        Some(g) => format!("16,{g}"),
        None => "16".to_string(),
    };
    let url = format!(
        "{API}/discover/{endpoint}?with_genres={genres_param}&with_origin_country={countries}&sort_by=popularity.desc&vote_count.gte={min_votes}&include_adult=false&page={page}"
    );
    Some((url, fallback_country))
}

pub fn parse_discover(json: &Value, typ: &str, fallback_country: Option<&str>) -> Vec<Item> {
    let Some(results) = json["results"].as_array() else { return vec![] };
    let mut seen = std::collections::HashSet::new();
    results
        .iter()
        .filter_map(|r| item_of(r, typ, fallback_country))
        .filter(|i| seen.insert(i.id.clone()))
        .collect()
}

/// Fetch one page of a built-in TMDB catalog.
pub fn fetch_catalog(key: &str, typ: &str, id: &str, skip: usize, genre: Option<&str>) -> Result<Vec<Item>, String> {
    let Some((url, fallback_country)) = catalog_url(typ, id, skip, genre) else { return Ok(vec![]) };
    let json = get_json(&url, key)?;
    Ok(parse_discover(&json, typ, fallback_country))
}

/// Parse and rank TMDB search results: drop unrated posterless stubs and bubble exact
/// and prefix title matches with real popularity/votes to the top.
pub fn parse_search(json: &Value, typ: &str, query: &str) -> Vec<Item> {
    let Some(results) = json["results"].as_array() else { return vec![] };
    let want = title::match_key(query);
    let postered = results.iter().filter(|r| s(r, "poster_path").is_some()).count();

    let mut scored: Vec<(u8, u8, u64, u64, Item)> = vec![];
    let mut seen = std::collections::HashSet::new();
    for r in results {
        let votes = r["vote_count"].as_u64().unwrap_or(0);
        let has_poster = s(r, "poster_path").is_some();
        if !has_poster && votes == 0 {
            continue;
        }
        if postered >= 3 && !has_poster && votes < 5 {
            continue;
        }
        let Some(item) = item_of(r, typ, None) else { continue };
        if !seen.insert(item.id.clone()) {
            continue;
        }
        let names = [
            title::match_key(&item.name),
            s(r, "original_title").or_else(|| s(r, "original_name")).map(|n| title::match_key(&n)).unwrap_or_default(),
        ];
        let tier = if !want.is_empty() && names.iter().any(|n| n == &want) {
            3
        } else if !want.is_empty() && names.iter().any(|n| n.starts_with(&want)) {
            2
        } else if !want.is_empty() && names.iter().any(|n| n.contains(&want)) {
            1
        } else {
            0
        };
        let pop = (r["popularity"].as_f64().unwrap_or(0.0) * 100.0).max(0.0) as u64;
        scored.push((tier, has_poster as u8, votes, pop, item));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)).then(b.3.cmp(&a.3)));
    scored.into_iter().map(|x| x.4).collect()
}

/// Clean search against TMDB (`/search/movie` or `/search/tv`).
pub fn fetch_search(key: &str, typ: &str, query: &str) -> Result<Vec<Item>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let endpoint = if typ == "movie" { "movie" } else { "tv" };
    let url = format!("{API}/search/{endpoint}?query={}&include_adult=false&page=1", enc(q));
    let json = get_json(&url, key)?;
    Ok(parse_search(&json, typ, q))
}

fn pick_logo(json: &Value) -> Option<String> {
    let logos = json["images"]["logos"].as_array()?;
    let rank = |l: &Value| {
        let lang = l["iso_639_1"].as_str();
        let png = l["file_path"].as_str().map_or(false, |p| p.ends_with(".png"));
        (if lang == Some("en") { 0 } else if lang.is_none() { 1 } else { 2 }, !png as u8)
    };
    logos.iter().filter(|l| rank(l).0 < 2).min_by_key(|l| rank(l)).and_then(|l| img(s(l, "file_path"), "w500"))
}

pub fn parse_meta(json: &Value, typ: &str, tmdb_id: u64, episodes: Vec<Episode>) -> Option<Meta> {
    let name = s(json, "title").or_else(|| s(json, "name"))?;
    let imdb = s(&json["external_ids"], "imdb_id").filter(|i| i.starts_with("tt"));
    let id = imdb.unwrap_or_else(|| format!("tmdb:{tmdb_id}"));
    let genres: Vec<String> = json["genres"].as_array().map(|a| a.iter().filter_map(|g| s(g, "name")).collect()).unwrap_or_default();
    let votes = json["vote_count"].as_u64().unwrap_or(0);
    let rating = json["vote_average"].as_f64().filter(|x| *x > 0.0 && votes >= 5).map(|x| format!("{x:.1}"));
    let year = if typ == "movie" {
        s(json, "release_date").and_then(|d| d.get(..4).map(String::from))
    } else {
        let start = s(json, "first_air_date").and_then(|d| d.get(..4).map(String::from));
        let end = s(json, "last_air_date").and_then(|d| d.get(..4).map(String::from));
        let ended = matches!(s(json, "status").as_deref(), Some("Ended") | Some("Canceled"));
        match (start, end) {
            (Some(a), Some(b)) if ended && b != a => Some(format!("{a}–{b}")),
            (Some(a), _) if !ended => Some(format!("{a}–")),
            (Some(a), _) => Some(a),
            _ => None,
        }
    };
    let runtime = json["runtime"]
        .as_u64()
        .or_else(|| json["episode_run_time"].as_array().and_then(|a| a.first()).and_then(|v| v.as_u64()))
        .filter(|m| *m > 0)
        .map(|m| format!("{m} min"));
    let cast: Vec<String> = json["credits"]["cast"].as_array().map(|a| a.iter().filter_map(|c| s(c, "name")).take(10).collect()).unwrap_or_default();
    let mut director: Vec<String> = json["credits"]["crew"]
        .as_array()
        .map(|a| a.iter().filter(|c| s(c, "job").as_deref() == Some("Director")).filter_map(|c| s(c, "name")).take(4).collect())
        .unwrap_or_default();
    if director.is_empty() {
        director = json["created_by"].as_array().map(|a| a.iter().filter_map(|c| s(c, "name")).take(4).collect()).unwrap_or_default();
    }
    let item = Item {
        id,
        typ: typ.to_string(),
        name,
        poster: img(s(json, "poster_path"), "w500"),
        background: img(s(json, "backdrop_path"), "w1280"),
        logo: pick_logo(json),
        year,
        description: s(json, "overview"),
        genres,
        rating,
        runtime,
        country: countries_of(json, None),
    };
    Some(Meta { item, cast, director, episodes })
}

fn parse_season_episodes(json: &Value, base_id: &str) -> Vec<Episode> {
    let Some(eps) = json["episodes"].as_array() else { return vec![] };
    eps.iter()
        .filter_map(|e| {
            let season = e["season_number"].as_u64().map(|n| n as u32);
            let episode = e["episode_number"].as_u64().map(|n| n as u32)?;
            let s_num = season.unwrap_or(1);
            let title = s(e, "name").unwrap_or_else(|| format!("Episode {episode}"));
            Some(Episode {
                id: format!("{base_id}:{s_num}:{episode}"),
                season,
                episode: Some(episode),
                title,
                overview: s(e, "overview"),
                thumbnail: img(s(e, "still_path"), "w300"),
                released: s(e, "air_date").map(|d| d.chars().take(10).collect()),
            })
        })
        .collect()
}

/// Full details (and episodes for shows) from TMDB, used for `tmdb:` ids and as a
/// fallback when Cinemeta has no metadata for an IMDb id.
pub fn fetch_meta(key: &str, typ: &str, tmdb_id: u64) -> Option<Meta> {
    let endpoint = if typ == "movie" { "movie" } else { "tv" };
    let url = format!("{API}/{endpoint}/{tmdb_id}?append_to_response=credits,images,external_ids");
    let json = get_json(&url, key).ok()?;
    let imdb = s(&json["external_ids"], "imdb_id").filter(|i| i.starts_with("tt"));
    let base_id = imdb.unwrap_or_else(|| format!("tmdb:{tmdb_id}"));

    let mut episodes = vec![];
    if typ != "movie" {
        let season_nums: Vec<u32> = json["seasons"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|sn| sn["season_number"].as_u64().map(|n| n as u32))
                    .take(50)
                    .collect()
            })
            .unwrap_or_default();
        let chunks: Vec<Vec<Episode>> = std::thread::scope(|sc| {
            let jobs: Vec<_> = season_nums
                .iter()
                .map(|&sn| {
                    let base = base_id.as_str();
                    sc.spawn(move || {
                        let surl = format!("{API}/tv/{tmdb_id}/season/{sn}?");
                        get_json(&surl, key).ok().map(|sj| parse_season_episodes(&sj, base)).unwrap_or_default()
                    })
                })
                .collect();
            jobs.into_iter().filter_map(|j| j.join().ok()).collect()
        });
        for mut c in chunks {
            episodes.append(&mut c);
        }
        episodes.sort_by_key(|e| (e.season.map(|sn| if sn == 0 { u32::MAX } else { sn }).unwrap_or(u32::MAX - 1), e.episode.unwrap_or(0)));
    }
    parse_meta(&json, typ, tmdb_id, episodes)
}

/// Look up a TMDB id from an IMDb `tt…` id and return TMDB's metadata.
pub fn fetch_meta_by_imdb(key: &str, typ: &str, imdb_id: &str) -> Option<Meta> {
    let url = format!("{API}/find/{}?external_source=imdb_id", enc(imdb_id));
    let json = get_json(&url, key).ok()?;
    let bucket = if typ == "movie" { "movie_results" } else { "tv_results" };
    let tmdb_id = json[bucket].as_array()?.first()?["id"].as_u64()?;
    fetch_meta(key, typ, tmdb_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_animation_discover_urls() {
        let (u, c) = catalog_url("series", "cartoons", 0, None).unwrap();
        assert!(u.contains("/discover/tv?with_genres=16&with_origin_country=US%7CCA"), "{u}");
        assert!(u.contains("page=1"), "{u}");
        assert_eq!(c, None);

        let (u2, c2) = catalog_url("series", "anime", 20, Some("Comedy")).unwrap();
        assert!(u2.contains("with_genres=16,35"), "{u2}");
        assert!(u2.contains("with_origin_country=JP"), "{u2}");
        assert!(u2.contains("page=2"), "{u2}");
        assert_eq!(c2, Some("Japan"));

        let (u3, _) = catalog_url("movie", "cartoon-movies", 18, Some("Family")).unwrap();
        assert!(u3.contains("/discover/movie?with_genres=16,10751"), "{u3}");
        assert!(u3.contains("page=2"), "{u3}");

        assert!(catalog_url("series", "unknown", 0, None).is_none());
    }

    #[test]
    fn search_drops_stubs_and_ranks_exact_first() {
        let resp = json!({"results": [
            {"id": 1, "name": "Adventure Time ep 2, Seeing my bestfriend", "vote_count": 0, "popularity": 1.0},
            {"id": 2, "name": "Adventure Time: Distant Lands", "poster_path": "/dl.jpg", "first_air_date": "2020-06-25", "genre_ids": [16, 10759], "origin_country": ["US"], "vote_count": 180, "vote_average": 8.4, "popularity": 25.0},
            {"id": 3, "name": "Adventure Time", "poster_path": "/at.jpg", "first_air_date": "2010-04-05", "genre_ids": [16, 35], "origin_country": ["US"], "vote_count": 2800, "vote_average": 8.5, "popularity": 90.0},
            {"id": 4, "name": "Adventure Time: Fionna & Cake", "poster_path": "/fc.jpg", "first_air_date": "2023-08-31", "genre_ids": [16, 10765], "origin_country": ["US"], "vote_count": 320, "vote_average": 8.7, "popularity": 40.0},
            {"id": 5, "name": "Clip: roblox adventure time", "vote_count": 2, "popularity": 0.5}
        ]});
        let items = parse_search(&resp, "series", "Adventure Time");
        let names: Vec<_> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Adventure Time", "Adventure Time: Fionna & Cake", "Adventure Time: Distant Lands"]);
        assert_eq!(items[0].id, "tmdb:3");
        assert_eq!(items[0].year.as_deref(), Some("2010"));
        assert_eq!(items[0].country.as_deref(), Some("United States"));
        assert_eq!(items[0].genres, vec!["Animation", "Comedy"]);
        assert_eq!(items[0].rating.as_deref(), Some("8.5"));
    }

    #[test]
    fn parses_meta_and_episodes() {
        let show = json!({
            "id": 15260, "name": "Adventure Time", "first_air_date": "2010-04-05", "last_air_date": "2018-09-03",
            "status": "Ended", "overview": "Finn and Jake.", "poster_path": "/p.jpg", "backdrop_path": "/b.jpg",
            "vote_count": 2800, "vote_average": 8.5, "episode_run_time": [11], "origin_country": ["US"],
            "genres": [{"id": 16, "name": "Animation"}],
            "created_by": [{"name": "Pendleton Ward"}],
            "credits": {"cast": [{"name": "Jeremy Shada"}, {"name": "John DiMaggio"}], "crew": []},
            "external_ids": {"imdb_id": "tt1305826"},
            "images": {"logos": [{"iso_639_1": "en", "file_path": "/logo.png"}]}
        });
        let season = json!({"episodes": [
            {"season_number": 6, "episode_number": 1, "name": "Wake Up", "overview": "Cosmic crime.", "still_path": "/s6e1.jpg", "air_date": "2014-04-21"}
        ]});
        let eps = parse_season_episodes(&season, "tt1305826");
        let m = parse_meta(&show, "series", 15260, eps).unwrap();
        assert_eq!(m.item.id, "tt1305826");
        assert_eq!(m.item.year.as_deref(), Some("2010–2018"));
        assert_eq!(m.item.runtime.as_deref(), Some("11 min"));
        assert_eq!(m.director, vec!["Pendleton Ward"]);
        assert_eq!(m.cast, vec!["Jeremy Shada", "John DiMaggio"]);
        assert_eq!(m.episodes.len(), 1);
        assert_eq!(m.episodes[0].id, "tt1305826:6:1");
        assert_eq!(m.episodes[0].title, "Wake Up");
    }
}
