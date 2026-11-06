//! Spine Emulators (the separate installer, 0.33): a list of popular emulators, where each
//! one's official Windows build comes from, and how to download, check and unpack it.
//!
//! Sources are written into the program (never taken from the user or a web page), because
//! fake download pages for emulators exist. BIOS files, keys and firmware are never downloaded.
//!
//! Checking a download: the size GitHub lists, the sha256 GitHub lists when it has one
//! (files uploaded since mid-2025), and the archive's own per-file CRC checks while unpacking
//! (zip and 7z both carry them). Each emulator lands in `<folder>/<Name>/`, which Spine's
//! emulator search finds (two levels deep).

use crate::mpvget::sha256_hex;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Written in the emulators folder: what this installer put there.
pub const STAMP: &str = "spine-emulators.json";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    /// GitHub repository; its "latest release".
    GitHub(&'static str),
    /// A Forgejo/Gitea server's releases API (newest non-prerelease).
    Forgejo(&'static str),
}

pub struct EmuDef {
    /// Same id as Spine's `launch::EMULATORS`, so Spine's search names it the same way.
    pub id: &'static str,
    pub name: &'static str,
    pub consoles: &'static str,
    pub group: &'static str,
    pub source: Source,
    /// Picks the 64-bit Windows download from the release's file names.
    pub pick: fn(&str) -> bool,
    /// Program names (lowercase) that must be in the unpacked folder.
    pub exes: &'static [&'static str],
    /// The emulator's own website, shown in the list.
    pub site: &'static str,
}

fn lc(n: &str) -> String {
    n.to_ascii_lowercase()
}

pub const EMUS: &[EmuDef] = &[
    EmuDef { id: "mesen", name: "Mesen", consoles: "NES, SNES, Game Boy, GBA", group: "Retro", source: Source::GitHub("SourMesen/Mesen2"), pick: |n| lc(n).ends_with("_windows.zip"), exes: &["mesen.exe"], site: "https://www.mesen.ca" },
    EmuDef { id: "mgba", name: "mGBA", consoles: "Game Boy, Game Boy Color, GBA", group: "Retro", source: Source::GitHub("mgba-emu/mgba"), pick: |n| lc(n).ends_with("-win64.7z"), exes: &["mgba.exe"], site: "https://mgba.io" },
    EmuDef { id: "flycast", name: "Flycast", consoles: "Dreamcast", group: "Retro", source: Source::GitHub("flyinghead/flycast"), pick: |n| { let n = lc(n); n.starts_with("flycast-win64") && n.ends_with(".zip") }, exes: &["flycast.exe"], site: "https://github.com/flyinghead/flycast" },
    EmuDef { id: "duckstation", name: "DuckStation", consoles: "PlayStation", group: "PlayStation", source: Source::GitHub("stenzek/duckstation"), pick: |n| lc(n) == "duckstation-windows-x64-release.zip", exes: &["duckstation-qt-x64-releaseltcg.exe", "duckstation-qt.exe"], site: "https://www.duckstation.org" },
    EmuDef { id: "pcsx2", name: "PCSX2", consoles: "PlayStation 2", group: "PlayStation", source: Source::GitHub("PCSX2/pcsx2"), pick: |n| { let n = lc(n); n.ends_with("-windows-x64-qt.7z") && !n.contains("symbols") }, exes: &["pcsx2-qt.exe", "pcsx2-qtx64-avx2.exe", "pcsx2-qtx64.exe"], site: "https://pcsx2.net" },
    EmuDef { id: "rpcs3", name: "RPCS3", consoles: "PlayStation 3", group: "PlayStation", source: Source::GitHub("RPCS3/rpcs3-binaries-win"), pick: |n| lc(n).ends_with("_win64_msvc.7z"), exes: &["rpcs3.exe"], site: "https://rpcs3.net" },
    EmuDef { id: "vita3k", name: "Vita3K", consoles: "PlayStation Vita", group: "PlayStation", source: Source::GitHub("Vita3K/Vita3K"), pick: |n| lc(n) == "windows-latest.zip", exes: &["vita3k.exe"], site: "https://vita3k.org" },
    EmuDef { id: "melonds", name: "melonDS", consoles: "Nintendo DS", group: "Nintendo", source: Source::GitHub("melonDS-emu/melonDS"), pick: |n| lc(n).ends_with("-windows-x86_64.zip"), exes: &["melonds.exe"], site: "https://melonds.kuribo64.net" },
    EmuDef { id: "azahar", name: "Azahar", consoles: "Nintendo 3DS", group: "Nintendo", source: Source::GitHub("azahar-emu/azahar"), pick: |n| { let n = lc(n); n.starts_with("azahar-windows-msvc-") && n.ends_with(".zip") }, exes: &["azahar.exe", "azahar-qt.exe"], site: "https://azahar-emu.org" },
    EmuDef { id: "cemu", name: "Cemu", consoles: "Wii U", group: "Nintendo", source: Source::GitHub("cemu-project/Cemu"), pick: |n| { let n = lc(n); n.starts_with("cemu-") && n.ends_with("-windows-x64.zip") }, exes: &["cemu.exe"], site: "https://cemu.info" },
    EmuDef { id: "eden", name: "Eden", consoles: "Switch", group: "Nintendo", source: Source::Forgejo("https://git.eden-emu.dev/api/v1/repos/eden-emu/eden/releases"), pick: |n| { let n = lc(n); n.starts_with("eden-windows-") && n.ends_with("-amd64-clang-pgo.zip") }, exes: &["eden.exe"], site: "https://eden-emu.dev" },
    EmuDef { id: "xemu", name: "xemu", consoles: "Xbox", group: "Xbox", source: Source::GitHub("xemu-project/xemu"), pick: |n| lc(n) == "xemu-win-x86_64-release.zip", exes: &["xemu.exe"], site: "https://xemu.app" },
    EmuDef { id: "xenia", name: "Xenia Canary", consoles: "Xbox 360", group: "Xbox", source: Source::GitHub("xenia-canary/xenia-canary-releases"), pick: |n| { let n = lc(n); n.starts_with("xenia_canary_windows") && n.ends_with(".zip") }, exes: &["xenia_canary.exe"], site: "https://xenia.jp" },
];

