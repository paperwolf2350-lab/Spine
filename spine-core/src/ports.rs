//! Recomps and native PC ports: find them reliably and list them under PC Games
//! with a `Recomp` or `Port` tag.
//!
//! Three layers work together so ports are found without pulling in desktop apps:
//!   1. **Dedicated folders**: `ports` / `recomps` inside a ROM folder, or standalone
//!      folders added in Settings → Library. Top-level shortcuts count as one game;
//!      each subfolder counts as one game (picking the real game `.exe`, never
//!      uninstallers or crash handlers).
//!   2. **Known catalog + recomp fingerprints**: high-precision auto-detection of
//!      well-known recomps/decomps/ports (N64Recomp, XenonRecomp, Ship of Harkinian,
//!      2Ship2Harkinian, OpenGOAL, Gravity Rush, SM64, etc.) plus generic N64Recomp /
//!      RT64 markers (`recompcontrollerdb.txt`, `*Recompiled.exe`, `*Recomp.exe`)
//!      and Steam non-Steam shortcuts (`shortcuts.vdf`).
//!   3. **Manual & Desktop shortcut import**: user-picked `.exe`/`.lnk` entries and
//!      a pre-filtered Desktop / Start Menu shortcut checklist in Settings.

use crate::scan::{Game, Library, SystemLibrary, Version};
use crate::{meta::GameMeta, systems, title};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Whether an entry is a static recompilation or a native/fan PC port.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PortKind {
    Recomp,
    Port,
}

impl PortKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PortKind::Recomp => "recomp",
            PortKind::Port => "port",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            PortKind::Recomp => "Recomp",
            PortKind::Port => "Port",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        if s.eq_ignore_ascii_case("recomp") || s.eq_ignore_ascii_case("recompiled") {
            PortKind::Recomp
        } else {
            PortKind::Port
        }
    }
}

/// One detected or user-added recomp / port before merging into `Library`.
#[derive(Debug, Clone, PartialEq)]
pub struct PortGame {
    pub kind: PortKind,
    pub name: String,
    pub launch: PathBuf,
    pub args: Vec<String>,
    pub workdir: Option<PathBuf>,
    /// Original console id when known ("n64", "ps2", "xbox360", "psvita"...),
    /// used for free libretro box-art fallback.
    pub origin_system: Option<String>,
    pub source: &'static str,
}

/// A manually added or imported recomp/port saved in `spine-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomPort {
    pub path: String,
    pub name: String,
    pub kind: PortKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_system: Option<String>,
}

/// A Desktop or Start Menu shortcut offered in the Settings import checklist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutCandidate {
    pub path: String,
    pub name: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    pub kind: PortKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_system: Option<String>,
    pub reason: String,
    pub already_added: bool,
}

// ---------------------------------------------------------------------------
// Curated catalog of known recomps, decomps and native PC ports
// ---------------------------------------------------------------------------

struct CatalogEntry {
    /// Executable filenames (lower-case, with or without .exe).
    exes: &'static [&'static str],
    /// Optional shortcut / folder title phrases that uniquely identify this port.
    title_hints: &'static [&'static str],
    /// Canonical clean game title (for display and cover lookup).
    title: &'static str,
    kind: PortKind,
    /// Original console id in `systems::SYSTEMS`, if any.
    origin: Option<&'static str>,
}

const CATALOG: &[CatalogEntry] = &[
    // ---- Static Recompilations (N64Recomp / XenonRecomp / etc.)
    CatalogEntry {
        exes: &["zelda64recompiled.exe", "zelda64recompiled"],
        title_hints: &["zelda 64 recompiled", "zelda64recompiled", "majora's mask recompiled"],
        title: "The Legend of Zelda: Majora's Mask",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["starfox64recompiled.exe", "starfox64recompiled"],
        title_hints: &["star fox 64 recompiled", "starfox64recompiled"],
        title: "Star Fox 64",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["goemon64recomp.exe", "goemon64recompiled.exe", "goemon64recomp", "goemon64recompiled"],
        title_hints: &["goemon 64 recompiled", "mystical ninja recompiled"],
        title: "Mystical Ninja Starring Goemon",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["bkrecomp.exe", "banjorecompiled.exe", "banjo64recompiled.exe", "bkrecomp"],
        title_hints: &["banjo-kazooie recompiled", "banjo kazooie recompiled"],
        title: "Banjo-Kazooie",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["dk64recomp.exe", "dk64recompiled.exe", "dk64recomp"],
        title_hints: &["donkey kong 64 recompiled", "dk64 recompiled"],
        title: "Donkey Kong 64",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["mariokart64recomp.exe", "mk64recomp.exe", "mk64recompiled.exe"],
        title_hints: &["mario kart 64 recompiled"],
        title: "Mario Kart 64",
        kind: PortKind::Recomp,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["unleashedrecomp.exe", "unleashedrecomp"],
        title_hints: &["unleashed recompiled", "sonic unleashed recompiled"],
        title: "Sonic Unleashed",
        kind: PortKind::Recomp,
        origin: Some("xbox360"),
    },
    // ---- Decomps & Native PC Ports
    CatalogEntry {
        exes: &["soh.exe", "soh.appimage", "soh-linux"],
        title_hints: &["ship of harkinian"],
        title: "The Legend of Zelda: Ocarina of Time",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["2s2h.exe", "2s2h.appimage"],
        title_hints: &["2ship2harkinian", "2 ship 2 harkinian"],
        title: "The Legend of Zelda: Majora's Mask",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["spaghettikart.exe", "spaghettikart"],
        title_hints: &["spaghettikart", "spaghetti kart"],
        title: "Mario Kart 64",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["starship.exe", "starship.appimage"],
        title_hints: &["starship sf64"],
        title: "Star Fox 64",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["pd.x86_64.exe", "pd.x86_64", "pd.i686.exe"],
        title_hints: &["perfect dark pc", "perfect dark port"],
        title: "Perfect Dark",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["sm64.us.f32.exe", "sm64ex.exe", "render96ex.exe", "sm64coopdx.exe", "sm64pc.exe", "sm64plus.exe"],
        title_hints: &["super mario 64 pc", "sm64ex", "sm64coopdx", "render96"],
        title: "Super Mario 64",
        kind: PortKind::Port,
        origin: Some("n64"),
    },
    CatalogEntry {
        exes: &["zelda3.exe", "zelda3"],
        title_hints: &["zelda 3 pc"],
        title: "The Legend of Zelda: A Link to the Past",
        kind: PortKind::Port,
        origin: Some("snes"),
    },
    CatalogEntry {
        exes: &["gravityrush2.exe", "gravity rush 2.exe"],
        title_hints: &["gravity rush 2"],
        title: "Gravity Rush 2",
        kind: PortKind::Port,
        origin: None,
    },
    CatalogEntry {
        exes: &["gravityrush.exe", "gravityrushremastered.exe", "gravity rush.exe", "gravity rush launcher.exe", "gravityrushlauncher.exe"],
        title_hints: &["gravity rush remastered", "gravity rush launcher", "gravity rush"],
        title: "Gravity Rush",
        kind: PortKind::Port,
        origin: Some("psvita"),
    },
    CatalogEntry {
        exes: &["re3.exe", "re3"],
        title_hints: &["re3 gta"],
        title: "Grand Theft Auto III",
        kind: PortKind::Port,
        origin: Some("ps2"),
    },
    CatalogEntry {
        exes: &["revc.exe", "revc"],
        title_hints: &["revc gta"],
        title: "Grand Theft Auto: Vice City",
        kind: PortKind::Port,
        origin: Some("ps2"),
    },
    CatalogEntry {
        exes: &["devilutionx.exe", "devilutionx", "devilutionx.appimage"],
        title_hints: &["devilutionx"],
        title: "Diablo",
        kind: PortKind::Port,
        origin: Some("psx"),
    },
    CatalogEntry {
        exes: &["tr1x.exe", "tomb1main.exe", "tr1x"],
        title_hints: &["tr1x", "tomb1main"],
        title: "Tomb Raider",
        kind: PortKind::Port,
        origin: Some("psx"),
    },
    CatalogEntry {
        exes: &["tr2x.exe", "tr2x"],
        title_hints: &["tr2x"],
        title: "Tomb Raider II",
        kind: PortKind::Port,
        origin: Some("psx"),
    },
    CatalogEntry {
        exes: &["sonic3air.exe", "sonic3air"],
        title_hints: &["sonic 3 a.i.r", "sonic 3 air"],
        title: "Sonic 3 A.I.R.",
        kind: PortKind::Port,
        origin: Some("genesis"),
    },
    CatalogEntry {
        exes: &["openmw.exe", "openmw"],
        title_hints: &["openmw"],
        title: "The Elder Scrolls III: Morrowind",
        kind: PortKind::Port,
        origin: None,
    },
    CatalogEntry {
        exes: &["am2r.exe", "am2r"],
        title_hints: &["am2r"],
        title: "AM2R",
        kind: PortKind::Port,
        origin: None,
    },
    CatalogEntry {
        exes: &["d1x-rebirth.exe", "d1x-rebirth"],
        title_hints: &["d1x-rebirth"],
        title: "Descent",
        kind: PortKind::Port,
        origin: Some("dos"),
    },
    CatalogEntry {
        exes: &["d2x-rebirth.exe", "d2x-rebirth"],
        title_hints: &["d2x-rebirth"],
        title: "Descent II",
        kind: PortKind::Port,
        origin: Some("dos"),
    },
    CatalogEntry {
        exes: &["nxengine-evo.exe", "nxengine.exe", "doukutsu.exe"],
        title_hints: &["nxengine"],
        title: "Cave Story",
        kind: PortKind::Port,
        origin: None,
    },
    CatalogEntry {
        exes: &["opengoal-launcher.exe", "opengoal.exe"],
        title_hints: &["opengoal"],
        title: "Jak and Daxter: The Precursor Legacy",
        kind: PortKind::Port,
        origin: Some("ps2"),
    },
    CatalogEntry {
        exes: &["bloodborne.exe"],
        title_hints: &["bloodborne pc"],
        title: "Bloodborne",
        kind: PortKind::Port,
        origin: None,
    },
];

