//! External theme & asset pack importer (Phase 3):
//! Pulls compatible assets—console wallpapers, console icons/logos, colors,
//! fonts, background music, and UI sound effects—out of:
//!   - EmulationStation / ES-DE / Batocera / RetroBat themes (folder or `.zip`)
//!   - Daijishō platform wallpaper packs (`index.json` or image folders/zips)
//!   - RetroArch console icon packs (e.g., Monochrome / Systematic / FlatUI)
//!   - Generic console wallpaper or icon folders
//!
//! Everything is converted into a clean, safe, standard Spine Theme Pack under
//! `<themes_dir>/<id>/` with zero scripts or unsupported layout XML.

use crate::systems;
use crate::themes::{
    self, ThemeManifest, ThemePack, ThemeStyle, FONT_FILE_EXTS, MUSIC_FILE_EXTS, WALLPAPER_EXTS,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

const ICON_EXTS: &[&str] = &["png", "svg", "webp", "jpg", "jpeg"];
const MAX_IMPORT_FILES: usize = 1500;
const MAX_IMPORT_BYTES: u64 = 250 * 1024 * 1024; // 250 MB

/// Summary returned to the UI so the user knows exactly what was imported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub pack: ThemePack,
    pub source_kind: String,
    pub wallpapers: usize,
    pub icons: usize,
    pub fonts: usize,
    pub music: usize,
    pub sounds: usize,
    pub has_colors: bool,
}

#[derive(Debug, Deserialize)]
struct DaijishoIndex {
    #[serde(default)]
    name: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default, rename = "defaultWallpaper")]
    default_wallpaper: Option<String>,
    #[serde(default, rename = "wallpaperList")]
    wallpaper_list: Vec<DaijishoEntry>,
}

#[derive(Debug, Deserialize)]
struct DaijishoEntry {
    #[serde(default)]
    shortname: String,
    #[serde(default)]
    filename: String,
}