pub fn emu(id: &str) -> Option<&'static EmuDef> {
    EMUS.iter().find(|e| e.id == id)
}

/// The download chosen from a release.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub tag: String,
    pub name: String,
    pub url: String,
    /// 0 = not listed.
    pub size: u64,
    /// Lowercase hex, when the server lists one.
    pub sha256: Option<String>,
}

impl Asset {
    /// What's stored as the installed version: rolling releases ("latest", "continuous")
    /// keep their tag, so the fingerprint tells builds apart.
    pub fn version(&self) -> String {
        match &self.sha256 {
            Some(h) if matches!(self.tag.as_str(), "latest" | "continuous") => format!("{} {}", self.tag, &h[..12]),
            _ => self.tag.clone(),
        }
    }
}

fn read_digest(v: &serde_json::Value) -> Option<String> {
    let hex = v.as_str()?.strip_prefix("sha256:")?.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn pick_asset(release: &serde_json::Value, pick: fn(&str) -> bool) -> Option<Asset> {
    let tag = release["tag_name"].as_str()?.to_string();
    release["assets"].as_array()?.iter().find_map(|a| {
        let name = a["name"].as_str()?;
        if !pick(name) {
            return None;
        }
        let url = a["browser_download_url"].as_str()?;
        if !url.starts_with("https://") {
            return None;
        }
        Some(Asset { tag: tag.clone(), name: name.to_string(), url: url.to_string(), size: a["size"].as_u64().unwrap_or(0), sha256: read_digest(&a["digest"]) })
    })
}

/// A GitHub "latest release" answer.
pub fn parse_github(body: &[u8], pick: fn(&str) -> bool) -> Option<Asset> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    pick_asset(&v, pick)
}

/// A Forgejo "list releases" answer: the newest release that isn't a draft or prerelease
/// and has a matching file.
pub fn parse_forgejo(body: &[u8], pick: fn(&str) -> bool) -> Option<Asset> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    v.as_array()?.iter().filter(|r| !r["prerelease"].as_bool().unwrap_or(false) && !r["draft"].as_bool().unwrap_or(false)).find_map(|r| pick_asset(r, pick))
}

