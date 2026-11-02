//! Stremio-style addons: web addresses that answer three questions in a public JSON format.
//!
//! - `<base>/manifest.json`: who the addon is and what it offers (catalogs, details, streams).
//! - `<base>/catalog/<type>/<id>[/skip=N].json`: a list of titles ("Popular movies").
//! - `<base>/meta/<type>/<id>.json`: one title's details, with episodes for shows.
//! - `<base>/stream/<type>/<id>.json`: where to watch it (used from 0.13).
//!
//! An addon's link often carries the user's own debrid key inside it (TorBox,
//! Real-Debrid...). So the link is a secret: it stays in `addons.json` next to
//! credentials.txt, and nothing here puts it in an error message, a log or anything
//! the interface sees. The interface only gets `key` (a short fingerprint of the link).

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stremio's own public details addon: used for details when no installed addon gives them.
pub const CINEMETA: &str = "https://v3-cinemeta.strem.io";

/// Like JavaScript's encodeURIComponent (what Stremio's own apps use for ids).
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-').remove(b'_').remove(b'.').remove(b'!').remove(b'~')
    .remove(b'*').remove(b'\'').remove(b'(').remove(b')');

fn enc(s: &str) -> String {
    utf8_percent_encode(s, COMPONENT).to_string()
}

/// One installed addon, as saved in addons.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stored {
    /// The full manifest link (may hold the user's key: never show it).
    pub url: String,
    /// The manifest as the addon sent it, so catalogs show without going online.
    pub manifest: Value,
}

impl Stored {
    pub fn key(&self) -> String {
        key_of(&self.url)
    }
    pub fn base(&self) -> String {
        base_of(&self.url)
    }
    pub fn name(&self) -> String {
        str_of(&self.manifest, "name").unwrap_or_else(|| "Addon".into())
    }
}

/// A short, stable fingerprint of a link (FNV-1a), safe to show and to use as an id.
pub fn key_of(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:010x}", h & 0xff_ffff_ffff)
}

/// Turn what the user pasted into the manifest link: `stremio://` becomes `https://`,
/// and a link without `/manifest.json` gets it added.
pub fn normalize(input: &str) -> Result<String, String> {
    let mut u = input.trim().trim_matches('"').to_string();
    if let Some(rest) = u.strip_prefix("stremio://") {
        u = format!("https://{rest}");
    }
    if !(u.starts_with("https://") || u.starts_with("http://")) {
        return Err("That doesn't look like an addon link. It should start with https:// (or stremio://).".into());
    }
    if u.contains(char::is_whitespace) {
        return Err("The link has a space in it. Copy it again and paste the whole thing.".into());
    }
    // Keep any ?query out of the base, but leave it on the manifest link.
    let (path, query) = match u.split_once('?') {
        Some((p, q)) => (p.to_string(), Some(q.to_string())),
        None => (u.clone(), None),
    };
    let path = path.trim_end_matches('/');
    let path = if path.ends_with("/manifest.json") { path.to_string() } else { format!("{path}/manifest.json") };
    Ok(match query {
        Some(q) => format!("{path}?{q}"),
        None => path,
    })
}

/// The addon's base address: the manifest link without `/manifest.json` (and any query).
pub fn base_of(manifest_url: &str) -> String {
    let path = manifest_url.split('?').next().unwrap_or(manifest_url);
    path.trim_end_matches("/manifest.json").trim_end_matches('/').to_string()
}

/// Remove the link (and its base) from a message, so an error never shows the key.
pub fn redact(msg: &str, url: &str) -> String {
    let mut out = msg.replace(url, "(addon link)");
    let base = base_of(url);
    if base.len() > 12 {
        out = out.replace(&base, "(addon link)");
    }
    out
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v.get(k)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => vec![],
    }
}

/// Check a manifest has the basics; returns a plain-words reason if not.
pub fn check_manifest(m: &Value) -> Result<(), String> {
    if !m.is_object() || str_of(m, "id").is_none() {
        return Err("That link didn't give back an addon. Check it's the addon's manifest link.".into());
    }
    if resources(m).is_empty() && catalogs(m).is_empty() {
        return Err("This addon doesn't offer anything Spine can use.".into());
    }
    Ok(())
}