fn file_ext(p: &Path) -> String {
    p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Match a filename, folder name, or libretro/Daijisho/ES-DE platform label to a Spine console/tile ID.
pub fn match_console_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let low = trimmed.to_ascii_lowercase();

    // Skip RetroArch utility / cartridge-content variants so console icons win.
    if low.ends_with("-content")
        || low.ends_with("_content")
        || low.contains("content")
        || matches!(
            low.as_str(),
            "clock"
                | "core"
                | "cursor"
                | "favorites"
                | "history"
                | "image"
                | "images"
                | "input"
                | "menu"
                | "music"
                | "netplay"
                | "off"
                | "on"
                | "reload"
                | "retroarch"
                | "setting"
                | "settings"
                | "subsetting"
                | "video"
                | "videos"
                | "zip"
        )
    {
        if low == "favorites" {
            return Some("favorites".into());
        }
        return None;
    }

    // Special Spine non-console tiles
    match low.as_str() {
        "default" | "_default" | "background" | "bg" | "wallpaper" | "main" | "system" => {
            return Some("default".into());
        }
        "fav" | "favorite" | "favourite" | "favourites" => return Some("favorites".into()),
        "random" | "surprise" => return Some("random".into()),
        "movies" | "movie" => return Some("movies".into()),
        "tv" | "shows" | "series" => return Some("tv".into()),
        "streaming" | "stream" => return Some("streaming".into()),
        "recent" | "recents" | "lastplayed" => return Some("recent".into()),
        "collections" | "collection" | "custom-collections" => return Some("collection".into()),
        _ => {}
    }

    // Strip common vendor prefixes used by RetroArch / libretro / Daijisho / ES-DE
    let mut cleaned = low.as_str();
    for prefix in [
        "nintendo - ",
        "sony - ",
        "sega - ",
        "microsoft - ",
        "atari - ",
        "nec - ",
        "snk - ",
        "bandai - ",
        "nintendo_",
        "sony_",
        "sega_",
        "microsoft_",
        "atari_",
        "nintendo-",
        "sony-",
        "sega-",
        "microsoft-",
        "atari-",
    ] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            cleaned = rest.trim();
            break;
        }
    }

    // Strip trailing decorative suffixes like "-logo", "_bg", "_wallpaper", "(usa)"
    let mut stem = cleaned.to_string();
    if let Some((before, _)) = stem.split_once(" (") {
        stem = before.trim().to_string();
    }
    for suf in [
        "-wallpaper",
        "_wallpaper",
        "-background",
        "_background",
        "-fanart",
        "_fanart",
        "-logo",
        "_logo",
        "-icon",
        "_icon",
        "-bg",
        "_bg",
        " wallpaper",
        " background",
        " logo",
        " icon",
    ] {
        if let Some(rest) = stem.strip_suffix(suf) {
            stem = rest.trim().to_string();
        }
    }

    let norm = systems::norm_folder(&stem);
    if norm.is_empty() {
        return None;
    }

    // Explicit extra aliases common in RetroArch, Daijisho, and ES-DE
    let extra = match norm.as_str() {
        "nintendoentertainmentsystem" | "famicom" | "fc" => Some("nes"),
        "familycomputerdisksystem" | "famicomdisksystem" => Some("fds"),
        "supernintendoentertainmentsystem" | "superfamicom" | "sfc" | "snes" | "snesmsu1" => Some("snes"),
        "nintendo64" | "nintendo64dd" | "n64dd" => Some("n64"),
        "gameboy" => Some("gb"),
        "gameboycolor" => Some("gbc"),
        "gameboyadvance" => Some("gba"),
        "nintendods" | "nds" | "ds" => Some("nds"),
        "nintendo3ds" | "n3ds" | "3ds" => Some("n3ds"),
        "virtualboy" => Some("virtualboy"),
        "gamecube" | "ngc" | "gc" => Some("gc"),
        "wii" | "nintendowii" => Some("wii"),
        "wiiu" | "nintendowiiu" => Some("wiiu"),
        "switch" | "nintendoswitch" | "nsw" => Some("switch"),
        "switch2" | "nintendoswitch2" | "nsw2" => Some("switch2"),
        "mastersystem" | "mastersystemmarkiii" | "markiii" | "sms" => Some("mastersystem"),
        "megadrivegenesis" | "megadrive" | "genesis" | "segagenesis" | "md" => Some("genesis"),
        "gamegear" | "gg" => Some("gamegear"),
        "megacdsegacd" | "segacd" | "megacd" => Some("segacd"),
        "32x" | "sega32x" | "super32x" => Some("sega32x"),
        "saturn" | "segasaturn" => Some("saturn"),
        "dreamcast" | "dc" => Some("dreamcast"),
        "playstation" | "playstation1" | "ps1" | "psx" | "psone" => Some("psx"),
        "playstation2" | "ps2" => Some("ps2"),
        "playstation3" | "ps3" => Some("ps3"),
        "playstationportable" | "psp" => Some("psp"),
        "playstationvita" | "psvita" | "vita" => Some("psvita"),
        "pcengineturbografx16" | "turbografx16" | "pcengine" | "tg16" | "pce" => Some("pcengine"),
        "pcenginecdturbografxcd" | "turbografxcd" | "pcenginecd" | "pcecd" | "tgcd" => Some("pcenginecd"),
        "neogeo" | "snkneogeo" | "neogeoaes" | "neogeomvs" => Some("neogeo"),
        "neogeopocket" | "neogeopocketcolor" | "ngp" | "ngpc" => Some("ngpc"),
        "arcade" | "mame" | "fbneo" | "fbneoarcadegames" | "finalburnneo" | "fba" | "cps1" | "cps2" | "cps3" => Some("arcade"),
        "2600" | "atari2600" => Some("atari2600"),
        "lynx" | "atarilynx" => Some("atarilynx"),
        "wonderswan" | "wonderswancolor" | "ws" | "wsc" => Some("wonderswan"),
        "dos" | "msdos" | "dosbox" => Some("dos"),
        "xbox" => Some("xbox"),
        "xbox360" | "x360" => Some("xbox360"),
        "pc" | "windows" | "steam" | "pcgames" | "desktop" | "epic" | "gog" => Some("pc"),
        _ => None,
    };
    if let Some(id) = extra {
        return Some(id.to_string());
    }

    if let Some(s) = systems::system_by_id(&norm) {
        return Some(s.id.to_string());
    }
    // Exact folder-alias match only (avoid overly broad prefix guesses on arbitrary filenames)
    if let Some(s) = systems::SYSTEMS
        .iter()
        .find(|s| s.folders.iter().any(|f| systems::norm_folder(f) == norm))
    {
        return Some(s.id.to_string());
    }
    None
}

/// Normalize a 6- or 8-digit hex string from ES-DE XML (`#RRGGBB`, `RRGGBB`, `RRGGBBAA`) to `#RRGGBB`.
/// Rejects fully transparent colors (`alpha == 00`).
fn parse_xml_hex(raw: &str) -> Option<String> {
    let s = raw.trim().trim_start_matches('#');
    if !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if s.len() == 6 {
        return Some(format!("#{}", s.to_ascii_uppercase()));
    }
    if s.len() == 8 {
        // Skip transparent or nearly invisible colors (alpha < 0x40)
        let alpha = u8::from_str_radix(&s[6..8], 16).ok()?;
        if alpha < 0x40 {
            return None;
        }
        return Some(format!("#{}", s[0..6].to_ascii_uppercase()));
    }
    None
}