fn match_catalog_exe(exe_name_lower: &str) -> Option<&'static CatalogEntry> {
    let cleaned = exe_name_lower.replace(['™', '®', '©'], "");
    CATALOG.iter().find(|c| c.exes.iter().any(|&e| e == exe_name_lower || e == cleaned))
}

fn match_catalog_title(raw: &str) -> Option<&'static CatalogEntry> {
    let low = raw.to_ascii_lowercase().replace(['™', '®', '©'], "");
    CATALOG.iter().find(|c| c.title_hints.iter().any(|&h| low.contains(h)))
}

// ---------------------------------------------------------------------------
// Windows .lnk (MS-SHLLINK) binary parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedLnk {
    pub target: Option<PathBuf>,
    pub args: Vec<String>,
    pub workdir: Option<PathBuf>,
}

/// Parse a Windows `.lnk` file into its target executable, CLI arguments, and working directory.
pub fn parse_lnk(path: &Path) -> Option<ParsedLnk> {
    let bytes = fs::read(path).ok()?;
    parse_lnk_bytes(&bytes, path.parent())
}

pub fn parse_lnk_bytes(bytes: &[u8], lnk_dir: Option<&Path>) -> Option<ParsedLnk> {
    // Standard ShellLinkHeader: 76 bytes, HeaderSize == 0x4C, CLSID == 00021401-0000-0000-C000-000000000046.
    const CLSID: [u8; 16] = [
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
    ];
    if bytes.len() < 76 || u32::from_le_bytes(bytes[0..4].try_into().ok()?) != 0x4C || bytes[4..20] != CLSID {
        return None;
    }

    let flags = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    let has_id_list = flags & 0x01 != 0;
    let has_link_info = flags & 0x02 != 0;
    let has_name = flags & 0x04 != 0;
    let has_rel_path = flags & 0x08 != 0;
    let has_work_dir = flags & 0x10 != 0;
    let has_args = flags & 0x20 != 0;
    let is_unicode = flags & 0x80 != 0;

    let mut pos = 76usize;
    let mut out = ParsedLnk::default();

    // 1. LinkTargetIDList
    if has_id_list {
        if pos + 2 > bytes.len() {
            return None;
        }
        let sz = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        pos = pos.checked_add(2)?.checked_add(sz)?;
        if pos > bytes.len() {
            return None;
        }
    }

    // 2. LinkInfo
    if has_link_info {
        if pos + 4 > bytes.len() {
            return None;
        }
        let info_start = pos;
        let info_size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        if info_size < 28 || info_start.checked_add(info_size).map_or(true, |e| e > bytes.len()) {
            return None;
        }
        let info = &bytes[info_start..info_start + info_size];
        let hdr_size = u32::from_le_bytes(info[4..8].try_into().ok()?) as usize;
        let info_flags = u32::from_le_bytes(info[8..12].try_into().ok()?);
        let base_off = u32::from_le_bytes(info[16..20].try_into().ok()?) as usize;
        let suffix_off = u32::from_le_bytes(info[24..28].try_into().ok()?) as usize;

        if info_flags & 0x01 != 0 {
            let join_suffix = |b: String, s: String| -> String {
                if s.is_empty() {
                    b
                } else if b.ends_with('\\') || s.starts_with('\\') {
                    format!("{b}{s}")
                } else {
                    format!("{b}\\{s}")
                }
            };
            let mut local = None;
            if hdr_size >= 36 && info.len() >= 36 {
                let u_base = u32::from_le_bytes(info[28..32].try_into().ok()?) as usize;
                let u_suffix = u32::from_le_bytes(info[32..36].try_into().ok()?) as usize;
                if u_base > 0 && u_base < info.len() {
                    let b = read_utf16_cstr(&info[u_base..]);
                    let s = if u_suffix > 0 && u_suffix < info.len() { read_utf16_cstr(&info[u_suffix..]) } else { String::new() };
                    if !b.is_empty() {
                        local = Some(join_suffix(b, s));
                    }
                }
            }
            if local.is_none() && base_off > 0 && base_off < info.len() {
                let b = read_ansi_cstr(&info[base_off..]);
                let s = if suffix_off > 0 && suffix_off < info.len() { read_ansi_cstr(&info[suffix_off..]) } else { String::new() };
                if !b.is_empty() {
                    local = Some(join_suffix(b, s));
                }
            }
            if let Some(t) = local {
                out.target = Some(PathBuf::from(t));
            }
        }
        pos = info_start + info_size;
    }

    // 3. StringData (order is fixed by spec: Name, RelativePath, WorkingDir, Arguments, IconLocation)
    let read_str = |p: &mut usize| -> Option<String> {
        if *p + 2 > bytes.len() {
            return None;
        }
        let count = u16::from_le_bytes([bytes[*p], bytes[*p + 1]]) as usize;
        *p += 2;
        if is_unicode {
            let byte_len = count.checked_mul(2)?;
            if *p + byte_len > bytes.len() {
                return None;
            }
            let units: Vec<u16> = bytes[*p..*p + byte_len]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            *p += byte_len;
            Some(String::from_utf16_lossy(&units))
        } else {
            if *p + count > bytes.len() {
                return None;
            }
            let s = decode_win1252(&bytes[*p..*p + count]);
            *p += count;
            Some(s)
        }
    };

    if has_name {
        let _ = read_str(&mut pos);
    }
    let rel_path = if has_rel_path { read_str(&mut pos) } else { None };
    if has_work_dir {
        if let Some(wd) = read_str(&mut pos).filter(|s| !s.trim().is_empty()) {
            out.workdir = Some(PathBuf::from(wd.trim()));
        }
    }
    if has_args {
        if let Some(a) = read_str(&mut pos).filter(|s| !s.trim().is_empty()) {
            out.args = split_cmdline(&a);
        }
    }
    // Skip IconLocation if present so `pos` lands cleanly on ExtraData blocks.
    if flags & 0x40 != 0 {
        let _ = read_str(&mut pos);
    }

    // 4. ExtraData: check EnvironmentVariableDataBlock (0xA0000001) if target is missing or not on disk.
    if out.target.as_ref().map_or(true, |t| !t.exists()) {
        if let Some(env_target) = parse_lnk_env_target(bytes, pos) {
            if out.target.is_none() || env_target.exists() {
                out.target = Some(env_target);
            }
        }
    }

    // 5. Fallbacks if LinkInfo didn't give an existing path.
    if out.target.as_ref().map_or(true, |t| !t.exists()) {
        if let (Some(rel), Some(base)) = (rel_path.filter(|s| !s.trim().is_empty()), lnk_dir) {
            let candidate = base.join(rel.trim());
            if candidate.exists() {
                out.target = Some(candidate);
            }
        }
    }
    if out.target.is_none() {
        if let Some(found) = scan_utf16_exe_path(bytes).or_else(|| scan_ascii_exe_path(bytes)) {
            out.target = Some(PathBuf::from(found));
        }
    }

    Some(out)
}

/// Decode a Windows-1252 byte slice (preserving characters like `™` = 0x99, `–` = 0x96, `’` = 0x92).
fn decode_win1252(slice: &[u8]) -> String {
    const CP1252_80_9F: [char; 32] = [
        '€', '\u{0081}', '‚', 'ƒ', '„', '…', '†', '‡',
        'ˆ', '‰', 'Š', '‹', 'Œ', '\u{008D}', 'Ž', '\u{008F}',
        '\u{0090}', '‘', '’', '“', '”', '•', '–', '—',
        '˜', '™', 'š', '›', 'œ', '\u{009D}', 'ž', 'Ÿ',
    ];
    slice
        .iter()
        .map(|&b| if (0x80..=0x9F).contains(&b) { CP1252_80_9F[(b - 0x80) as usize] } else { b as char })
        .collect()
}

fn read_ansi_cstr(slice: &[u8]) -> String {
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    decode_win1252(&slice[..end])
}

fn read_utf16_cstr(slice: &[u8]) -> String {
    let mut units = Vec::new();
    for chunk in slice.chunks_exact(2) {
        let u = u16::from_le_bytes([chunk[0], chunk[1]]);
        if u == 0 {
            break;
        }
        units.push(u);
    }
    String::from_utf16_lossy(&units)
}