/// Ask the emulator's release server for its newest Windows download.
pub fn latest(def: &EmuDef) -> Result<Asset, String> {
    let (url, gh) = match def.source {
        Source::GitHub(repo) => (format!("https://api.github.com/repos/{repo}/releases/latest"), true),
        Source::Forgejo(api) => (format!("{api}?limit=10"), false),
    };
    let headers = if gh { vec!["Accept: application/vnd.github+json".to_string()] } else { vec![] };
    let body = crate::covers::curl(&url, true, &headers).map_err(|e| {
        if e.contains("403") || e.contains("429") {
            "GitHub is limiting how often it can be asked. Wait an hour and try again.".to_string()
        } else {
            format!("Couldn't reach the download server ({e}).")
        }
    })?;
    let a = match def.source {
        Source::GitHub(_) => parse_github(&body, def.pick),
        Source::Forgejo(_) => parse_forgejo(&body, def.pick),
    };
    a.ok_or_else(|| format!("{}'s newest release has no 64-bit Windows download right now.", def.name))
}

// ---------- what's installed ----------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stamp {
    pub version: String,
    pub file: String,
    pub at: u64,
}

pub fn read_stamps(dir: &Path) -> BTreeMap<String, Stamp> {
    fs::read_to_string(dir.join(STAMP)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_stamp(dir: &Path, id: &str, s: Stamp) {
    let mut all = read_stamps(dir);
    all.insert(id.to_string(), s);
    if let Ok(t) = serde_json::to_string_pretty(&all) {
        let _ = fs::write(dir.join(STAMP), t);
    }
}

/// Where an emulator goes: `<folder>/<Name>`.
pub fn target(dir: &Path, def: &EmuDef) -> PathBuf {
    dir.join(def.name)
}

/// The emulator's program inside its folder, if it's there.
pub fn find_exe(folder: &Path, def: &EmuDef) -> Option<PathBuf> {
    let rd = fs::read_dir(folder).ok()?;
    let names: Vec<(String, PathBuf)> = rd.filter_map(|e| e.ok()).map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path())).collect();
    def.exes.iter().find_map(|x| names.iter().find(|(n, _)| n == x).map(|(_, p)| p.clone()))
}

// ---------- unpacking ----------

fn unzip(archive: &Path, into: &Path) -> Result<(), String> {
    let f = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut file = z.by_index(i).map_err(|e| e.to_string())?;
        // Some Windows zips use "\" between folders (Azahar's does): treat it as "/".
        let name = file.name().replace('\\', "/");
        let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        if parts.is_empty() {
            continue;
        }
        if parts.iter().any(|p| *p == ".." || p.contains(':')) || name.starts_with('/') {
            return Err(format!("unsafe path in archive: {name}"));
        }
        let out = parts.iter().fold(into.to_path_buf(), |p, s| p.join(s));
        if name.ends_with('/') || file.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut w = fs::File::create(&out).map_err(|e| e.to_string())?;
        // Reading to the end checks the file's CRC (an error if it doesn't match).
        std::io::copy(&mut file, &mut w).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}

/// If the archive had one folder holding everything ("azahar-windows-msvc-2126.2/..."),
/// move its contents up so the program sits right in the emulator's folder.
fn flatten(dir: &Path) -> Result<(), String> {
    let entries: Vec<PathBuf> = fs::read_dir(dir).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    if entries.len() != 1 || !entries[0].is_dir() {
        return Ok(());
    }
    let inner = &entries[0];
    let tmp = dir.with_extension("flat");
    let _ = fs::remove_dir_all(&tmp);
    fs::rename(inner, &tmp).map_err(|e| e.to_string())?;
    fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir).map_err(|e| e.to_string())
}