fn hex_lum(hex: &str) -> f32 {
    let s = hex.trim_start_matches('#');
    if s.len() != 6 {
        return 0.5;
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(128) as f32;
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(128) as f32;
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(128) as f32;
    (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
}

fn mix_hex(a: &str, b: &str, t: f32) -> String {
    let pa = a.trim_start_matches('#');
    let pb = b.trim_start_matches('#');
    if pa.len() != 6 || pb.len() != 6 {
        return a.to_string();
    }
    let ch = |i: usize| {
        let x = u8::from_str_radix(&pa[i..i + 2], 16).unwrap_or(0) as f32;
        let y = u8::from_str_radix(&pb[i..i + 2], 16).unwrap_or(0) as f32;
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u8
    };
    format!("#{:02X}{:02X}{:02X}", ch(0), ch(2), ch(4))
}

/// Extract theme name and palette colors from ES-DE / EmulationStation XML files.
fn extract_esde_xml_info(xml_files: &[PathBuf]) -> (Option<String>, BTreeMap<String, String>) {
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let re = TAG_RE.get_or_init(|| {
        Regex::new(r"(?is)<([a-zA-Z0-9_-]+)>([^<]{1,120})</([a-zA-Z0-9_-]+)>").unwrap()
    });

    let mut name: Option<String> = None;
    let mut bg: Option<String> = None;
    let mut surface: Option<String> = None;
    let mut text: Option<String> = None;
    let mut accent: Option<String> = None;

    for path in xml_files {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for cap in re.captures_iter(&content) {
            let tag = cap[1].to_ascii_lowercase();
            if tag != cap[3].to_ascii_lowercase() {
                continue;
            }
            let val = cap[2].trim();
            if name.is_none() && matches!(tag.as_str(), "themename" | "name") && !val.is_empty() && val.len() <= 48 && !val.contains('$') {
                name = Some(val.to_string());
                continue;
            }
            let Some(hex) = parse_xml_hex(val) else {
                continue;
            };
            if bg.is_none()
                && matches!(
                    tag.as_str(),
                    "backgroundcolor" | "colorbackground" | "bgcolor" | "bg" | "mainbackgroundcolor"
                )
            {
                bg = Some(hex);
            } else if surface.is_none()
                && matches!(
                    tag.as_str(),
                    "secondarycolor" | "colorsecondary" | "cardcolor" | "panelcolor" | "surfacecolor" | "helpbackgroundcolor"
                )
            {
                surface = Some(hex);
            } else if accent.is_none()
                && matches!(
                    tag.as_str(),
                    "selectedcolor" | "selectorcolor" | "accentcolor" | "coloraccent" | "highlightcolor" | "primarycolor"
                )
            {
                accent = Some(hex);
            } else if text.is_none()
                && matches!(
                    tag.as_str(),
                    "textcolor" | "colortext" | "fontColor" | "fontcolor" | "systeminfocolor" | "primarytextcolor"
                )
            {
                text = Some(hex);
            }
        }
    }

    let mut colors = BTreeMap::new();
    if bg.is_some() || accent.is_some() || text.is_some() {
        let final_bg = bg.unwrap_or_else(|| "#121318".into());
        let dark = hex_lum(&final_bg) < 0.45;
        let final_text = text
            .filter(|t| (hex_lum(t) - hex_lum(&final_bg)).abs() > 0.25)
            .unwrap_or_else(|| if dark { "#F2F0EB".into() } else { "#1C1A17".into() });
        let final_surface = surface
            .filter(|s| (hex_lum(s) - hex_lum(&final_text)).abs() > 0.25)
            .unwrap_or_else(|| mix_hex(&final_bg, &final_text, if dark { 0.08 } else { 0.06 }));
        let final_surface2 = mix_hex(&final_surface, &final_text, if dark { 0.08 } else { 0.1 });
        let final_muted = mix_hex(&final_text, &final_bg, 0.36);
        let final_line = mix_hex(&final_surface, &final_text, 0.15);

        colors.insert("bg".into(), final_bg);
        colors.insert("surface".into(), final_surface);
        colors.insert("surface2".into(), final_surface2);
        colors.insert("text".into(), final_text.clone());
        colors.insert("muted".into(), final_muted);
        colors.insert("line".into(), final_line);
        colors.insert("focus".into(), final_text);
        if let Some(acc) = accent {
            colors.insert("accent".into(), acc);
        }
    }
    (name, colors)
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 || out.len() >= MAX_IMPORT_FILES {
        return;
    }
    let Ok(rd) = fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let p = ent.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || name == "__MACOSX" {
            continue;
        }
        if p.is_dir() {
            collect_files(&p, out, depth + 1);
        } else if p.is_file() {
            out.push(p);
            if out.len() >= MAX_IMPORT_FILES {
                return;
            }
        }
    }
}

fn map_external_sound_key(stem: &str) -> Option<&'static str> {
    let s = stem.trim().to_ascii_lowercase();
    if s.contains("move") || s.contains("scroll") || s.contains("nav") || s.contains("cursor") {
        return Some("move");
    }
    if s.contains("launch") || s.contains("start") || s.contains("play") {
        return Some("launch");
    }
    if s.contains("select") || s.contains("confirm") || s.contains("accept") || s.contains("click") {
        return Some("select");
    }
    if s.contains("back") || s.contains("cancel") || s.contains("close") {
        return Some("back");
    }
    if s.contains("fav") {
        return Some("favOn");
    }
    if s.contains("open") || s.contains("menu") {
        return Some("open");
    }
    if s.contains("tick") || s.contains("tab") || s.contains("switch") {
        return Some("tick");
    }
    None
}