/// What an addon offers: (resource name, types, id prefixes). Resources can be plain
/// names ("meta") that use the manifest's own types/idPrefixes, or objects with their own.
pub fn resources(m: &Value) -> Vec<(String, Vec<String>, Vec<String>)> {
    let types = strings(m.get("types"));
    let prefixes = strings(m.get("idPrefixes"));
    let mut out = vec![];
    if let Some(Value::Array(rs)) = m.get("resources") {
        for r in rs {
            match r {
                Value::String(name) => out.push((name.clone(), types.clone(), prefixes.clone())),
                Value::Object(_) => {
                    let Some(name) = str_of(r, "name") else { continue };
                    let t = if r.get("types").is_some() { strings(r.get("types")) } else { types.clone() };
                    let p = if r.get("idPrefixes").is_some() { strings(r.get("idPrefixes")) } else { prefixes.clone() };
                    out.push((name, t, p));
                }
                _ => {}
            }
        }
    }
    out
}

/// Does this addon answer `resource` (meta, stream) for this type and id?
pub fn serves(m: &Value, resource: &str, typ: &str, id: &str) -> bool {
    resources(m).iter().any(|(name, types, prefixes)| {
        name == resource && types.iter().any(|t| t == typ) && (prefixes.is_empty() || prefixes.iter().any(|p| id.starts_with(p.as_str())))
    })
}

/// A catalog an addon offers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Catalog {
    #[serde(rename = "type")]
    pub typ: String,
    pub id: String,
    pub name: String,
    /// Needs something typed or picked first (a search box, a genre): not browsable as a row.
    pub needs_input: bool,
    /// Can be searched (has a "search" extra).
    pub searchable: bool,
}

pub fn catalogs(m: &Value) -> Vec<Catalog> {
    let Some(Value::Array(cs)) = m.get("catalogs") else { return vec![] };
    cs.iter()
        .filter_map(|c| {
            let typ = str_of(c, "type")?;
            let id = str_of(c, "id")?;
            let name = str_of(c, "name").unwrap_or_else(|| id.clone());
            // Old style: extraRequired: ["search"]; new style: extra: [{name, isRequired}].
            let mut required = strings(c.get("extraRequired"));
            let mut supported = strings(c.get("extraSupported"));
            if let Some(Value::Array(ex)) = c.get("extra") {
                for e in ex {
                    if let Some(n) = str_of(e, "name") {
                        if e.get("isRequired").and_then(|v| v.as_bool()) == Some(true) {
                            required.push(n.clone());
                        }
                        supported.push(n);
                    }
                }
            }
            Some(Catalog {
                needs_input: !required.is_empty(),
                searchable: supported.iter().any(|s| s == "search"),
                typ,
                id,
                name,
            })
        })
        .collect()
}

pub fn catalog_url(base: &str, typ: &str, id: &str, skip: usize) -> String {
    if skip > 0 {
        format!("{base}/catalog/{}/{}/skip={skip}.json", enc(typ), enc(id))
    } else {
        format!("{base}/catalog/{}/{}.json", enc(typ), enc(id))
    }
}

pub fn meta_url(base: &str, typ: &str, id: &str) -> String {
    format!("{base}/meta/{}/{}.json", enc(typ), enc(id))
}

/// One title in a catalog.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub name: String,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub year: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub rating: Option<String>,
    pub runtime: Option<String>,
}

/// One episode of a show.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Episode {
    pub id: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub title: String,
    pub overview: Option<String>,
    pub thumbnail: Option<String>,
    /// Air date, YYYY-MM-DD.
    pub released: Option<String>,
}

/// A title's full details.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Meta {
    #[serde(flatten)]
    pub item: Item,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    pub episodes: Vec<Episode>,
}

fn image(v: &Value, k: &str) -> Option<String> {
    str_of(v, k).filter(|u| u.starts_with("https://") || u.starts_with("http://"))
}