/// Parse ExtraData's EnvironmentVariableDataBlock (signature 0xA0000001) and expand %VAR%.
fn parse_lnk_env_target(bytes: &[u8], mut pos: usize) -> Option<PathBuf> {
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        if size < 4 {
            break;
        }
        if pos.checked_add(size).map_or(true, |e| e > bytes.len()) {
            break;
        }
        let sig = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?);
        if sig == 0xA000_0001 && size >= 788 {
            let unicode = read_utf16_cstr(&bytes[pos + 268..pos + 788]);
            let raw = if !unicode.is_empty() { unicode } else { read_ansi_cstr(&bytes[pos + 8..pos + 268]) };
            if !raw.is_empty() {
                return Some(PathBuf::from(expand_win_env(&raw)));
            }
        }
        pos += size;
    }
    None
}

fn expand_win_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(j) = after.find('%') {
            let var = &after[..j];
            if !var.is_empty() {
                if let Ok(val) = std::env::var(var) {
                    out.push_str(&val);
                    rest = &after[j + 1..];
                    continue;
                }
            }
            out.push('%');
            out.push_str(var);
            out.push('%');
            rest = &after[j + 1..];
        } else {
            out.push('%');
            rest = after;
            break;
        }
    }
    out.push_str(rest);
    out
}

fn scan_utf16_exe_path(bytes: &[u8]) -> Option<String> {
    for align in 0..2 {
        if bytes.len() <= align + 12 {
            continue;
        }
        let units: Vec<u16> = bytes[align..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text: String = units
            .into_iter()
            .map(|u| if (0x20..0xD800).contains(&u) { char::from_u32(u as u32).unwrap_or('\n') } else { '\n' })
            .collect();
        if let Some(line) = text.lines().find(|l| {
            let b = l.as_bytes();
            b.len() > 6 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && l.to_ascii_lowercase().ends_with(".exe")
        }) {
            return Some(line.to_string());
        }
    }
    None
}

fn scan_ascii_exe_path(bytes: &[u8]) -> Option<String> {
    let text: String = bytes
        .iter()
        .map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else if (0x80..=0x9f).contains(&b) {
                decode_win1252(&[b]).chars().next().unwrap_or('\n')
            } else {
                '\n'
            }
        })
        .collect();
    text.lines()
        .find(|l| {
            let b = l.as_bytes();
            b.len() > 6 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' && l.to_ascii_lowercase().ends_with(".exe")
        })
        .map(String::from)
}

/// Split a Windows/POSIX command-line argument string while preserving quoted segments.
pub fn split_cmdline(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut has_token = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                has_token = true;
            }
            '\\' if in_quote && chars.peek() == Some(&'"') => {
                cur.push(chars.next().unwrap());
                has_token = true;
            }
            c if c.is_whitespace() && !in_quote => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            _ => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------------------
// PE header subsystem check (distinguish GUI games from console helpers)
// ---------------------------------------------------------------------------

/// Returns `Some(true)` for Windows GUI `.exe`, `Some(false)` for Console `.exe`, `None` if not PE.
fn pe_is_gui(path: &Path) -> Option<bool> {
    let mut f = fs::File::open(path).ok()?;
    use std::io::Read;
    let mut buf = vec![0u8; 1024];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.len() < 0x40 || &buf[0..2] != b"MZ" {
        return None;
    }
    let pe_off = u32::from_le_bytes(buf[0x3C..0x40].try_into().ok()?) as usize;
    let sub_off = pe_off.checked_add(24 + 68)?;
    if sub_off + 2 > buf.len() || &buf[pe_off..pe_off + 4] != b"PE\0\0" {
        return None;
    }
    let subsystem = u16::from_le_bytes([buf[sub_off], buf[sub_off + 1]]);
    match subsystem {
        2 => Some(true),  // IMAGE_SUBSYSTEM_WINDOWS_GUI
        3 => Some(false), // IMAGE_SUBSYSTEM_WINDOWS_CUI
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Title cleaning & classification helpers
// ---------------------------------------------------------------------------

/// Executables inside a game folder that are never the game itself.
const REJECT_EXE_PREFIXES: &[&str] = &[
    "unins", "uninstall", "setup", "install", "redist", "vcredist", "dxsetup", "dxwebsetup",
    "crash", "unitycrash", "bugreport", "report", "updater", "autoupdate", "helper",
    "bootstrap", "ueprereq", "ue4prereq", "prereq", "7z", "ffmpeg", "ffprobe", "curl",
    "createdump", "dotnet", "vc_redist", "ndp", "directx", "physx", "openal", "oalinst",
];

/// Apps that frequently live on Desktop / Start Menu and should not clutter the importer.
const NON_GAME_APPS: &[&str] = &[
    "chrome", "firefox", "edge", "brave", "opera", "vivaldi", "zen browser", "waterfox",
    "discord", "slack", "teams", "zoom", "telegram", "whatsapp", "signal", "spotify",
    "steam", "epic games launcher", "gog galaxy", "ea app", "ubisoft connect", "battle.net",
    "xbox", "google play games", "playnite", "heroic", "lutris", "spine",
    "claude", "chatgpt", "open webui", "ollama", "lm studio", "cursor", "visual studio",
    "vs code", "code", "notepad", "obsidian", "notion", "libreoffice", "word", "excel",
    "powerpoint", "onedrive", "megasync", "dropbox", "google drive", "syncthing",
    "cheat engine", "modly", "vortex", "mod organizer", "r2modman", "thunderstore",
    "obs", "vlc", "mpv", "audacity", "blender", "gimp", "photoshop", "davinci",
    "7-zip", "winrar", "qbittorrent", "filezilla", "putty", "git", "github", "docker",
    "nvidia", "amd", "geforce", "msi afterburner", "rtss", "hwinfo", "cpu-z", "gpu-z",
    "ds4windows", "reWASD", "joytokey", "antimicro", "stream deck", "elgato", "icue",
    "logitech", "razer", "steelseries", "armoury crate", "wallpaper engine", "powertoys",
    "everything", "ccleaner", "malwarebytes", "bitwarden", "1password", "keepass",
    // Emulators themselves (not games)
    "retroarch", "pcsx2", "dolphin", "cemu", "ppsspp", "azahar", "citra", "vita3k",
    "eden", "ryujinx", "rpcs3", "melonds", "xemu", "xenia", "duckstation", "rmg",
    "simple64", "mgba", "flycast", "mesen", "shadps4", "lichii", "eglantine",
];

fn is_rejected_exe(name_lower: &str) -> bool {
    let stem = name_lower.strip_suffix(".exe").unwrap_or(name_lower);
    REJECT_EXE_PREFIXES.iter().any(|p| stem.starts_with(p))
        || stem.contains("unins")
        || stem.contains("crashhandler")
        || stem.contains("crashreport")
        || stem.contains("vcredist")
        || stem.ends_with("_helper")
        || stem.ends_with("-helper")
}

fn is_non_game_app(name: &str, exe_stem: &str) -> bool {
    let nl = name.to_ascii_lowercase();
    let el = exe_stem.to_ascii_lowercase();
    NON_GAME_APPS.iter().any(|&a| {
        let a = a.to_ascii_lowercase();
        nl == a || el == a || nl.starts_with(&format!("{a} ")) || nl.ends_with(&format!(" {a}"))
    })
}

/// Detect generic static-recomp markers in a game's folder (catches future N64Recomp /
/// XenonRecomp / PS2Recomp releases without needing an updated hardcoded list).
pub fn folder_has_recomp_markers(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    const MARKERS: &[&str] = &[
        "recompcontrollerdb.txt",
        "rt64.json",
        "recomp_config.json",
    ];
    if MARKERS.iter().any(|m| dir.join(m).exists()) {
        return true;
    }
    // Check one level down (e.g. assets/recompcontrollerdb.txt)
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if n == "recompcontrollerdb.txt" || n.starts_with("n64recomp") || n.starts_with("xenonrecomp") {
                return true;
            }
        }
    }
    false
}

/// Heuristic: does this executable name / folder / title signal a Recomp?
pub fn looks_like_recomp(text: &str, dir: Option<&Path>) -> bool {
    if let Some(d) = dir {
        if folder_has_recomp_markers(d) {
            return true;
        }
    }
    let low = text.to_ascii_lowercase();
    low.contains("recompiled") || low.contains("recomp")
}