fn copy_into(dest_dir: &Path, filename: &str, src: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    fs::copy(src, dest_dir.join(filename)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Import an unzipped external folder (ES-DE theme, Daijishō wallpaper pack,
/// RetroArch icon pack, or generic console art/music folder) into `<themes_dir>/`.
pub fn import_external_folder(themes_dir: &Path, src_dir: &Path) -> Result<ImportReport, String> {
    if !src_dir.is_dir() {
        return Err("Selected path is not a folder.".into());
    }

    // If the user picked a folder that only wraps one subfolder, unwrap it.
    let mut root = src_dir.to_path_buf();
    if let Ok(rd) = fs::read_dir(&root) {
        let items: Vec<_> = rd
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| !n.starts_with('.') && n != "__MACOSX")
            })
            .collect();
        if items.len() == 1 && items[0].path().is_dir() {
            root = items[0].path();
        }
    }

    // Check if it's a Daijisho wallpaper pack (has index.json with wallpaperList)
    let daijisho: Option<DaijishoIndex> = fs::read_to_string(root.join("index.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<DaijishoIndex>(&t).ok())
        .filter(|d| !d.wallpaper_list.is_empty() || d.default_wallpaper.is_some());

    let mut files = Vec::new();
    collect_files(&root, &mut files, 0);
    if files.is_empty() {
        return Err("No files found inside this folder.".into());
    }

    let xml_files: Vec<PathBuf> = files
        .iter()
        .filter(|p| file_ext(p) == "xml")
        .cloned()
        .collect();
    let is_esde = !xml_files.is_empty()
        || root.join("theme.xml").is_file()
        || root.join("capabilities.xml").is_file();

    let (xml_name, xml_colors) = if is_esde {
        extract_esde_xml_info(&xml_files)
    } else {
        (None, BTreeMap::new())
    };

    let folder_label = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Imported Pack")
        .replace(['-', '_'], " ");

    let pack_name = daijisho
        .as_ref()
        .map(|d| d.name.trim().to_string())
        .filter(|s| !s.is_empty())
        .or(xml_name)
        .unwrap_or(folder_label);

    let author = daijisho
        .as_ref()
        .map(|d| {
            if !d.author.trim().is_empty() {
                d.author.trim().to_string()
            } else {
                d.authors.join(", ")
            }
        })
        .unwrap_or_default();

    let mut wallpapers: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut icons: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut fonts: Vec<PathBuf> = Vec::new();
    let mut music: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut sounds: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut preview: Option<PathBuf> = None;

    // 1. Daijisho index.json explicit wallpaper mapping
    if let Some(ref d) = daijisho {
        if let Some(ref def_wp) = d.default_wallpaper {
            let p = root.join(def_wp);
            if p.is_file() && WALLPAPER_EXTS.contains(&file_ext(&p).as_str()) {
                wallpapers.insert("default".into(), p);
            }
        }
        for entry in &d.wallpaper_list {
            let p = root.join(&entry.filename);
            if p.is_file() && WALLPAPER_EXTS.contains(&file_ext(&p).as_str()) {
                if let Some(id) = match_console_id(&entry.shortname) {
                    wallpapers.entry(id).or_insert(p);
                }
            }
        }
    }

    // Detect if this looks like a RetroArch icon folder (many PNG/SVG files matching "Vendor - System.png")
    let path_lower = root.to_string_lossy().to_ascii_lowercase();
    let is_mono_hint = path_lower.contains("monochrome") || path_lower.contains("mono");
    let mut retroarch_hits = 0usize;

    // 2. General recursive asset discovery across ES-DE, RetroArch, and plain folders
    for p in &files {
        let rel = p.strip_prefix(&root).unwrap_or(p);
        let rel_str = rel.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
        let ext = file_ext(p);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let stem_low = stem.to_ascii_lowercase();

        // Fonts
        if FONT_FILE_EXTS.contains(&ext.as_str()) {
            if fonts.len() < 4 {
                fonts.push(p.clone());
            }
            continue;
        }

        // Audio (music vs UI sound effects)
        if MUSIC_FILE_EXTS.contains(&ext.as_str()) {
            let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let in_music_dir = rel_str.contains("/music/")
                || rel_str.starts_with("music/")
                || rel_str.contains("/bgm/")
                || rel_str.starts_with("bgm/");
            let in_sound_dir = rel_str.contains("/sound")
                || rel_str.starts_with("sound")
                || rel_str.contains("/sfx/");
            if in_music_dir || (!in_sound_dir && size >= 250_000) || matches!(stem_low.as_str(), "bgm" | "music" | "theme" | "default") {
                let key = match stem_low.as_str() {
                    "classic" | "wave" | "channels" | "blades" | "cube" | "tv" => stem_low.clone(),
                    _ => "default".into(),
                };
                music.entry(key).or_insert_with(|| p.clone());
            } else if let Some(skey) = map_external_sound_key(&stem_low) {
                sounds.entry(skey.to_string()).or_insert_with(|| p.clone());
            }
            continue;
        }

        // Images (preview, icons/logos, wallpapers)
        if !ICON_EXTS.contains(&ext.as_str()) && !WALLPAPER_EXTS.contains(&ext.as_str()) {
            continue;
        }

        if preview.is_none()
            && matches!(stem_low.as_str(), "preview" | "screenshot" | "thumbnail" | "cover")
            && WALLPAPER_EXTS.contains(&ext.as_str())
        {
            preview = Some(p.clone());
            continue;
        }

        // Classify whether this image is an icon/logo or a wallpaper/background.
        let comps: Vec<String> = rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().to_ascii_lowercase()),
                _ => None,
            })
            .collect();
        let parent_dirs = &comps[..comps.len().saturating_sub(1)];
        let is_logo_dir = parent_dirs.iter().any(|d| {
            d.contains("logo") || d.contains("icon") || d.contains("wheel") || d.contains("marquee") || d == "png" || d == "svg"
        });
        let is_wp_dir = parent_dirs.iter().any(|d| {
            d.contains("wallpaper") || d.contains("background") || d.contains("fanart") || d == "bg" || d == "backgrounds"
        });
        let is_logo_name = stem_low.contains("logo") || stem_low.contains("icon") || ext == "svg";
        let is_wp_name = stem_low.contains("wallpaper") || stem_low.contains("background") || stem_low.contains("fanart") || stem_low == "bg";

        // Try matching console id from filename first, then from parent folder (ES-DE `<sys>/art/background.jpg`)
        let sys_from_file = match_console_id(stem);
        let sys_from_dir = parent_dirs
            .iter()
            .rev()
            .find_map(|d| match_console_id(d).filter(|id| id != "default"));

        if stem.contains(" - ") && sys_from_file.is_some() {
            retroarch_hits += 1;
        }

        if is_logo_dir || (is_logo_name && !is_wp_dir) {
            if let Some(id) = sys_from_file.or(sys_from_dir) {
                if id != "default" && ICON_EXTS.contains(&ext.as_str()) {
                    // Prefer SVG or PNG over JPG for icons
                    let replace = icons
                        .get(&id)
                        .map_or(true, |cur| file_ext(cur) != "svg" && ext == "svg");
                    if replace {
                        icons.insert(id, p.clone());
                    }
                }
            }
        } else if is_wp_dir || is_wp_name {
            if let Some(id) = sys_from_dir.or(sys_from_file) {
                if WALLPAPER_EXTS.contains(&ext.as_str()) {
                    wallpapers.entry(id).or_insert_with(|| p.clone());
                }
            }
        } else if let Some(id) = sys_from_file {
            // Flat folder of console images (e.g., RetroArch icon pack OR Daijisho wallpaper folder without index.json)
            if ext == "svg" || path_lower.contains("icon") || path_lower.contains("logo") || path_lower.contains("retroarch") || retroarch_hits >= 2 {
                if id != "default" && ICON_EXTS.contains(&ext.as_str()) {
                    icons.entry(id).or_insert_with(|| p.clone());
                }
            } else if daijisho.is_some() || path_lower.contains("wallpaper") || path_lower.contains("background") || !is_esde {
                if WALLPAPER_EXTS.contains(&ext.as_str()) {
                    wallpapers.entry(id).or_insert_with(|| p.clone());
                }
            }
        }
    }

    if wallpapers.is_empty()
        && icons.is_empty()
        && fonts.is_empty()
        && music.is_empty()
        && sounds.is_empty()
        && xml_colors.is_empty()
    {
        return Err(
            "Couldn't find any compatible wallpapers, console icons, fonts, colors, or audio in this folder."
                .into(),
        );
    }

    let source_kind = if daijisho.is_some() {
        "Daijishō pack"
    } else if is_esde {
        "ES-DE theme"
    } else if retroarch_hits >= 2 || (!icons.is_empty() && wallpapers.is_empty()) {
        "Console icon pack"
    } else {
        "External art pack"
    }
    .to_string();

    // Create destination Spine theme folder
    fs::create_dir_all(themes_dir).map_err(|e| format!("Couldn't create themes folder: {e}"))?;
    let base_id = themes::sanitize_id(&pack_name);
    let mut id = base_id.clone();
    let mut n = 2u32;
    while themes_dir.join(&id).exists() {
        id = format!("{base_id}-{n}");
        n += 1;
    }
    let dest = themes_dir.join(&id);
    fs::create_dir_all(&dest).map_err(|e| format!("Couldn't create theme folder: {e}"))?;

    // Copy wallpapers
    for (key, src) in &wallpapers {
        let ext = file_ext(src);
        copy_into(&dest.join("wallpapers"), &format!("{key}.{ext}"), src)?;
    }

    // Copy icons
    for (key, src) in &icons {
        let ext = file_ext(src);
        copy_into(&dest.join("icons"), &format!("{key}.{ext}"), src)?;
    }

    // Copy fonts & wire first font into colors if present
    let mut colors = xml_colors;
    let mut first_font_family: Option<String> = None;
    for (idx, src) in fonts.iter().enumerate() {
        let ext = file_ext(src);
        let raw_stem = src
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ThemeFont");
        let clean: String = raw_stem
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
            .collect::<String>()
            .trim()
            .to_string();
        let fam = if clean.is_empty() {
            format!("ThemeFont{}", idx + 1)
        } else {
            clean
        };
        copy_into(&dest.join("fonts"), &format!("{fam}.{ext}"), src)?;
        if first_font_family.is_none() {
            first_font_family = Some(fam);
        }
    }
    if let Some(fam) = first_font_family {
        let stack = format!("'{fam}', system-ui, sans-serif");
        colors.insert("display".into(), stack.clone());
        colors.insert("body".into(), stack);
    }

    // Copy music
    for (key, src) in &music {
        let ext = file_ext(src);
        copy_into(&dest.join("music"), &format!("{key}.{ext}"), src)?;
    }

    // Copy sounds
    for (key, src) in &sounds {
        let ext = file_ext(src);
        copy_into(&dest.join("sounds"), &format!("{key}.{ext}"), src)?;
    }

    // Copy preview
    let mut preview_rel = None;
    if let Some(ref prev) = preview {
        let ext = file_ext(prev);
        let fname = format!("preview.{ext}");
        copy_into(&dest, &fname, prev)?;
        preview_rel = Some(fname);
    }

    let base = colors
        .get("bg")
        .map(|b| if hex_lum(b) < 0.45 { "midnight" } else { "cream" })
        .unwrap_or("midnight")
        .to_string();

    let description = daijisho
        .as_ref()
        .map(|d| d.description.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Imported from {source_kind}."));

    let style = if !wallpapers.is_empty() {
        Some(ThemeStyle {
            wallpaper_dim: Some(0.5),
            ..ThemeStyle::default()
        })
    } else {
        None
    };

    let manifest = ThemeManifest {
        name: pack_name,
        author,
        version: "1.0.0".into(),
        description,
        base,
        preview: preview_rel,
        icon_mode: if !icons.is_empty() {
            Some(if is_mono_hint { "mono" } else { "color" }.into())
        } else {
            None
        },
        colors: colors.clone(),
        style,
        options: vec![],
    };

    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(dest.join("theme.json"), json).map_err(|e| e.to_string())?;

    let pack = themes::load_theme(&dest)
        .ok_or_else(|| "Imported files couldn't be loaded as a Spine theme.".to_string())?;

    Ok(ImportReport {
        pack,
        source_kind,
        wallpapers: wallpapers.len(),
        icons: icons.len(),
        fonts: fonts.len(),
        music: music.len(),
        sounds: sounds.len(),
        has_colors: !colors.is_empty(),
    })
}