/// Unpack a checked archive into `<folder>/<Name>`: first into `<Name>.new`, then swap,
/// so a failed unpack never leaves a broken emulator behind. Files the user added
/// (settings in a portable folder) in the old copy are not kept: the old folder is
/// moved to `<Name>.old` and only removed after the new one is in place.
pub fn unpack(dir: &Path, def: &EmuDef, archive: &Path, a: &Asset) -> Result<PathBuf, String> {
    let dest = target(dir, def);
    let new = dir.join(format!("{}.new", def.name));
    let _ = fs::remove_dir_all(&new);
    fs::create_dir_all(&new).map_err(|e| format!("Couldn't make a folder for {} ({e}).", def.name))?;
    let lower = lc(&a.name);
    let r = if lower.ends_with(".zip") {
        unzip(archive, &new)
    } else if lower.ends_with(".7z") {
        sevenz_rust2::decompress_file(archive, &new).map_err(|e| e.to_string())
    } else {
        Err("unknown archive type".into())
    };
    if let Err(e) = r.and_then(|_| flatten(&new)) {
        let _ = fs::remove_dir_all(&new);
        return Err(format!("Couldn't unpack {} ({e}).", def.name));
    }
    if find_exe(&new, def).is_none() {
        let _ = fs::remove_dir_all(&new);
        return Err(format!("The download didn't contain {}'s program.", def.name));
    }
    let old = dir.join(format!("{}.old", def.name));
    if dest.exists() {
        let _ = fs::remove_dir_all(&old);
        if fs::rename(&dest, &old).is_err() {
            let _ = fs::remove_dir_all(&new);
            return Err(format!("{} is still open. Close it and try again.", def.name));
        }
    }
    if let Err(e) = fs::rename(&new, &dest) {
        let _ = fs::rename(&old, &dest);
        return Err(format!("Couldn't put {} in place ({e}).", def.name));
    }
    let _ = fs::remove_dir_all(&old);
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    write_stamp(dir, def.id, Stamp { version: a.version(), file: a.name.clone(), at });
    find_exe(&dest, def).ok_or_else(|| "program missing after install".into())
}

/// Size and fingerprint check of a finished download.
pub fn check(archive: &Path, a: &Asset) -> Result<(), String> {
    let bytes = fs::read(archive).map_err(|e| e.to_string())?;
    if a.size > 0 && bytes.len() as u64 != a.size {
        return Err("The download was incomplete. Try again.".into());
    }
    if let Some(h) = &a.sha256 {
        if &sha256_hex(&bytes) != h {
            return Err("The download didn't match its fingerprint, so it wasn't used. Try again.".into());
        }
    }
    Ok(())
}

/// Progress reports during an install.
pub enum Step {
    /// Short status line.
    Text(String),
    /// Download progress: bytes so far, total (0 = unknown).
    Bytes(u64, u64),
}

/// The whole job for one emulator: ask, download, check, unpack.
pub fn install(dir: &Path, def: &EmuDef, step: &dyn Fn(Step)) -> Result<(PathBuf, String), String> {
    step(Step::Text("Finding the newest version…".into()));
    let a = latest(def)?;
    fs::create_dir_all(dir).map_err(|e| format!("Couldn't make the emulators folder ({e})."))?;
    let ext = if lc(&a.name).ends_with(".7z") { "7z" } else { "zip" };
    let archive = dir.join(format!(".{}-download.{ext}", def.id));
    let _ = fs::remove_file(&archive);
    step(Step::Text(format!("Downloading {}…", a.tag)));
    // curl writes the file while we watch its size for the progress bar.
    let (tx, rx) = std::sync::mpsc::channel();
    let (url, path) = (a.url.clone(), archive.clone());
    std::thread::spawn(move || {
        let _ = tx.send(crate::covers::curl_to_file(&url, &path, 1800));
    });
    let r = loop {
        match rx.recv_timeout(std::time::Duration::from_millis(300)) {
            Ok(r) => break r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => step(Step::Bytes(fs::metadata(&archive).map(|m| m.len()).unwrap_or(0), a.size)),
            Err(_) => break Err("download stopped".into()),
        }
    };
    if let Err(e) = r {
        let _ = fs::remove_file(&archive);
        return Err(format!("Download failed ({e})."));
    }
    step(Step::Text("Checking the download…".into()));
    if let Err(e) = check(&archive, &a) {
        let _ = fs::remove_file(&archive);
        return Err(e);
    }
    step(Step::Text("Unpacking…".into()));
    let r = unpack(dir, def, &archive, &a);
    let _ = fs::remove_file(&archive);
    r.map(|p| (p, a.version()))
}