fn item_of(v: &Value, fallback_type: &str) -> Option<Item> {
    let id = str_of(v, "id")?;
    let name = str_of(v, "name").or_else(|| str_of(v, "title"))?;
    let mut genres = strings(v.get("genres"));
    if genres.is_empty() {
        genres = strings(v.get("genre"));
    }
    if genres.is_empty() {
        // Newer addons put genres in links: [{name, category: "Genres"}].
        if let Some(Value::Array(ls)) = v.get("links") {
            genres = ls.iter().filter(|l| str_of(l, "category").as_deref() == Some("Genres")).filter_map(|l| str_of(l, "name")).collect();
        }
    }
    let year = str_of(v, "releaseInfo").or_else(|| str_of(v, "year")).or_else(|| str_of(v, "released").map(|r| r.chars().take(4).collect()));
    Some(Item {
        id,
        typ: str_of(v, "type").unwrap_or_else(|| fallback_type.to_string()),
        name,
        poster: image(v, "poster"),
        background: image(v, "background"),
        logo: image(v, "logo"),
        year: year.map(|y| y.replace('\u{2013}', "–")),
        description: str_of(v, "description"),
        genres,
        rating: str_of(v, "imdbRating").filter(|r| r != "0" && r != "0.0"),
        runtime: str_of(v, "runtime"),
    })
}

pub fn parse_catalog(json: &[u8], typ: &str) -> Result<Vec<Item>, String> {
    let v: Value = serde_json::from_slice(json).map_err(|_| "The addon sent something Spine can't read.".to_string())?;
    let Some(Value::Array(ms)) = v.get("metas") else { return Err("The addon sent something Spine can't read.".into()) };
    let mut seen = std::collections::HashSet::new();
    Ok(ms.iter().filter_map(|m| item_of(m, typ)).filter(|i| seen.insert(i.id.clone())).collect())
}

pub fn parse_meta(json: &[u8], typ: &str) -> Option<Meta> {
    let v: Value = serde_json::from_slice(json).ok()?;
    let m = v.get("meta")?;
    let item = item_of(m, typ)?;
    let num = |e: &Value, k: &str| e.get(k).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).map(|n| n as u32);
    let mut episodes: Vec<Episode> = match m.get("videos") {
        Some(Value::Array(vs)) => vs
            .iter()
            .filter_map(|e| {
                let id = str_of(e, "id")?;
                let season = num(e, "season");
                let episode = num(e, "episode").or_else(|| num(e, "number"));
                let title = str_of(e, "title").or_else(|| str_of(e, "name")).unwrap_or_else(|| match episode {
                    Some(n) => format!("Episode {n}"),
                    None => "Video".into(),
                });
                Some(Episode {
                    id,
                    season,
                    episode,
                    title,
                    overview: str_of(e, "overview").or_else(|| str_of(e, "description")),
                    thumbnail: image(e, "thumbnail"),
                    released: str_of(e, "released").map(|r| r.chars().take(10).collect()),
                })
            })
            .collect(),
        _ => vec![],
    };
    // Specials (season 0) last; otherwise season then episode order.
    episodes.sort_by_key(|e| (e.season.map(|s| if s == 0 { u32::MAX } else { s }).unwrap_or(u32::MAX - 1), e.episode.unwrap_or(0)));
    Some(Meta {
        item,
        cast: strings(m.get("cast")),
        director: strings(m.get("director")),
        episodes,
    })
}

/// Fetch and check a manifest. Errors never contain the link.
pub fn fetch_manifest(url: &str) -> Result<Value, String> {
    let bytes = crate::covers::curl(url, true, &[]).map_err(|e| friendly(&redact(&e, url)))?;
    let m: Value = serde_json::from_slice(&bytes).map_err(|_| "That link didn't give back an addon. Check it's the addon's manifest link.".to_string())?;
    check_manifest(&m)?;
    Ok(m)
}

pub fn fetch_catalog(a: &Stored, typ: &str, id: &str, skip: usize) -> Result<Vec<Item>, String> {
    let url = catalog_url(&a.base(), typ, id, skip);
    let bytes = crate::covers::curl(&url, true, &[]).map_err(|e| format!("{}: {}", a.name(), friendly(&redact(&redact(&e, &url), &a.url))))?;
    parse_catalog(&bytes, typ).map_err(|e| format!("{}: {e}", a.name()))
}