/// Clean a port/recomp shortcut, folder, or exe name into a display title.
///
/// Examples:
///   "Gravity Rush 2™ - Shortcut" -> "Gravity Rush 2"
///   "Zelda64Recompiled" -> "Zelda 64"
///   "Super Mario 64 (PC Port) [v1.2]" -> "Super Mario 64"
pub fn clean_port_title(raw: &str) -> String {
    static TRAIL_SHORTCUT: OnceLock<Regex> = OnceLock::new();
    static TRAIL_VER: OnceLock<Regex> = OnceLock::new();
    static TRAIL_NOISE: OnceLock<Regex> = OnceLock::new();
    static CAMEL: OnceLock<Regex> = OnceLock::new();
    static DIGIT_SPLIT: OnceLock<Regex> = OnceLock::new();
    static SPACES: OnceLock<Regex> = OnceLock::new();

    let mut s = raw.trim().to_string();
    // Strip file extensions if given one.
    for ext in [".lnk", ".url", ".exe", ".bat", ".sh", ".desktop", ".appimage"] {
        if s.to_ascii_lowercase().ends_with(ext) {
            s.truncate(s.len() - ext.len());
            break;
        }
    }
    s = s.replace(['™', '®', '©'], "").replace('_', " ");

    // "Gravity Rush 2 - Shortcut" -> "Gravity Rush 2"
    let r_sc = TRAIL_SHORTCUT.get_or_init(|| Regex::new(r"(?i)\s*[-–—]?\s*shortcut\s*$").unwrap());
    s = r_sc.replace(&s, "").into_owned();

    // Strip parenthetical / bracketed noise like (Win64), [PC Port], [v1.2.3], (x64)
    let parsed = title::parse_name(&s, false);
    if !parsed.title.is_empty() {
        s = parsed.title;
    }

    // Strip trailing version / architecture / port-type words (we show the tag pill instead).
    let r_ver = TRAIL_VER.get_or_init(|| Regex::new(r"(?i)\s+(?:v?\d+\.\d+(?:\.\d+)*|win64|win32|x64|x86_64|x86|linux|windows|portable)\s*$").unwrap());
    loop {
        let next = r_ver.replace(&s, "").into_owned();
        if next == s {
            break;
        }
        s = next;
    }

    // Split single-word CamelCase exe names like "StarFox64Recompiled" -> "Star Fox 64 Recompiled"
    if !s.contains(' ') {
        let r_camel = CAMEL.get_or_init(|| Regex::new(r"([a-z0-9])([A-Z])").unwrap());
        s = r_camel.replace_all(&s, "$1 $2").into_owned();
        let r_dig = DIGIT_SPLIT.get_or_init(|| Regex::new(r"([A-Za-z])(\d{2,})").unwrap());
        s = r_dig.replace_all(&s, "$1 $2").into_owned();
    }

    let r_noise = TRAIL_NOISE.get_or_init(|| Regex::new(r"(?i)[\s:\-–—]+(?:recompiled|recomp|pc\s*port|native\s*port|decomp)\s*$").unwrap());
    let trimmed = r_noise.replace(&s, "").trim().to_string();
    if trimmed.len() >= 2 {
        s = trimmed;
    }

    let r_sp = SPACES.get_or_init(|| Regex::new(r"\s+").unwrap());
    r_sp.replace_all(&s, " ").trim().trim_end_matches(['-', ':', ' ']).trim().to_string()
}

// ---------------------------------------------------------------------------
// Layer 1: Dedicated Ports / Recomps folder scanning
// ---------------------------------------------------------------------------

/// Folder names inside a ROM root that Spine automatically treats as a Ports/Recomps folder.
pub const PORT_FOLDER_NAMES: &[&str] = &["ports", "recomps", "pcports", "recompilations", "fanports", "decomps"];

pub fn is_port_folder_name(name: &str) -> bool {
    let n = systems::norm_folder(name);
    PORT_FOLDER_NAMES.contains(&n.as_str())
}

/// Scan a dedicated ports/recomps folder:
///   - Top-level `.lnk` / `.url` / `.bat` / `.sh` / `.desktop` / `.AppImage` -> 1 game each.
///   - Top-level subfolder -> 1 game (smart `.exe` selection, ignoring uninstallers/helpers).
pub fn scan_port_folder(root: &Path) -> Vec<PortGame> {
    let Ok(rd) = fs::read_dir(root) else { return vec![] };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());

    let folder_is_recomp = root
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase().contains("recomp"))
        .unwrap_or(false);

    let mut out = Vec::new();
    for e in entries {
        let p = e.path();
        let raw_name = e.file_name().to_string_lossy().to_string();
        if raw_name.starts_with('.') || raw_name.starts_with("._") {
            continue;
        }
        if p.is_dir() {
            if let Some(g) = detect_game_in_subfolder(&p, &raw_name, folder_is_recomp) {
                out.push(g);
            }
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if ext == "lnk" {
            if let Some(g) = game_from_lnk(&p, &raw_name, folder_is_recomp, "folder") {
                out.push(g);
            }
        } else if matches!(ext.as_str(), "exe" | "appimage" | "sh" | "bat" | "url" | "desktop") {
            if ext == "exe" && is_rejected_exe(&raw_name.to_ascii_lowercase()) {
                continue;
            }
            let cat = match_catalog_exe(&raw_name.to_ascii_lowercase()).or_else(|| match_catalog_title(&raw_name));
            let kind = cat.map(|c| c.kind).unwrap_or_else(|| {
                if folder_is_recomp || looks_like_recomp(&raw_name, p.parent()) {
                    PortKind::Recomp
                } else {
                    PortKind::Port
                }
            });
            let name = cat.map(|c| c.title.to_string()).unwrap_or_else(|| clean_port_title(&raw_name));
            if !name.is_empty() {
                out.push(PortGame {
                    kind,
                    name,
                    launch: p.clone(),
                    args: vec![],
                    workdir: p.parent().map(Path::to_path_buf),
                    origin_system: cat.and_then(|c| c.origin).map(String::from),
                    source: "folder",
                });
            }
        }
    }
    out
}

/// Inspect a single game subfolder and pick its best launchable executable.
fn detect_game_in_subfolder(dir: &Path, folder_name: &str, default_recomp: bool) -> Option<PortGame> {
    let mut exes: Vec<(PathBuf, usize)> = Vec::new();
    let mut lnks: Vec<PathBuf> = Vec::new();
    collect_launchables(dir, 0, &mut exes, &mut lnks);

    // 1. Exact catalog match on any executable inside the folder.
    for (p, _) in &exes {
        let fname = p.file_name()?.to_string_lossy().to_ascii_lowercase();
        if let Some(cat) = match_catalog_exe(&fname) {
            return Some(PortGame {
                kind: cat.kind,
                name: cat.title.to_string(),
                launch: p.clone(),
                args: vec![],
                workdir: p.parent().map(Path::to_path_buf),
                origin_system: cat.origin.map(String::from),
                source: "folder",
            });
        }
    }

    // 2. If the user dropped a `.lnk` right inside the folder, honor it.
    if let Some(lnk) = lnks.first() {
        let lnk_name = lnk.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| folder_name.to_string());
        if let Some(mut g) = game_from_lnk(lnk, &lnk_name, default_recomp, "folder") {
            if g.name.is_empty() {
                g.name = clean_port_title(folder_name);
            }
            return Some(g);
        }
    }

    // 3. Score candidate executables.
    let folder_tokens: Vec<String> = clean_port_title(folder_name)
        .to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(String::from)
        .collect();

    let mut best: Option<(i64, PathBuf)> = None;
    for (p, depth) in exes {
        let fname = p.file_name()?.to_string_lossy().to_ascii_lowercase();
        if is_rejected_exe(&fname) {
            continue;
        }
        let stem = fname.strip_suffix(".exe").unwrap_or(&fname);
        let mut score: i64 = 0;

        // Depth bonus: root of the game folder beats nested bin/tools folders.
        if depth == 0 {
            score += 25;
        } else if depth == 1 {
            score += 10;
        }

        // Token match against the folder name (strongest signal).
        let stem_clean: String = stem.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let matched_tokens = folder_tokens.iter().filter(|t| stem_clean.contains(t.as_str())).count();
        score += (matched_tokens as i64) * 35;

        // Prefer GUI binaries over console tools on Windows.
        if fname.ends_with(".exe") {
            match pe_is_gui(&p) {
                Some(true) => score += 25,
                Some(false) => score -= 15,
                None => {}
            }
        }

        // Slight penalty for "launcher" / "config" when a real game binary sits alongside it.
        if stem.contains("launcher") || stem.contains("config") || stem.contains("settings") {
            score -= 12;
        }

        // Mild size tie-breaker (bigger binaries are usually the engine/recomp, capped).
        let mb = fs::metadata(&p).map(|m| m.len() / (1024 * 1024)).unwrap_or(0).min(40) as i64;
        score += mb;

        if best.as_ref().map_or(true, |(bs, _)| score > *bs) {
            best = Some((score, p));
        }
    }

    let (_, exe) = best?;
    let exe_name = exe.file_name()?.to_string_lossy().to_string();
    let cat = match_catalog_title(folder_name).or_else(|| match_catalog_title(&exe_name));
    let is_recomp = default_recomp
        || cat.map_or(false, |c| c.kind == PortKind::Recomp)
        || looks_like_recomp(folder_name, Some(dir))
        || looks_like_recomp(&exe_name, exe.parent());
    let kind = if is_recomp { PortKind::Recomp } else { PortKind::Port };
    let name = cat.map(|c| c.title.to_string()).unwrap_or_else(|| clean_port_title(folder_name));
    if name.is_empty() {
        return None;
    }
    Some(PortGame {
        kind,
        name,
        launch: exe.clone(),
        args: vec![],
        workdir: exe.parent().map(Path::to_path_buf),
        origin_system: cat.and_then(|c| c.origin).map(String::from),
        source: "folder",
    })
}

