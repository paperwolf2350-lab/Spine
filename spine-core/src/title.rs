//! Turning messy file names into clean titles.
//!
//! "03. Pokemon - Emerald Version (USA, Europe).gba"
//!     -> title "Pokemon: Emerald Version", region "USA, Europe"
//! "Legend of Zelda, The - Twilight Princess HD (USA) (En,Fr,Es) (Rev 2).wux"
//!     -> title "The Legend of Zelda: Twilight Princess HD", rev "2"
//! "Pokémon Legends Z-A [0100F43008C44000][v1.0.0][US][Base][eShop].nsp"
//!     -> title "Pokémon Legends Z-A", title_id "0100F43008C44000"

use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ParsedName {
    /// Clean display title.
    pub title: String,
    /// Region text as written, e.g. "USA, Europe".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Revision or version, e.g. "2" for (Rev 2), "1.0.0" for [v1.0.0].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Disc number for multi-disc games.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc: Option<u32>,
    /// Console title id (Switch 16 hex digits, Wii U 16 hex digits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_id: Option<String>,
    /// What kind of content the name says this is.
    pub kind: ContentKind,
    /// Beta, prototype, demo, sample or a known-bad dump ([b]).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub prerelease: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    #[default]
    Game,
    Update,
    Dlc,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

const REGION_WORDS: &[&str] = &[
    "usa", "us", "europe", "eu", "eur", "japan", "jp", "jpn", "world", "asia", "australia", "korea",
    "brazil", "canada", "china", "france", "germany", "spain", "italy", "netherlands", "sweden",
    "uk", "russia", "taiwan", "hong kong", "scandinavia", "latin america", "ntsc", "pal", "ntsc-u", "ntsc-j",
];

const LANG_CODES: &[&str] = &[
    "en", "fr", "de", "es", "it", "nl", "pt", "sv", "no", "da", "fi", "ja", "ko", "zh", "ru", "pl",
    "cs", "hu", "el", "tr", "ar", "ca", "hant", "hans",
];

fn is_region(tag: &str) -> bool {
    let parts: Vec<String> = tag.split(',').map(|p| p.trim().to_ascii_lowercase()).collect();
    !parts.is_empty() && parts.iter().all(|p| REGION_WORDS.contains(&p.as_str()))
}

fn is_language_list(tag: &str) -> bool {
    // "En,Fr,Es" or "EnFrDeEsItPtRu" (some dumps squash them together)
    let t = tag.trim().to_ascii_lowercase();
    if t.contains(',') || t.contains('+') {
        return t.split([',', '+']).all(|p| {
            let p = p.trim();
            LANG_CODES.contains(&p) || p.split('-').next().map_or(false, |b| LANG_CODES.contains(&b))
        });
    }
    if t.len() >= 4 && t.len() % 2 == 0 && t.chars().all(|c| c.is_ascii_alphabetic()) {
        return t.as_bytes().chunks(2).all(|c| LANG_CODES.contains(&std::str::from_utf8(c).unwrap()));
    }
    LANG_CODES.contains(&t.as_str()) && t.len() == 2 && tag.chars().next().map_or(false, |c| c.is_uppercase())
}

/// Parse a file or folder name (with or without extension).
pub fn parse_name(raw: &str, strip_ext: bool) -> ParsedName {
    static RANK: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    static REV: OnceLock<Regex> = OnceLock::new();
    static VER: OnceLock<Regex> = OnceLock::new();
    static DISC: OnceLock<Regex> = OnceLock::new();
    static HEXID: OnceLock<Regex> = OnceLock::new();
    static SPACES: OnceLock<Regex> = OnceLock::new();

    let mut s = raw.trim().to_string();
    if strip_ext {
        if let Some(dot) = s.rfind('.') {
            // Only strip a real extension (1-6 chars, no spaces): keeps "Mario Bros. U" intact.
            let ext = &s[dot + 1..];
            if !ext.is_empty() && ext.len() <= 6 && !ext.contains(' ') {
                s.truncate(dot);
            }
        }
    }

    // "01. Title", "05.The Legend...", "3) Title" — chart rankings people prefix.
    // A dash is NOT a rank separator, so "007 - Nightfire" survives.
    let rank = re(&RANK, r"^\s*\d{1,3}\s*[.)]\s*");
    s = rank.replace(&s, "").into_owned();

    let mut out = ParsedName::default();
    let tag = re(&TAG, r"\(([^()]*)\)|\[([^\[\]]*)\]");
    let rev = re(&REV, r"(?i)^rev\s*([0-9a-z.]+)$");
    let ver = re(&VER, r"(?i)^v\s*([0-9][0-9a-z.]*)$");
    let disc = re(&DISC, r"(?i)^(?:disc|disk|cd)\s*([0-9]+)(?:\s*of\s*[0-9]+)?$");
    let hexid = re(&HEXID, r"(?i)^[0-9a-f]{15,16}$");

    for cap in tag.captures_iter(&s.clone()) {
        let inner = cap.get(1).or(cap.get(2)).map(|m| m.as_str().trim()).unwrap_or("");
        let low = inner.to_ascii_lowercase();
        if inner.is_empty() {
            continue;
        }
        if hexid.is_match(inner) {
            out.title_id = Some(inner.to_ascii_uppercase());
        } else if let Some(c) = rev.captures(inner) {
            out.revision = Some(c[1].to_string());
        } else if let Some(c) = ver.captures(inner) {
            out.revision = Some(c[1].to_string());
        } else if let Some(c) = disc.captures(inner) {
            out.disc = c[1].parse().ok();
        } else if is_region(inner) {
            out.region.get_or_insert_with(|| inner.to_string());
        } else if low == "update" || low.starts_with("update ") {
            out.kind = ContentKind::Update;
        } else if low == "dlc" || low.starts_with("dlc ") {
            out.kind = ContentKind::Dlc;
        } else if matches!(low.as_str(), "b" | "beta" | "proto" | "prototype" | "demo" | "sample" | "kiosk" | "preview")
            || low.starts_with("beta ") || low.starts_with("proto ") || low.starts_with("demo ")
        {
            out.prerelease = true;
        }
        // Everything else (languages, "Base", "eShop", "SGB Enhanced", "Game"...) is dropped.
        let _ = is_language_list(inner);
    }
    s = tag.replace_all(&s, " ").into_owned();

    // Trademark noise and odd separators.
    s = s.replace(['™', '®', '©'], "").replace('_', " ");
    let spaces = re(&SPACES, r"\s+");
    s = spaces.replace_all(&s, " ").trim().trim_end_matches(['-', ',', ' ']).trim().to_string();

    s = flip_article(&s);
    s = colonize(&s);
    s = fix_shouting(&s);
    out.title = s;
    out
}

/// No-Intro writes "Legend of Zelda, The - Twilight Princess".
/// Turn that back into "The Legend of Zelda - Twilight Princess".
fn flip_article(s: &str) -> String {
    for art in ["The", "A", "An"] {
        let marker = format!(", {art}");
        if let Some(pos) = s.find(&marker) {
            let after = &s[pos + marker.len()..];
            // Only when the article ends the name or is followed by " - subtitle".
            if after.is_empty() || after.starts_with(" -") || after.starts_with(':') {
                return format!("{art} {}{}", &s[..pos], after);
            }
        }
    }
    s.to_string()
}

/// No-Intro uses " - " where the box uses a colon. Convert the first one only:
/// "Ben 10 - Omniverse 2" -> "Ben 10: Omniverse 2".
fn colonize(s: &str) -> String {
    match s.find(" - ") {
        Some(i) if i > 0 => format!("{}: {}", &s[..i], &s[i + 3..]),
        _ => s.to_string(),
    }
}

/// "MARIO KART 8" -> "Mario Kart 8", "angry birds trilogy" -> "Angry Birds Trilogy".
/// Leaves normally-cased names alone.
fn fix_shouting(s: &str) -> String {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    let all_upper = letters.len() >= 6 && !letters.iter().any(|c| c.is_lowercase());
    let all_lower = letters.len() >= 4 && !letters.iter().any(|c| c.is_uppercase());
    if !all_upper && !all_lower {
        return s.to_string();
    }
    if all_lower {
        const SMALL: &[&str] = &["of", "the", "and", "a", "an", "in", "on", "to", "for", "vs"];
        return s
            .split(' ')
            .enumerate()
            .map(|(i, w)| {
                if i > 0 && SMALL.contains(&w) {
                    w.to_string()
                } else {
                    let mut cs = w.chars();
                    match cs.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                        None => String::new(),
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    const KEEP: &[&str] = &["HD", "DS", "3D", "II", "III", "IV", "VI", "VII", "VIII", "IX", "XI", "XII", "U", "TV", "NFL", "NBA", "NHL", "WWE", "DLC", "EX", "GX", "XL", "GTA", "DX", "RPG"];
    s.split(' ')
        .map(|w| {
            if KEEP.contains(&w) || w.chars().all(|c| !c.is_alphabetic()) {
                w.to_string()
            } else {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strip accents: "Pokémon" -> "Pokemon", "Ōkami" -> "Okami".
pub fn fold_accents(s: &str) -> String {
    s.nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect()
}

/// Key used for A–Z ordering: ignores a leading "The ", accents, case and punctuation.
pub fn sort_key(title: &str) -> String {
    let t = fold_accents(title).to_lowercase();
    let t = t.strip_prefix("the ").unwrap_or(&t).to_string();
    t.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Key used to spot the same game twice: no spaces, no articles, no punctuation.
pub fn match_key(title: &str) -> String {
    let t = fold_accents(title).to_lowercase().replace('&', " and ");
    t.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an"))
        .collect::<String>()
}

/// First letter for the A–Z rail ('#' for digits and symbols).
pub fn index_letter(sort_key: &str) -> char {
    match sort_key.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase(),
        _ => '#',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> String {
        parse_name(s, true).title
    }

    #[test]
    fn rankings_and_regions() {
        let p = parse_name("03. Pokemon - Emerald Version (USA, Europe).gba", true);
        assert_eq!(p.title, "Pokemon: Emerald Version");
        assert_eq!(p.region.as_deref(), Some("USA, Europe"));
        assert_eq!(t("05.The Legend of Zelda - Majora's Mask (USA).z64"), "The Legend of Zelda: Majora's Mask");
        assert_eq!(t("01. Golden Sun - The Lost Age (USA, Europe).gba"), "Golden Sun: The Lost Age");
    }

    #[test]
    fn numbers_that_are_not_ranks() {
        assert_eq!(t("007 - Nightfire (USA).iso"), "007: Nightfire");
        assert_eq!(t("20 Minutes Till Dawn.url"), "20 Minutes Till Dawn");
        assert_eq!(t("1942 (Japan, USA).nes"), "1942");
    }

    #[test]
    fn articles_and_revisions() {
        let p = parse_name("Legend of Zelda, The - Twilight Princess HD (USA) (En,Fr,Es) (Rev 2).wux", true);
        assert_eq!(p.title, "The Legend of Zelda: Twilight Princess HD");
        assert_eq!(p.revision.as_deref(), Some("2"));
        assert_eq!(t("Simpsons, The - Hit & Run (USA).iso"), "The Simpsons: Hit & Run");
        assert_eq!(t("Grim Adventures of Billy & Mandy, The (USA).rvz"), "The Grim Adventures of Billy & Mandy");
        assert_eq!(t("Mario Kart 8 (USA) (En,Fr,Es) (Rev 5).wux"), "Mario Kart 8");
        assert_eq!(t("New Super Mario Bros. U (USA) (En,Fr,Es) (Rev 2).wux"), "New Super Mario Bros. U");
    }

    #[test]
    fn switch_and_wiiu_tags() {
        let p = parse_name("Pokémon Legends Z-A [0100F43008C44000][v1.0.0][US][Base][eShop].nsp", true);
        assert_eq!(p.title, "Pokémon Legends Z-A");
        assert_eq!(p.title_id.as_deref(), Some("0100F43008C44000"));
        assert_eq!(p.revision.as_deref(), Some("1.0.0"));
        let u = parse_name("Pokémon Legends Z-A [0100F43008C44800][v65536][US][Update v1.0.1].nsp", true);
        assert_eq!(u.kind, ContentKind::Update);
        let w = parse_name("The Legend of Zelda Breath of the Wild [DLC] [0005000c101c9400]", false);
        assert_eq!(w.kind, ContentKind::Dlc);
        assert_eq!(w.title, "The Legend of Zelda Breath of the Wild");
        assert_eq!(t("Need for Speed™ Most Wanted U (US).wua"), "Need for Speed Most Wanted U");
        assert_eq!(t("MARIO KART 8 (US).wua"), "Mario Kart 8");
        assert_eq!(t("Shantae [0100430013120000] [USA] [v0].nsp"), "Shantae");
    }

    #[test]
    fn keys() {
        assert_eq!(sort_key("The Legend of Zelda: The Wind Waker HD"), "legend of zelda the wind waker hd");
        assert_eq!(sort_key("Ōkami"), "okami");
        assert_eq!(match_key("The Wind Waker HD"), "windwakerhd");
        assert_eq!(match_key("MARIO KART 8"), match_key("Mario Kart 8"));
        assert_eq!(index_letter(&sort_key("20 Minutes Till Dawn")), '#');
    }
}