/// Details for a title: first the addon it came from, then any other addon that gives
/// details for it, then Cinemeta (IMDb ids). None if nobody knows it.
pub fn fetch_meta(addons: &[Stored], from: Option<&str>, typ: &str, id: &str) -> Option<Meta> {
    let mut order: Vec<&Stored> = addons.iter().filter(|a| Some(a.key().as_str()) == from).collect();
    order.extend(addons.iter().filter(|a| Some(a.key().as_str()) != from));
    for a in order {
        if !serves(&a.manifest, "meta", typ, id) {
            continue;
        }
        if let Ok(b) = crate::covers::curl(&meta_url(&a.base(), typ, id), true, &[]) {
            if let Some(m) = parse_meta(&b, typ) {
                return Some(m);
            }
        }
    }
    if id.starts_with("tt") {
        if let Ok(b) = crate::covers::curl(&meta_url(CINEMETA, typ, id), true, &[]) {
            return parse_meta(&b, typ);
        }
    }
    None
}

/// Network errors in plain words.
fn friendly(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("401") || l.contains("403") {
        "the addon refused (is your key in its link still valid?)".into()
    } else if l.contains("404") {
        "the addon doesn't have that (link out of date?)".into()
    } else if l.contains("resolve") || l.contains("dns") || l.contains("connect") || l.contains("timed out") || l.contains("timeout") {
        "couldn't reach the addon (check your internet connection)".into()
    } else {
        e.chars().take(160).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Talks to Stremio's real Cinemeta addon: `cargo test -- --ignored live_cinemeta`.
    #[test]
    #[ignore]
    fn live_cinemeta() {
        let url = normalize("https://v3-cinemeta.strem.io").unwrap();
        let m = fetch_manifest(&url).unwrap();
        let a = Stored { url, manifest: m };
        let cats = catalogs(&a.manifest);
        assert!(cats.iter().any(|c| c.typ == "movie" && !c.needs_input), "{cats:?}");
        let top = fetch_catalog(&a, "series", "top", 0).unwrap();
        assert!(top.len() >= 20, "{}", top.len());
        let more = fetch_catalog(&a, "series", "top", top.len()).unwrap();
        assert!(!more.is_empty() && more[0].id != top[0].id);
        let bb = fetch_meta(&[a], None, "series", "tt0903747").unwrap();
        assert_eq!(bb.item.name, "Breaking Bad");
        assert!(bb.episodes.len() > 60, "{}", bb.episodes.len());
        assert_eq!((bb.episodes[0].season, bb.episodes[0].episode), (Some(1), Some(1)));
        println!("{} top shows; first: {:?}; BB poster {:?}", top.len(), top[0].name, bb.item.poster);
    }

    #[test]
    fn links_are_cleaned_up() {
        assert_eq!(normalize("stremio://addon.example.com/abc/manifest.json").unwrap(), "https://addon.example.com/abc/manifest.json");
        assert_eq!(normalize(" https://a.example.com/cfg/ ").unwrap(), "https://a.example.com/cfg/manifest.json");
        assert_eq!(normalize("https://a.example.com/manifest.json?x=1").unwrap(), "https://a.example.com/manifest.json?x=1");
        assert!(normalize("addon.example.com").is_err());
        assert_eq!(base_of("https://a.example.com/key123/manifest.json"), "https://a.example.com/key123");
        assert_eq!(base_of("https://a.example.com/manifest.json?x=1"), "https://a.example.com");
    }

    #[test]
    fn errors_never_show_the_link() {
        let url = "https://addon.example.com/torbox=SECRETKEY123/manifest.json";
        let e = format!("https://addon.example.com/torbox=SECRETKEY123/catalog/movie/top.json: status code 500");
        let r = redact(&e, url);
        assert!(!r.contains("SECRETKEY123"), "{r}");
        assert_eq!(key_of(url), key_of(url));
        assert!(!key_of(url).contains("SECRET"));
        assert_eq!(key_of(url).len(), 10);
    }

    #[test]
    fn urls_encode_ids() {
        assert_eq!(catalog_url("https://a.b", "movie", "top", 0), "https://a.b/catalog/movie/top.json");
        assert_eq!(catalog_url("https://a.b", "series", "my list", 100), "https://a.b/catalog/series/my%20list/skip=100.json");
        assert_eq!(meta_url("https://a.b", "series", "kitsu:42"), "https://a.b/meta/series/kitsu%3A42.json");
    }

    fn sample_manifest() -> Value {
        serde_json::json!({
            "id": "org.example.addon", "version": "1.2.0", "name": "Example",
            "types": ["movie", "series"], "idPrefixes": ["tt"],
            "resources": ["catalog", "meta", {"name": "stream", "types": ["movie", "series"], "idPrefixes": ["tt", "kitsu"]}],
            "catalogs": [
                {"type": "movie", "id": "top", "name": "Popular", "extra": [{"name": "skip"}, {"name": "search"}]},
                {"type": "series", "id": "search", "name": "Search", "extra": [{"name": "search", "isRequired": true}]},
                {"type": "movie", "id": "genre", "name": "By genre", "extraRequired": ["genre"], "extraSupported": ["genre"]}
            ]
        })
    }

    #[test]
    fn reads_what_an_addon_offers() {
        let m = sample_manifest();
        assert!(check_manifest(&m).is_ok());
        assert!(check_manifest(&serde_json::json!({"name": "x"})).is_err());
        let c = catalogs(&m);
        assert_eq!(c.len(), 3);
        assert!(!c[0].needs_input && c[0].searchable);
        assert!(c[1].needs_input && c[1].searchable);
        assert!(c[2].needs_input && !c[2].searchable);
        assert!(serves(&m, "meta", "movie", "tt0111161"));
        assert!(!serves(&m, "meta", "movie", "kitsu:1"));
        assert!(serves(&m, "stream", "series", "kitsu:1:2"));
        assert!(!serves(&m, "stream", "channel", "tt1"));
    }

    #[test]
    fn reads_catalogs_and_details() {
        let cat = br#"{"metas":[
            {"id":"tt0111161","type":"movie","name":"The Shawshank Redemption","poster":"https://img.example/p.jpg","releaseInfo":"1994","imdbRating":"9.3","genres":["Drama"]},
            {"id":"tt0111161","type":"movie","name":"Duplicate"},
            {"id":"tt2","name":"No type","poster":"not a url","links":[{"name":"Comedy","category":"Genres"},{"name":"Someone","category":"Cast"}]},
            {"name":"No id"}]}"#;
        let items = parse_catalog(cat, "movie").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].year.as_deref(), Some("1994"));
        assert_eq!(items[0].rating.as_deref(), Some("9.3"));
        assert_eq!(items[1].typ, "movie");
        assert_eq!(items[1].poster, None);
        assert_eq!(items[1].genres, vec!["Comedy"]);
        assert!(parse_catalog(b"<html>", "movie").is_err());

        let meta = r#"{"meta":{"id":"tt0903747","type":"series","name":"Breaking Bad","releaseInfo":"2008–2013","description":"A teacher...","cast":["Bryan Cranston"],
            "videos":[
              {"id":"tt0903747:2:1","title":"Seven Thirty-Seven","season":2,"episode":1,"released":"2009-03-08T00:00:00.000Z","thumbnail":"https://img.example/s2e1.jpg"},
              {"id":"tt0903747:0:1","name":"Special","season":0,"episode":1},
              {"id":"tt0903747:1:2","season":1,"episode":2,"overview":"Cat's in the bag"},
              {"id":"tt0903747:1:1","title":"Pilot","season":"1","episode":"1"}]}}"#;
        let m = parse_meta(meta.as_bytes(), "series").unwrap();
        assert_eq!(m.item.year.as_deref(), Some("2008–2013"));
        let order: Vec<_> = m.episodes.iter().map(|e| (e.season, e.episode)).collect();
        assert_eq!(order, vec![(Some(1), Some(1)), (Some(1), Some(2)), (Some(2), Some(1)), (Some(0), Some(1))]);
        assert_eq!(m.episodes[1].title, "Episode 2");
        assert_eq!(m.episodes[2].released.as_deref(), Some("2009-03-08"));
        assert_eq!(m.cast, vec!["Bryan Cranston"]);
    }
}