fn collect_launchables(dir: &Path, depth: usize, exes: &mut Vec<(PathBuf, usize)>, lnks: &mut Vec<PathBuf>) {
    if depth > 2 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let low = name.to_ascii_lowercase();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if matches!(low.as_str(), "redist" | "_commonredist" | "directx" | "vcredist" | "crashreport" | "logs" | "saves" | "mods" | "textures" | "assets") {
                continue;
            }
            collect_launchables(&p, depth + 1, exes, lnks);
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if ext == "lnk" && depth == 0 {
            lnks.push(p);
        } else if cfg!(windows) {
            if ext == "exe" {
                exes.push((p, depth));
            }
        } else if matches!(ext.as_str(), "appimage" | "sh" | "x86_64" | "exe") || is_unix_executable(&p) {
            exes.push((p, depth));
        }
    }
}

#[cfg(unix)]
fn is_unix_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}
#[cfg(not(unix))]
fn is_unix_executable(_p: &Path) -> bool {
    false
}

fn game_from_lnk(lnk_path: &Path, raw_name: &str, default_recomp: bool, source: &'static str) -> Option<PortGame> {
    let parsed = parse_lnk(lnk_path).unwrap_or_default();
    let target = parsed.target.clone().filter(|t| t.is_file());
    let launch = target.clone().unwrap_or_else(|| lnk_path.to_path_buf());
    let target_dir = target.as_ref().and_then(|t| t.parent());
    let target_exe = target
        .as_ref()
        .and_then(|t| t.file_name())
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    // Check shortcut title before exe so a specific shortcut ("Gravity Rush 2") isn't
    // misclassified as a shared launcher exe ("Gravity Rush Launcher.exe").
    let cat = match_catalog_title(raw_name).or_else(|| match_catalog_exe(&target_exe));
    let kind = cat.map(|c| c.kind).unwrap_or_else(|| {
        if default_recomp || looks_like_recomp(raw_name, target_dir) || looks_like_recomp(&target_exe, target_dir) {
            PortKind::Recomp
        } else {
            PortKind::Port
        }
    });
    let name = cat
        .map(|c| c.title.to_string())
        .unwrap_or_else(|| clean_port_title(raw_name));
    if name.is_empty() {
        return None;
    }
    let workdir = parsed.workdir.or_else(|| target_dir.map(Path::to_path_buf));
    Some(PortGame {
        kind,
        name,
        launch,
        args: parsed.args,
        workdir,
        origin_system: cat.and_then(|c| c.origin).map(String::from),
        source,
    })
}

/// Scan multiple dedicated port folders, remembering offline folders via cache (like `movies::scan_many`).
pub fn scan_port_roots(roots: &[PathBuf], cache: &Path) -> (Library, Vec<PathBuf>) {
    crate::scan::scan_many_with(roots, cache, |r| Some(ports_to_library(r, scan_port_folder(r))))
}

fn ports_to_library(root: &Path, games: Vec<PortGame>) -> Library {
    let mut lib = Library { root: root.to_path_buf(), systems: vec![], notes: vec![], game_count: 0 };
    merge(&mut lib, games);
    lib
}

// ---------------------------------------------------------------------------
// Layer 2: Zero-setup high-precision auto-detection (catalog + recomp markers)
// ---------------------------------------------------------------------------