/// The folder the installer uses by default: `Emulators` in the user's home folder
/// (Spine searches it).
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| PathBuf::from(h).join("Emulators"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spine-emus-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// File names seen on the real release pages (Nov 6, 2026): each emulator picks exactly
    /// its 64-bit Windows build.
    #[test]
    fn picks_the_windows_build() {
        let cases: &[(&str, &[&str], &str)] = &[
            ("mesen", &["Mesen_2.1.1_Linux_x64.zip", "Mesen_2.1.1_macOS_x64_Intel.zip", "Mesen_2.1.1_Windows.zip"], "Mesen_2.1.1_Windows.zip"),
            ("mgba", &["mGBA-0.10.5-win32.7z", "mGBA-0.10.5-win64-installer.exe", "mGBA-0.10.5-switch.7z", "mGBA-0.10.5-win64.7z"], "mGBA-0.10.5-win64.7z"),
            ("flycast", &["flycast-2.7.apk", "flycast-macOS-2.7.zip", "flycast-win64-2.7.zip"], "flycast-win64-2.7.zip"),
            ("duckstation", &["duckstation-windows-arm64-release.zip", "duckstation-windows-x64-installer.exe", "duckstation-windows-x64-sse2-release.zip", "duckstation-windows-x64-release-symbols.7z", "duckstation-windows-x64-release.zip"], "duckstation-windows-x64-release.zip"),
            ("pcsx2", &["pcsx2-v2.8.2-windows-x64-installer.exe", "pcsx2-v2.8.2-windows-x64-Qt-symbols.7z", "pcsx2-v2.8.2-windows-x64-Qt.7z"], "pcsx2-v2.8.2-windows-x64-Qt.7z"),
            ("rpcs3", &["rpcs3-v0.0.43-20410-c365ea76_win64_msvc.7z.sha256", "rpcs3-v0.0.43-20410-c365ea76_win64_msvc.7z"], "rpcs3-v0.0.43-20410-c365ea76_win64_msvc.7z"),
            ("vita3k", &["windows-arm64-latest.zip", "ubuntu-latest.zip", "windows-latest.zip"], "windows-latest.zip"),
            ("melonds", &["melonDS-1.1-windows-aarch64.zip", "melonDS-1.1-ubuntu-x86_64.zip", "melonDS-1.1-windows-x86_64.zip"], "melonDS-1.1-windows-x86_64.zip"),
            ("azahar", &["azahar-libretro-windows-x86_64-2126.2.zip", "azahar-windows-msvc-2126.2-installer.exe", "azahar-windows-msys2-2126.2.zip", "azahar-windows-msvc-2126.2.zip"], "azahar-windows-msvc-2126.2.zip"),
            ("cemu", &["cemu-2.6-ubuntu-22.04-x64.zip", "cemu-2.6-macos-12-x64.dmg", "cemu-2.6-windows-x64.zip"], "cemu-2.6-windows-x64.zip"),
            ("eden", &["Eden-Windows-v0.2.1-amd64-gcc-standard.zip", "Eden-Windows-v0.2.1-arm64-clang-pgo.zip", "Eden-Windows-v0.2.1-rog-ally-clang-pgo.zip", "Eden-Linux-v0.2.1-amd64-clang-pgo.AppImage", "Eden-Windows-v0.2.1-amd64-clang-pgo.zip"], "Eden-Windows-v0.2.1-amd64-clang-pgo.zip"),
            ("xemu", &["xemu-0.8.138-dbg-windows-x86_64.zip", "xemu-win-aarch64-release.zip", "xemu-0.8.138-windows-x86_64-pdb.zip", "xemu-win-x86_64-release.zip"], "xemu-win-x86_64-release.zip"),
            ("xenia", &["xenia_canary_linux.tar_.xz", "xenia_canary_windows_.zip"], "xenia_canary_windows_.zip"),
        ];
        assert_eq!(cases.len(), EMUS.len(), "every emulator has a case");
        for (id, names, want) in cases {
            let d = emu(id).unwrap();
            let hits: Vec<&&str> = names.iter().filter(|n| (d.pick)(n)).collect();
            assert_eq!(hits, vec![want], "{id}");
        }
    }

    #[test]
    fn ids_match_spines_emulators() {
        for e in EMUS {
            let l = crate::launch::emulator(e.id).unwrap_or_else(|| panic!("{} missing in launch.rs", e.id));
            assert!(e.exes.iter().all(|x| l.exes.contains(x)), "{}: Spine must look for the same program names", e.id);
        }
    }

    #[test]
    fn reads_github_and_forgejo() {
        let hex = "c7b126b98c19a87fa5f7068adc26d8f59d8e3266cd7ec5d257d039c8984fbbb8";
        let gh = format!(
            r#"{{"tag_name":"latest","assets":[
            {{"name":"duckstation-windows-x64-installer.exe","size":1,"digest":"sha256:{hex}","browser_download_url":"https://x/i.exe"}},
            {{"name":"duckstation-windows-x64-release.zip","size":69900000,"digest":"sha256:{}","browser_download_url":"https://x/r.zip"}}]}}"#,
            hex.to_uppercase()
        );
        let a = parse_github(gh.as_bytes(), emu("duckstation").unwrap().pick).unwrap();
        assert_eq!((a.url.as_str(), a.size, a.sha256.as_deref()), ("https://x/r.zip", 69900000, Some(hex)));
        assert_eq!(a.version(), "latest c7b126b98c19");
        // No digest (older uploads): still usable, checked by size and the archive's CRCs.
        let old = br#"{"tag_name":"v2.6","assets":[{"name":"cemu-2.6-windows-x64.zip","size":5,"digest":null,"browser_download_url":"https://x/c.zip"}]}"#;
        let a = parse_github(old, emu("cemu").unwrap().pick).unwrap();
        assert_eq!((a.sha256.clone(), a.version()), (None, "v2.6".to_string()));
        // Plain http links are refused.
        assert!(parse_github(br#"{"tag_name":"1","assets":[{"name":"cemu-2.6-windows-x64.zip","browser_download_url":"http://x/c.zip"}]}"#, emu("cemu").unwrap().pick).is_none());
        let fj = br#"[
          {"tag_name":"v0.3.0-rc1","prerelease":true,"assets":[{"name":"Eden-Windows-v0.3.0-rc1-amd64-clang-pgo.zip","size":0,"browser_download_url":"https://stable.eden-emu.dev/rc.zip"}]},
          {"tag_name":"v0.2.1","prerelease":false,"assets":[{"name":"Eden-Windows-v0.2.1-amd64-msvc-standard.zip","size":0,"browser_download_url":"https://stable.eden-emu.dev/m.zip"},
             {"name":"Eden-Windows-v0.2.1-amd64-clang-pgo.zip","size":0,"browser_download_url":"https://stable.eden-emu.dev/v0.2.1/Eden-Windows-v0.2.1-amd64-clang-pgo.zip"}]}]"#;
        let a = parse_forgejo(fj, emu("eden").unwrap().pick).unwrap();
        assert_eq!((a.tag.as_str(), a.size, a.sha256.is_none()), ("v0.2.1", 0, true));
        assert!(a.url.ends_with("amd64-clang-pgo.zip"));
    }

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let f = fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (n, b) in files {
            z.start_file(*n, opt).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }

    fn asset(name: &str, path: &Path, digest: bool) -> Asset {
        let b = fs::read(path).unwrap();
        Asset { tag: "v1".into(), name: name.into(), url: "https://x".into(), size: b.len() as u64, sha256: digest.then(|| sha256_hex(&b)) }
    }

    #[test]
    fn unpacks_flattens_and_replaces() {
        let d = tmp("unpack");
        let def = emu("azahar").unwrap();
        let z = d.join("a.zip");
        make_zip(&z, &[("azahar-windows-msvc-2126.2/azahar.exe", b"MZ1"), ("azahar-windows-msvc-2126.2/Qt6Core.dll", b"x")]);
        let a = asset("azahar-windows-msvc-2126.2.zip", &z, true);
        check(&z, &a).unwrap();
        let exe = unpack(&d, def, &z, &a).unwrap();
        assert_eq!(exe, d.join("Azahar").join("azahar.exe"));
        assert!(d.join("Azahar").join("Qt6Core.dll").is_file());
        assert_eq!(read_stamps(&d)["azahar"].version, "v1");
        // Again, as an update: the old copy is replaced, nothing left over.
        make_zip(&z, &[("azahar.exe", b"MZ2")]);
        let mut a2 = asset("azahar-windows-msvc-2127.zip", &z, false);
        a2.tag = "v2".into();
        unpack(&d, def, &z, &a2).unwrap();
        assert_eq!(fs::read(d.join("Azahar").join("azahar.exe")).unwrap(), b"MZ2");
        assert!(!d.join("Azahar").join("Qt6Core.dll").exists());
        assert!(!d.join("Azahar.old").exists() && !d.join("Azahar.new").exists());
        assert_eq!(read_stamps(&d)["azahar"].version, "v2");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn refuses_bad_downloads() {
        let d = tmp("bad");
        let def = emu("cemu").unwrap();
        let z = d.join("c.zip");
        make_zip(&z, &[("readme.txt", b"no program here")]);
        let a = asset("cemu-2.6-windows-x64.zip", &z, true);
        assert!(unpack(&d, def, &z, &a).unwrap_err().contains("didn't contain"));
        assert!(!d.join("Cemu").exists() && !d.join("Cemu.new").exists());
        // Wrong fingerprint / size.
        let mut bad = a.clone();
        bad.sha256 = Some("0".repeat(64));
        assert!(check(&z, &bad).is_err());
        bad.sha256 = None;
        bad.size += 1;
        assert!(check(&z, &bad).is_err());
        // Not an archive at all.
        fs::write(&z, b"<html>not found</html>").unwrap();
        assert!(unpack(&d, def, &z, &a).is_err());
        assert!(!d.join("Cemu").exists());
        let _ = fs::remove_dir_all(&d);
    }

    /// Real downloads (needs the internet; the workspace can fetch GitHub release files):
    /// cargo test -- --ignored live_downloads
    #[test]
    #[ignore]
    fn live_downloads() {
        let d = tmp("live");
        let cases = [
            ("melonds", "https://github.com/melonDS-emu/melonDS/releases/download/1.1/melonDS-1.1-windows-x86_64.zip"),
            ("mgba", "https://github.com/mgba-emu/mgba/releases/download/0.10.5/mGBA-0.10.5-win64.7z"),
            ("xenia", "https://github.com/xenia-canary/xenia-canary-releases/releases/download/02d2cb5/xenia_canary_windows_.zip"),
            ("eden", "https://stable.eden-emu.dev/v0.2.1/Eden-Windows-v0.2.1-amd64-clang-pgo.zip"),
            ("azahar", "https://github.com/azahar-emu/azahar/releases/download/2126.2/azahar-windows-msvc-2126.2.zip"),
            ("pcsx2", "https://github.com/PCSX2/pcsx2/releases/download/v2.8.2/pcsx2-v2.8.2-windows-x64-Qt.7z"),
            ("cemu", "https://github.com/cemu-project/Cemu/releases/download/v2.6/cemu-2.6-windows-x64.zip"),
            ("rpcs3", "https://github.com/RPCS3/rpcs3-binaries-win/releases/download/build-c365ea76a552961ff078ea1a46da9d4b07729674/rpcs3-v0.0.43-20410-c365ea76_win64_msvc.7z"),
        ];
        for (id, url) in cases {
            let def = emu(id).unwrap();
            let name = url.rsplit('/').next().unwrap();
            let f = d.join(name);
            crate::covers::curl_to_file(url, &f, 900).unwrap();
            let a = asset(name, &f, false);
            let exe = unpack(&d, def, &f, &a).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(fs::read(&exe).unwrap().starts_with(b"MZ"), "{id}");
            // Spine's own search finds it.
            let found = crate::launch::Installed::detect(&[d.clone()]);
            assert_eq!(found.path(id), Some(&exe), "{id}");
            let _ = fs::remove_file(&f);
        }
        let _ = fs::remove_dir_all(&d);
    }
}