/// Smart `.zip` importer: tries native Spine theme import first; if the zip isn't
/// a native Spine theme, safely extracts allowed static/XML files to a temp folder
/// and runs the external pack importer (ES-DE / Daijishō / RetroArch).
pub fn import_any_zip(themes_dir: &Path, zip_path: &Path) -> Result<ImportReport, String> {
    // 1. Check if this zip contains a native Spine theme (`theme.json`, `theme.css`, or `mascot/mascot.json`).
    if let Ok(pack) = themes::import_zip(themes_dir, zip_path) {
        let wp = pack.wallpapers.len();
        let ic = pack.icons.len();
        let fn_n = pack.fonts.len();
        let mu = pack.music.len();
        let so = pack.sounds.len();
        let hc = !pack.colors.is_empty();
        return Ok(ImportReport {
            pack,
            source_kind: "Spine theme".into(),
            wallpapers: wp,
            icons: ic,
            fonts: fn_n,
            music: mu,
            sounds: so,
            has_colors: hc,
        });
    }

    // 2. Otherwise extract safe files (+ `.xml` for ES-DE) into a temp directory and convert via external importer.
    let file = fs::File::open(zip_path).map_err(|e| format!("Couldn't open zip file: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Not a valid .zip file: {e}"))?;
    if archive.len() == 0 {
        return Err("This .zip file is empty.".into());
    }
    if archive.len() > MAX_IMPORT_FILES {
        return Err(format!("Too many files in this archive (max {MAX_IMPORT_FILES})."));
    }

    let stage = std::env::temp_dir().join(format!(
        "spine-ext-import-{}-{}",
        std::process::id(),
        zip_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(themes::sanitize_id)
            .unwrap_or_else(|| "pack".into())
    ));
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| format!("Couldn't prepare temporary folder: {e}"))?;

    let mut written: u64 = 0;
    let extract_res: Result<(), String> = (|| {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            if rel.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s == "__MACOSX" || s.starts_with('.')
            }) {
                continue;
            }
            let ext = file_ext(&rel);
            // Only extract safe static assets + .xml (for ES-DE color/name parsing); skip scripts/binaries cleanly.
            if !themes::is_safe_asset_ext(&ext) && ext != "xml" {
                continue;
            }
            let dest = stage.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
            let mut limited = (&mut entry).take(MAX_IMPORT_BYTES.saturating_sub(written) + 1);
            let n = io::copy(&mut limited, &mut out).map_err(|e| e.to_string())?;
            written = written.saturating_add(n);
            if written > MAX_IMPORT_BYTES {
                return Err("Archive is too large (max 250 MB extracted).".into());
            }
        }
        Ok(())
    })();

    let res = match extract_res {
        Ok(()) => import_external_folder(themes_dir, &stage),
        Err(e) => Err(e),
    };
    let _ = fs::remove_dir_all(&stage);
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("spine-ext-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn matches_esde_daijisho_and_retroarch_names() {
        assert_eq!(match_console_id("Nintendo - Super Nintendo Entertainment System").as_deref(), Some("snes"));
        assert_eq!(match_console_id("Sony - PlayStation 2").as_deref(), Some("ps2"));
        assert_eq!(match_console_id("Sega - Mega Drive - Genesis").as_deref(), Some("genesis"));
        assert_eq!(match_console_id("Microsoft - Xbox 360").as_deref(), Some("xbox360"));
        assert_eq!(match_console_id("Nintendo - Game Boy Advance").as_deref(), Some("gba"));
        assert_eq!(match_console_id("Nintendo - Super Nintendo Entertainment System-content"), None);
        assert_eq!(match_console_id("gamecube_wallpaper").as_deref(), Some("gc"));
        assert_eq!(match_console_id("ps1-logo").as_deref(), Some("psx"));
        assert_eq!(match_console_id("default").as_deref(), Some("default"));
    }

    #[test]
    fn imports_esde_theme_folder() {
        let root = tmp("esde");
        let src = root.join("art-book-sample");
        let themes_dir = root.join("themes");
        for sub in ["_inc/backgrounds", "_inc/logos", "_inc/fonts", "_inc/music", "_inc/sounds", "ps2"] {
            fs::create_dir_all(src.join(sub)).unwrap();
        }
        fs::write(
            src.join("capabilities.xml"),
            r#"<themeCapabilities><themeName>Art Book Sample</themeName></themeCapabilities>"#,
        )
        .unwrap();
        fs::write(
            src.join("theme.xml"),
            r#"<theme><variables><backgroundColor>141824FF</backgroundColor><selectedColor>FF5577</selectedColor><textColor>F0F2FA</textColor></variables></theme>"#,
        )
        .unwrap();
        fs::write(src.join("_inc/backgrounds/default.jpg"), b"bg").unwrap();
        fs::write(src.join("_inc/backgrounds/snes.png"), b"snesbg").unwrap();
        fs::write(src.join("ps2/background.jpg"), b"ps2bg").unwrap();
        fs::write(src.join("_inc/logos/snes.svg"), b"<svg/>").unwrap();
        fs::write(src.join("_inc/logos/gamecube.png"), b"gclogo").unwrap();
        fs::write(src.join("_inc/fonts/Cabinet.ttf"), b"font").unwrap();
        fs::write(src.join("_inc/music/bgm.mp3"), b"music").unwrap();
        fs::write(src.join("_inc/sounds/scroll.wav"), b"wav").unwrap();

        let rep = import_external_folder(&themes_dir, &src).unwrap();
        assert_eq!(rep.source_kind, "ES-DE theme");
        assert_eq!(rep.pack.name, "Art Book Sample");
        assert_eq!(rep.wallpapers, 3);
        assert_eq!(rep.icons, 2);
        assert_eq!(rep.fonts, 1);
        assert_eq!(rep.music, 1);
        assert_eq!(rep.sounds, 1);
        assert!(rep.has_colors);
        assert_eq!(rep.pack.colors.get("bg").map(String::as_str), Some("#141824"));
        assert_eq!(rep.pack.colors.get("accent").map(String::as_str), Some("#FF5577"));
        assert!(rep.pack.wallpapers.contains_key("snes"));
        assert!(rep.pack.wallpapers.contains_key("ps2"));
        assert!(rep.pack.icons.contains_key("gc"));
        assert!(rep.pack.sounds.contains_key("move"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn imports_daijisho_wallpaper_zip() {
        let root = tmp("daijisho");
        let themes_dir = root.join("themes");
        let zip_path = root.join("pop-wallpapers.zip");
        {
            let f = fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("index.json", opts).unwrap();
            zw.write_all(
                br#"{"name":"Pop Wallpapers","authors":["Creator"],"defaultWallpaper":"def.jpg","wallpaperList":[{"shortname":"snes","filename":"nintendo_snes.jpg"},{"shortname":"gc","filename":"ngc.png"}]}"#,
            )
            .unwrap();
            zw.start_file("def.jpg", opts).unwrap();
            zw.write_all(b"jpg").unwrap();
            zw.start_file("nintendo_snes.jpg", opts).unwrap();
            zw.write_all(b"jpg").unwrap();
            zw.start_file("ngc.png", opts).unwrap();
            zw.write_all(b"png").unwrap();
            zw.finish().unwrap();
        }

        let rep = import_any_zip(&themes_dir, &zip_path).unwrap();
        assert_eq!(rep.source_kind, "Daijishō pack");
        assert_eq!(rep.pack.name, "Pop Wallpapers");
        assert_eq!(rep.wallpapers, 3);
        assert!(rep.pack.wallpapers.contains_key("default"));
        assert!(rep.pack.wallpapers.contains_key("snes"));
        assert!(rep.pack.wallpapers.contains_key("gc"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn imports_retroarch_monochrome_icons_folder() {
        let root = tmp("retroarch");
        let src = root.join("monochrome");
        let themes_dir = root.join("themes");
        fs::create_dir_all(src.join("png")).unwrap();
        fs::write(src.join("png/Nintendo - Super Nintendo Entertainment System.png"), b"p").unwrap();
        fs::write(src.join("png/Nintendo - Super Nintendo Entertainment System-content.png"), b"c").unwrap();
        fs::write(src.join("png/Sony - PlayStation 2.png"), b"p2").unwrap();
        fs::write(src.join("png/clock.png"), b"clk").unwrap();

        let rep = import_external_folder(&themes_dir, &src).unwrap();
        assert_eq!(rep.icons, 2);
        assert_eq!(rep.pack.icon_mode, "mono");
        assert!(rep.pack.icons.contains_key("snes"));
        assert!(rep.pack.icons.contains_key("ps2"));
        assert!(!rep.pack.has_visual);
        assert!(rep.pack.has_icons);

        let _ = fs::remove_dir_all(&root);
    }
}