/// Shortcut directories (Desktop + Start Menu) on Windows.
pub fn shortcut_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(windows)]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            let h = PathBuf::from(&home);
            dirs.push(h.join("Desktop"));
            dirs.push(h.join("OneDrive").join("Desktop"));
            dirs.push(h.join("AppData").join("Roaming").join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
        }
        if let Ok(pub_dir) = std::env::var("PUBLIC") {
            dirs.push(PathBuf::from(pub_dir).join("Desktop"));
        }
        if let Ok(pd) = std::env::var("ProgramData") {
            dirs.push(PathBuf::from(pd).join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let h = PathBuf::from(home);
            dirs.push(h.join("Desktop"));
            dirs.push(h.join(".local/share/applications"));
        }
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

/// Auto-detect known recomps and fan ports without false positives:
///   1. Desktop & Start Menu shortcuts matching the known catalog OR recomp markers.
///   2. Common game/port directories (shallow scan for catalog exes & recomp markers).
///   3. OpenGOAL's installed Jak games (via `%APPDATA%\OpenGOAL`).
///   4. Steam non-Steam shortcuts (`shortcuts.vdf`) that match a recomp/port or tag.
pub fn auto_detect(rom_roots: &[PathBuf]) -> Vec<PortGame> {
    let mut out = Vec::new();

    // 1. Shortcuts on Desktop & Start Menu (high-precision filter only!).
    for dir in shortcut_dirs() {
        scan_shortcuts_for_known(&dir, 0, &mut out);
    }

    // 2. Shallow scan of common game / port folders (depth <= 2).
    let mut search = Vec::new();
    for r in rom_roots {
        if let Some(p) = r.parent() {
            for sub in ["Ports", "Recomps", "PC Ports", "Games"] {
                search.push(p.join(sub));
            }
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let h = PathBuf::from(&home);
        for sub in ["Ports", "Recomps", "Games", "Applications"] {
            search.push(h.join(sub));
        }
    }
    for drive in ["C:\\Games", "D:\\Games", "C:\\Recomps", "D:\\Recomps", "C:\\Ports", "D:\\Ports"] {
        search.push(PathBuf::from(drive));
    }
    let mut seen_dirs = HashSet::new();
    for dir in search {
        let key = norm_path(&dir);
        if dir.is_dir() && seen_dirs.insert(key) {
            scan_dir_for_known(&dir, 0, &mut out);
        }
    }

    // 3. Steam non-Steam shortcuts (only entries matching catalog/recomp/port).
    if let Some(root) = crate::stores::steam_root() {
        out.extend(steam_non_steam_ports(&root));
    }

    out
}

fn scan_shortcuts_for_known(dir: &Path, depth: usize, out: &mut Vec<PortGame>) {
    if depth > 2 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_shortcuts_for_known(&p, depth + 1, out);
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if ext != "lnk" {
            continue;
        }
        let raw = e.file_name().to_string_lossy().to_string();
        let parsed = parse_lnk(&p).unwrap_or_default();
        let raw_target = parsed.target.clone();

        // If the shortcut points to a folder (e.g. "Gravity Rush 2 - Shortcut" -> game dir),
        // pick the game executable inside that folder.
        if let Some(dir_target) = raw_target.as_ref().filter(|t| t.is_dir()) {
            let title_cat = match_catalog_title(&raw);
            if title_cat.is_some() || looks_like_recomp(&raw, Some(dir_target)) {
                if let Some(mut g) = detect_game_in_subfolder(dir_target, &raw, title_cat.map_or(false, |c| c.kind == PortKind::Recomp)) {
                    if let Some(cat) = title_cat {
                        g.name = cat.title.to_string();
                        g.kind = cat.kind;
                        g.origin_system = cat.origin.map(String::from);
                    }
                    g.source = "catalog";
                    out.push(g);
                }
            }
            continue;
        }

        let target = raw_target.filter(|t| t.is_file());
        let exe_low = target
            .as_ref()
            .and_then(|t| t.file_name())
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if !exe_low.is_empty() && is_rejected_exe(&exe_low) {
            continue;
        }
        let target_dir = target.as_deref().and_then(|t| t.parent());

        // Check shortcut title first so a game-specific shortcut ("Gravity Rush 2™ - Shortcut")
        // isn't swallowed by a shared launcher exe ("Gravity Rush Launcher.exe").
        let cat = match_catalog_title(&raw).or_else(|| (!exe_low.is_empty()).then(|| match_catalog_exe(&exe_low)).flatten());
        if let Some(cat) = cat {
            let launch = target.clone().unwrap_or_else(|| p.clone());
            let args = if target.is_some() { parsed.args } else { vec![] };
            out.push(PortGame {
                kind: cat.kind,
                name: cat.title.to_string(),
                launch,
                args,
                workdir: parsed.workdir.or_else(|| target_dir.map(Path::to_path_buf)),
                origin_system: cat.origin.map(String::from),
                source: "catalog",
            });
        } else if let Some(ref target) = target {
            if looks_like_recomp(&raw, target_dir) || looks_like_recomp(&exe_low, target_dir) {
                let name = clean_port_title(&raw);
                if !name.is_empty() && !is_non_game_app(&name, exe_low.strip_suffix(".exe").unwrap_or(&exe_low)) {
                    out.push(PortGame {
                        kind: PortKind::Recomp,
                        name,
                        launch: target.clone(),
                        args: parsed.args,
                        workdir: parsed.workdir.or_else(|| target_dir.map(Path::to_path_buf)),
                        origin_system: None,
                        source: "catalog",
                    });
                }
            }
        }
    }
}

fn scan_dir_for_known(dir: &Path, depth: usize, out: &mut Vec<PortGame>) {
    if depth > 2 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_dir_for_known(&p, depth + 1, out);
            continue;
        }
        let fname = e.file_name().to_string_lossy().to_string();
        let low = fname.to_ascii_lowercase();
        if !(low.ends_with(".exe") || low.ends_with(".appimage")) || is_rejected_exe(&low) {
            continue;
        }
        if let Some(cat) = match_catalog_exe(&low) {
            out.push(PortGame {
                kind: cat.kind,
                name: cat.title.to_string(),
                launch: p.clone(),
                args: vec![],
                workdir: p.parent().map(Path::to_path_buf),
                origin_system: cat.origin.map(String::from),
                source: "catalog",
            });
        } else if looks_like_recomp(&low, p.parent()) {
            let folder_label = p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| fname.clone());
            let name = clean_port_title(if low.contains("recomp") { &fname } else { &folder_label });
            if !name.is_empty() {
                out.push(PortGame {
                    kind: PortKind::Recomp,
                    name,
                    launch: p.clone(),
                    args: vec![],
                    workdir: p.parent().map(Path::to_path_buf),
                    origin_system: None,
                    source: "catalog",
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Layer 3a: Steam non-Steam shortcuts (`userdata/<id>/config/shortcuts.vdf`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SteamShortcut {
    pub app_name: String,
    pub exe: String,
    pub start_dir: String,
    pub launch_options: String,
    pub tags: Vec<String>,
}

/// Parse Valve's binary VDF `shortcuts.vdf` file.
pub fn parse_shortcuts_vdf(bytes: &[u8]) -> Vec<SteamShortcut> {
    let mut out = Vec::new();
    let mut pos = 0usize;

    fn read_cstr(bytes: &[u8], pos: &mut usize) -> Option<String> {
        let start = *pos;
        let end = bytes[start..].iter().position(|&b| b == 0)?;
        *pos = start + end + 1;
        Some(String::from_utf8_lossy(&bytes[start..start + end]).into_owned())
    }

    fn skip_map(bytes: &[u8], pos: &mut usize, depth: usize) -> Option<()> {
        if depth > 16 {
            return None;
        }
        while *pos < bytes.len() {
            let ty = bytes[*pos];
            *pos += 1;
            if ty == 0x08 {
                return Some(());
            }
            let _key = read_cstr(bytes, pos)?;
            match ty {
                0x00 => skip_map(bytes, pos, depth + 1)?,
                0x01 => { let _ = read_cstr(bytes, pos)?; }
                0x02 => { *pos = pos.checked_add(4)?; }
                _ => return None,
            }
        }
        None
    }

    // Top level: 0x00 "shortcuts" \0 <entries...> 0x08 0x08
    if bytes.is_empty() || bytes[0] != 0x00 {
        return out;
    }
    pos += 1;
    if read_cstr(bytes, &mut pos).is_none() {
        return out;
    }

    while pos < bytes.len() {
        let ty = bytes[pos];
        pos += 1;
        if ty == 0x08 {
            break;
        }
        if ty != 0x00 {
            break;
        }
        if read_cstr(bytes, &mut pos).is_none() {
            break;
        }
        let mut sc = SteamShortcut::default();
        while pos < bytes.len() {
            let fty = bytes[pos];
            pos += 1;
            if fty == 0x08 {
                break;
            }
            let Some(key) = read_cstr(bytes, &mut pos) else { return out };
            let kl = key.to_ascii_lowercase();
            match fty {
                0x01 => {
                    let Some(val) = read_cstr(bytes, &mut pos) else { return out };
                    match kl.as_str() {
                        "appname" => sc.app_name = val,
                        "exe" => sc.exe = val.trim().trim_matches('"').to_string(),
                        "startdir" => sc.start_dir = val.trim().trim_matches('"').to_string(),
                        "launchoptions" => sc.launch_options = val,
                        _ => {}
                    }
                }
                0x02 => {
                    if pos + 4 > bytes.len() {
                        return out;
                    }
                    pos += 4;
                }
                0x00 => {
                    if kl == "tags" {
                        while pos < bytes.len() {
                            let tty = bytes[pos];
                            pos += 1;
                            if tty == 0x08 {
                                break;
                            }
                            if read_cstr(bytes, &mut pos).is_none() {
                                return out;
                            }
                            match tty {
                                0x01 => {
                                    if let Some(t) = read_cstr(bytes, &mut pos) {
                                        if !t.trim().is_empty() {
                                            sc.tags.push(t);
                                        }
                                    }
                                }
                                0x02 => {
                                    if pos + 4 > bytes.len() {
                                        return out;
                                    }
                                    pos += 4;
                                }
                                0x00 => {
                                    if skip_map(bytes, &mut pos, 1).is_none() {
                                        return out;
                                    }
                                }
                                _ => return out,
                            }
                        }
                    } else if skip_map(bytes, &mut pos, 1).is_none() {
                        return out;
                    }
                }
                _ => return out,
            }
        }
        if !sc.app_name.trim().is_empty() && !sc.exe.trim().is_empty() {
            out.push(sc);
        }
    }
    out
}

pub fn steam_non_steam_ports(steam_root: &Path) -> Vec<PortGame> {
    let userdata = steam_root.join("userdata");
    let Ok(rd) = fs::read_dir(&userdata) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let vdf = e.path().join("config").join("shortcuts.vdf");
        let Ok(bytes) = fs::read(&vdf) else { continue };
        for sc in parse_shortcuts_vdf(&bytes) {
            let exe_path = PathBuf::from(&sc.exe);
            if !exe_path.exists() {
                continue;
            }
            let exe_low = exe_path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if is_rejected_exe(&exe_low) {
                continue;
            }
            let tag_recomp = sc.tags.iter().any(|t| t.to_ascii_lowercase().contains("recomp"));
            let tag_port = sc.tags.iter().any(|t| t.to_ascii_lowercase().contains("port") || t.to_ascii_lowercase().contains("decomp"));
            let cat = match_catalog_exe(&exe_low).or_else(|| match_catalog_title(&sc.app_name));
            let is_recomp = tag_recomp
                || cat.map_or(false, |c| c.kind == PortKind::Recomp)
                || looks_like_recomp(&sc.app_name, exe_path.parent())
                || looks_like_recomp(&exe_low, exe_path.parent());
            let is_port = tag_port
                || cat.map_or(false, |c| c.kind == PortKind::Port)
                || sc.app_name.to_ascii_lowercase().contains("pc port")
                || sc.app_name.to_ascii_lowercase().contains("decomp");
            if !is_recomp && !is_port {
                continue;
            }
            let name = cat.map(|c| c.title.to_string()).unwrap_or_else(|| clean_port_title(&sc.app_name));
            if name.is_empty() {
                continue;
            }
            let workdir = (!sc.start_dir.is_empty())
                .then(|| PathBuf::from(&sc.start_dir))
                .filter(|d| d.is_dir())
                .or_else(|| exe_path.parent().map(Path::to_path_buf));
            out.push(PortGame {
                kind: if is_recomp { PortKind::Recomp } else { PortKind::Port },
                name,
                launch: exe_path,
                args: split_cmdline(&sc.launch_options),
                workdir,
                origin_system: cat.and_then(|c| c.origin).map(String::from),
                source: "steam",
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Layer 3b: Desktop / Start Menu shortcut candidate listing for Settings UI
// ---------------------------------------------------------------------------

/// List non-store Desktop / Start Menu shortcuts so the user can import any
/// recomp or port (like a new fan port or launcher) in one click.
pub fn desktop_shortcut_candidates(existing_pc_games: &[Game]) -> Vec<ShortcutCandidate> {
    let mut existing_keys: HashSet<String> = HashSet::new();
    let mut existing_targets: HashSet<String> = HashSet::new();
    for g in existing_pc_games {
        existing_keys.insert(title::match_key(&g.title));
        existing_targets.insert(norm_path(&g.path));
    }

    let mut seen_targets: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for (idx, dir) in shortcut_dirs().into_iter().enumerate() {
        let is_desktop = dir.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("desktop")).unwrap_or(false);
        // Only scan Desktop directly (depth 0) and Start Menu (depth 1) so the list stays crisp.
        collect_shortcut_candidates(
            &dir,
            if is_desktop { 0 } else { 1 },
            0,
            if is_desktop { "Desktop shortcut" } else { "Start Menu" },
            &existing_keys,
            &existing_targets,
            &mut seen_targets,
            &mut out,
            idx == 0,
        );
    }

    // Sort: not-yet-added first, known ports/recomps first, then A–Z.
    out.sort_by(|a, b| {
        a.already_added
            .cmp(&b.already_added)
            .then_with(|| (b.reason == "Known recomp" || b.reason == "Known port").cmp(&(a.reason == "Known recomp" || a.reason == "Known port")))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    out
}

#[allow(clippy::too_many_arguments)]
fn collect_shortcut_candidates(
    dir: &Path,
    max_depth: usize,
    depth: usize,
    origin_label: &str,
    existing_keys: &HashSet<String>,
    existing_targets: &HashSet<String>,
    seen_targets: &mut HashSet<String>,
    out: &mut Vec<ShortcutCandidate>,
    _primary: bool,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_shortcut_candidates(&p, max_depth, depth + 1, origin_label, existing_keys, existing_targets, seen_targets, out, _primary);
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if ext != "lnk" {
            continue;
        }
        let raw = e.file_name().to_string_lossy().to_string();
        let Some(parsed) = parse_lnk(&p) else { continue };
        let Some(target) = parsed.target.clone().filter(|t| t.exists()) else { continue };
        let exe_low = target.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !exe_low.ends_with(".exe") || is_rejected_exe(&exe_low) {
            continue;
        }
        let target_low = norm_path(&target);
        // Skip Windows system binaries and common store launcher stubs.
        if target_low.contains("\\windows\\") || target_low.contains("\\steamapps\\common\\") || target_low.contains("\\epic games\\") {
            continue;
        }
        let stem = exe_low.strip_suffix(".exe").unwrap_or(&exe_low);
        let cat = match_catalog_title(&raw).or_else(|| match_catalog_exe(&exe_low));
        let clean = cat.map(|c| c.title.to_string()).unwrap_or_else(|| clean_port_title(&raw));
        if clean.is_empty() {
            continue;
        }
        if cat.is_none() && is_non_game_app(&clean, stem) {
            continue;
        }
        let dedup_key = format!("{target_low}|{}", parsed.args.join(" "));
        if !seen_targets.insert(dedup_key) {
            continue;
        }
        let kind = cat.map(|c| c.kind).unwrap_or_else(|| {
            if looks_like_recomp(&raw, target.parent()) || looks_like_recomp(&exe_low, target.parent()) {
                PortKind::Recomp
            } else {
                PortKind::Port
            }
        });
        let reason = match cat {
            Some(c) if c.kind == PortKind::Recomp => "Known recomp".to_string(),
            Some(_) => "Known port".to_string(),
            None if kind == PortKind::Recomp => "Recomp detected".to_string(),
            None => origin_label.to_string(),
        };
        let already_added = existing_targets.contains(&target_low) || existing_keys.contains(&title::match_key(&clean));
        out.push(ShortcutCandidate {
            path: p.to_string_lossy().to_string(),
            name: clean,
            target: target.to_string_lossy().to_string(),
            args: parsed.args,
            workdir: parsed.workdir.or_else(|| target.parent().map(Path::to_path_buf)).map(|w| w.to_string_lossy().to_string()),
            kind,
            origin_system: cat.and_then(|c| c.origin).map(String::from),
            reason,
            already_added,
        });
    }
}

/// Build a `CustomPort` from a user-selected `.exe` or `.lnk` file.
pub fn custom_port_from_file(path: &Path, kind_override: Option<PortKind>) -> Option<CustomPort> {
    let raw = path.file_name()?.to_string_lossy().to_string();
    let ext = path.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext == "lnk" {
        let g = game_from_lnk(path, &raw, kind_override == Some(PortKind::Recomp), "manual")?;
        return Some(CustomPort {
            path: g.launch.to_string_lossy().to_string(),
            name: g.name,
            kind: kind_override.unwrap_or(g.kind),
            args: g.args,
            workdir: g.workdir.map(|w| w.to_string_lossy().to_string()),
            origin_system: g.origin_system,
        });
    }
    let low = raw.to_ascii_lowercase();
    let cat = match_catalog_exe(&low).or_else(|| match_catalog_title(&raw));
    let kind = kind_override.or_else(|| cat.map(|c| c.kind)).unwrap_or_else(|| {
        if looks_like_recomp(&raw, path.parent()) {
            PortKind::Recomp
        } else {
            PortKind::Port
        }
    });
    // Prefer the parent folder's name if the exe name is generic like "game.exe" or "launcher.exe".
    let stem = low.strip_suffix(".exe").unwrap_or(&low);
    let generic_exe = matches!(stem, "game" | "main" | "start" | "play" | "launcher" | "client" | "app");
    let raw_for_title = if generic_exe {
        path.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or(raw)
    } else {
        raw
    };
    let name = cat.map(|c| c.title.to_string()).unwrap_or_else(|| clean_port_title(&raw_for_title));
    if name.is_empty() {
        return None;
    }
    Some(CustomPort {
        path: path.to_string_lossy().to_string(),
        name,
        kind,
        args: vec![],
        workdir: path.parent().map(|w| w.to_string_lossy().to_string()),
        origin_system: cat.and_then(|c| c.origin).map(String::from),
    })
}

/// Convert saved `CustomPort` entries into `PortGame`s for merging.
pub fn from_custom(list: &[CustomPort]) -> Vec<PortGame> {
    list.iter()
        .map(|c| {
            let launch = PathBuf::from(&c.path);
            let workdir = c.workdir.as_ref().map(PathBuf::from).or_else(|| launch.parent().map(Path::to_path_buf));
            PortGame {
                kind: c.kind,
                name: c.name.clone(),
                launch,
                args: c.args.clone(),
                workdir,
                origin_system: c.origin_system.clone(),
                source: "manual",
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Merging into the PC Games system
// ---------------------------------------------------------------------------

/// Ensure a recomp/port game carries `"Recomp"` or `"Port"` in its clickable `meta.tags`.
pub fn ensure_tag(game: &mut Game) {
    let tag = match game.format.as_str() {
        "recomp" => "Recomp",
        "port" => "Port",
        _ => return,
    };
    let m = game.meta.get_or_insert_with(GameMeta::default);
    if !m.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
        m.tags.insert(0, tag.to_string());
    }
}

/// Add detected/manual recomps and ports into the `pc` system with `format = "recomp" | "port"`.
/// Returns how many new games were added.
pub fn merge(lib: &mut Library, games: Vec<PortGame>) -> usize {
    if games.is_empty() {
        return 0;
    }
    if !lib.systems.iter().any(|s| s.id == "pc") {
        let sys = systems::SYSTEMS.iter().find(|s| s.id == "pc").expect("pc system");
        lib.systems.push(SystemLibrary {
            id: sys.id,
            name: sys.name,
            short: sys.short,
            color: sys.color,
            folders: vec![],
            games: vec![],
        });
        lib.systems.sort_by_key(|s| systems::SYSTEMS.iter().position(|x| x.id == s.id).unwrap_or(usize::MAX));
    }
    let pc = lib.systems.iter_mut().find(|s| s.id == "pc").unwrap();
    let mut added = 0;

    for pg in games {
        let target_norm = norm_path(&pg.launch);
        let key = title::match_key(&pg.name);
        let dup = pc.games.iter().any(|g| {
            let same_target = norm_path(&g.path) == target_norm && g.launch_args == pg.args;
            let same_title = title::match_key(&g.title) == key;
            same_target || (same_title && (pg.source != "manual" || matches!(g.format.as_str(), "recomp" | "port")))
        });
        if dup {
            continue;
        }

        let sort_key = title::sort_key(&pg.name);
        let mut id = format!("pc/{}", slug(&sort_key));
        if pc.games.iter().any(|g| g.id == id) {
            id = format!("{id}-{}", pg.kind.as_str());
        }
        let mut n = 2usize;
        let base_id = id.clone();
        while pc.games.iter().any(|g| g.id == id) {
            id = format!("{base_id}-{n}");
            n += 1;
        }

        let fmt = pg.kind.as_str().to_string();
        let mut g = Game {
            id,
            letter: title::index_letter(&sort_key),
            title: pg.name.clone(),
            sort_key,
            path: pg.launch.clone(),
            format: fmt.clone(),
            region: None,
            revision: None,
            title_id: None,
            versions: vec![Version { path: pg.launch, format: fmt, size: 0, disc: None }],
            updates: 0,
            dlc: 0,
            cover: None,
            video: None,
            meta: Some(GameMeta { tags: vec![pg.kind.tag().to_string()], ..Default::default() }),
            info: None,
            warnings: vec![],
            episodes: vec![],
            launch_args: pg.args,
            workdir: pg.workdir,
            origin_system: pg.origin_system,
        };
        ensure_tag(&mut g);
        pc.games.push(g);
        added += 1;
    }

    pc.games.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
    lib.game_count = lib.systems.iter().map(|s| s.games.len()).sum();
    added
}

fn norm_path(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
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

    /// Build a minimal valid MS-SHLLINK binary with ANSI LocalBasePath, WorkingDir, and Arguments.
    fn make_lnk(target: &str, workdir: Option<&str>, args: Option<&str>) -> Vec<u8> {
        let mut b = vec![0u8; 76];
        b[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        b[4..20].copy_from_slice(&[
            0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ]);
        let mut flags: u32 = 0x02 | 0x80; // HasLinkInfo | IsUnicode
        if workdir.is_some() { flags |= 0x10; }
        if args.is_some() { flags |= 0x20; }
        b[20..24].copy_from_slice(&flags.to_le_bytes());

        // LinkInfo: 28-byte header + ANSI target + NUL + empty suffix NUL
        let base_off: u32 = 28;
        let suffix_off: u32 = base_off + target.len() as u32 + 1;
        let info_size: u32 = suffix_off + 1;
        let mut info = vec![0u8; info_size as usize];
        info[0..4].copy_from_slice(&info_size.to_le_bytes());
        info[4..8].copy_from_slice(&28u32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes()); // VolumeIDAndLocalBasePath
        info[16..20].copy_from_slice(&base_off.to_le_bytes());
        info[24..28].copy_from_slice(&suffix_off.to_le_bytes());
        info[28..28 + target.len()].copy_from_slice(target.as_bytes());
        b.extend_from_slice(&info);

        let push_u16_str = |buf: &mut Vec<u8>, s: &str| {
            let units: Vec<u16> = s.encode_utf16().collect();
            buf.extend_from_slice(&(units.len() as u16).to_le_bytes());
            for u in units {
                buf.extend_from_slice(&u.to_le_bytes());
            }
        };
        if let Some(wd) = workdir {
            push_u16_str(&mut b, wd);
        }
        if let Some(a) = args {
            push_u16_str(&mut b, a);
        }
        b
    }

    #[test]
    fn parses_lnk_target_args_and_workdir() {
        let bytes = make_lnk(
            r"D:\Ports\GravityRush2\GravityRush2.exe",
            Some(r"D:\Ports\GravityRush2"),
            Some(r#"--fullscreen --save-dir "C:\My Saves""#),
        );
        let p = parse_lnk_bytes(&bytes, None).unwrap();
        assert_eq!(p.target, Some(PathBuf::from(r"D:\Ports\GravityRush2\GravityRush2.exe")));
        assert_eq!(p.workdir, Some(PathBuf::from(r"D:\Ports\GravityRush2")));
        assert_eq!(p.args, vec!["--fullscreen", "--save-dir", r"C:\My Saves"]);
    }

    #[test]
    fn cleans_port_and_recomp_titles() {
        assert_eq!(clean_port_title("Gravity Rush 2™ - Shortcut.lnk"), "Gravity Rush 2");
        assert_eq!(clean_port_title("Zelda64Recompiled.exe"), "Zelda 64");
        assert_eq!(clean_port_title("Super Mario 64 (PC Port) [v1.2]"), "Super Mario 64");
        assert_eq!(clean_port_title("Sonic Unleashed Recompiled Win64"), "Sonic Unleashed");
    }

    #[test]
    fn scans_port_folder_and_skips_uninstallers() {
        let base = std::env::temp_dir().join(format!("spine-ports-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);

        // 1. Known recomp subfolder with an uninstaller + real exe
        let zelda = base.join("Zelda 64 Recomp");
        fs::create_dir_all(&zelda).unwrap();
        fs::write(zelda.join("unins000.exe"), b"junk").unwrap();
        fs::write(zelda.join("Zelda64Recompiled.exe"), b"recomp").unwrap();

        // 2. Unknown fan port subfolder with helper + real game exe
        let gr = base.join("Gravity Rush 2");
        fs::create_dir_all(&gr).unwrap();
        fs::write(gr.join("CrashHandler.exe"), b"junk").unwrap();
        fs::write(gr.join("vcredist_x64.exe"), b"junk").unwrap();
        fs::write(gr.join("GravityRush2.exe"), b"game").unwrap();

        // 3. Generic future N64Recomp detected by recompcontrollerdb.txt marker
        let fzero = base.join("F-Zero X");
        fs::create_dir_all(&fzero).unwrap();
        fs::write(fzero.join("recompcontrollerdb.txt"), b"03000000...").unwrap();
        fs::write(fzero.join("FZeroX.exe"), b"game").unwrap();

        let found = scan_port_folder(&base);
        assert_eq!(found.len(), 3, "{found:?}");

        let z = found.iter().find(|g| g.name.contains("Majora")).unwrap();
        assert_eq!(z.kind, PortKind::Recomp);
        assert_eq!(z.origin_system.as_deref(), Some("n64"));
        assert!(z.launch.ends_with("Zelda64Recompiled.exe"));

        let g = found.iter().find(|g| g.name == "Gravity Rush 2").unwrap();
        assert_eq!(g.kind, PortKind::Port);
        assert!(g.launch.ends_with("GravityRush2.exe"));

        let f = found.iter().find(|g| g.name == "F-Zero X").unwrap();
        assert_eq!(f.kind, PortKind::Recomp);
        assert!(f.launch.ends_with("FZeroX.exe"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn parses_binary_steam_shortcuts_vdf() {
        let mut vdf = Vec::new();
        let push_s = |v: &mut Vec<u8>, k: &str, val: &str| {
            v.push(0x01);
            v.extend_from_slice(k.as_bytes());
            v.push(0);
            v.extend_from_slice(val.as_bytes());
            v.push(0);
        };
        vdf.push(0x00);
        vdf.extend_from_slice(b"shortcuts\0");
        vdf.push(0x00);
        vdf.extend_from_slice(b"0\0");
        push_s(&mut vdf, "AppName", "Unleashed Recompiled");
        push_s(&mut vdf, "Exe", "\"C:\\Games\\Unleashed\\UnleashedRecomp.exe\"");
        push_s(&mut vdf, "StartDir", "\"C:\\Games\\Unleashed\"");
        push_s(&mut vdf, "LaunchOptions", "--fullscreen");
        vdf.push(0x00);
        vdf.extend_from_slice(b"tags\0");
        push_s(&mut vdf, "0", "Recomp");
        vdf.push(0x08);
        vdf.push(0x08);
        vdf.push(0x08);
        vdf.push(0x08);

        let parsed = parse_shortcuts_vdf(&vdf);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].app_name, "Unleashed Recompiled");
        assert_eq!(parsed[0].exe, r"C:\Games\Unleashed\UnleashedRecomp.exe");
        assert_eq!(parsed[0].launch_options, "--fullscreen");
        assert_eq!(parsed[0].tags, vec!["Recomp"]);
    }

    #[test]
    fn parses_lnk_win1252_trademark_and_shared_launcher() {
        // Simulate Windows writing '™' as Windows-1252 byte 0x99 in ANSI LocalBasePath.
        let mut raw_target = b"D:\\Games\\Gravity Rush 2".to_vec();
        raw_target.push(0x99);
        raw_target.extend_from_slice(b"\\Gravity Rush 2");
        raw_target.push(0x99);
        raw_target.extend_from_slice(b".exe");

        let mut b = vec![0u8; 76];
        b[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        b[4..20].copy_from_slice(&[
            0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ]);
        b[20..24].copy_from_slice(&0x02u32.to_le_bytes());
        let base_off: u32 = 28;
        let suffix_off: u32 = base_off + raw_target.len() as u32 + 1;
        let info_size: u32 = suffix_off + 1;
        let mut info = vec![0u8; info_size as usize];
        info[0..4].copy_from_slice(&info_size.to_le_bytes());
        info[4..8].copy_from_slice(&28u32.to_le_bytes());
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[16..20].copy_from_slice(&base_off.to_le_bytes());
        info[24..28].copy_from_slice(&suffix_off.to_le_bytes());
        info[28..28 + raw_target.len()].copy_from_slice(&raw_target);
        b.extend_from_slice(&info);

        let p = parse_lnk_bytes(&b, None).unwrap();
        assert_eq!(p.target, Some(PathBuf::from("D:\\Games\\Gravity Rush 2™\\Gravity Rush 2™.exe")));

        // Verify both Gravity Rush 1 and Gravity Rush 2 shortcuts are detected even if they share a launcher exe.
        let base = std::env::temp_dir().join(format!("spine-gr-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let launcher = base.join("Gravity Rush Launcher.exe");
        fs::write(&launcher, b"MZ").unwrap();
        let gr2_exe = base.join("Gravity Rush 2™.exe");
        fs::write(&gr2_exe, b"MZ").unwrap();

        let l1 = make_lnk(&launcher.to_string_lossy(), None, None);
        let l2 = make_lnk(&launcher.to_string_lossy(), None, Some("--game 2"));
        fs::write(base.join("Gravity Rush Launcher - Shortcut.lnk"), &l1).unwrap();
        fs::write(base.join("Gravity Rush 2™ - Shortcut.lnk"), &l2).unwrap();

        let mut found = Vec::new();
        scan_shortcuts_for_known(&base, 0, &mut found);
        let mut lib = Library { root: PathBuf::from("/r"), systems: vec![], notes: vec![], game_count: 0 };
        merge(&mut lib, found);
        let pc = lib.systems.iter().find(|s| s.id == "pc").unwrap();
        let titles: Vec<&str> = pc.games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, vec!["Gravity Rush", "Gravity Rush 2"]);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn merges_ports_under_pc_with_tag() {
        let mut lib = Library { root: PathBuf::from("/r"), systems: vec![], notes: vec![], game_count: 0 };
        let n = merge(&mut lib, vec![
            PortGame {
                kind: PortKind::Recomp,
                name: "Sonic Unleashed".into(),
                launch: PathBuf::from(r"D:\Recomps\Unleashed\UnleashedRecomp.exe"),
                args: vec!["--fullscreen".into()],
                workdir: Some(PathBuf::from(r"D:\Recomps\Unleashed")),
                origin_system: Some("xbox360".into()),
                source: "catalog",
            },
            PortGame {
                kind: PortKind::Port,
                name: "Gravity Rush 2".into(),
                launch: PathBuf::from(r"D:\Ports\GR2\GravityRush2.exe"),
                args: vec![],
                workdir: None,
                origin_system: None,
                source: "manual",
            },
        ]);
        assert_eq!(n, 2);
        let pc = lib.systems.iter().find(|s| s.id == "pc").unwrap();
        assert_eq!(pc.games.len(), 2);
        assert_eq!(pc.games[0].title, "Gravity Rush 2");
        assert_eq!(pc.games[0].format, "port");
        assert_eq!(pc.games[0].meta.as_ref().unwrap().tags, vec!["Port"]);
        assert_eq!(pc.games[1].title, "Sonic Unleashed");
        assert_eq!(pc.games[1].format, "recomp");
        assert_eq!(pc.games[1].meta.as_ref().unwrap().tags, vec!["Recomp"]);
        assert_eq!(pc.games[1].origin_system.as_deref(), Some("xbox360"));
        assert_eq!(pc.games[1].launch_args, vec!["--fullscreen"]);
    }
}
