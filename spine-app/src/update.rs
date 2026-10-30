//! Updating Spine from its GitHub releases.
//!
//! Each release carries `Spine.exe` and `Spine.exe.sha256`. Spine downloads both,
//! checks the fingerprint, then swaps itself: Windows lets a running program be
//! renamed (not deleted), so the old one becomes Spine.old.exe, the new one takes
//! its place, and Spine restarts. The old file is removed on the next start.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// The repository releases come from ("owner/spine"). Release builds get it from
/// GitHub; a build made on this PC uses the value in update-repo.txt next to Spine.exe.
pub fn repo(app_dir: &Path) -> Option<String> {
    option_env!("SPINE_REPO")
        .map(String::from)
        .or_else(|| fs::read_to_string(app_dir.join("update-repo.txt")).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| valid_repo(s))
}

fn valid_repo(s: &str) -> bool {
    let mut parts = s.split('/');
    let ok = |p: Option<&str>| p.map_or(false, |p| !p.is_empty() && p.len() <= 100 && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')));
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub exe_url: String,
    pub exe_size: u64,
    pub sha_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub available: bool,
    pub notes: String,
}

/// "v0.3.1" -> [0, 3, 1]
fn parse_version(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(latest: &str, current: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

/// The "what's new" part of release notes as plain lines: no Markdown, no install instructions.
pub fn clean_notes(body: &str) -> String {
    let top = body.split("\n---").next().unwrap_or("");
    top.lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', ' ']).replace("**", "").replace('`', ""))
        .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("what's new") && !l.starts_with("Full Changelog"))
        .take(8)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The release file this copy of Spine updates from.
#[cfg(not(target_os = "linux"))]
pub const ASSET: &str = "Spine.exe";
#[cfg(target_os = "linux")]
pub const ASSET: &str = "Spine-x86_64.AppImage";

/// Read GitHub's "latest release" answer.
pub fn parse_release(json: &[u8]) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let assets = v["assets"].as_array()?;
    let find = |name: &str| assets.iter().find(|a| a["name"].as_str().map_or(false, |n| n.eq_ignore_ascii_case(name)));
    let exe = find(ASSET)?;
    let sha = find(&format!("{ASSET}.sha256"))?;
    Some(Release {
        version: v["tag_name"].as_str()?.trim_start_matches('v').to_string(),
        notes: clean_notes(v["body"].as_str().unwrap_or("")),
        exe_url: exe["browser_download_url"].as_str()?.to_string(),
        exe_size: exe["size"].as_u64().unwrap_or(0),
        sha_url: sha["browser_download_url"].as_str()?.to_string(),
    })
}

fn curl(url: &str, out: Option<&Path>) -> Result<Vec<u8>, String> {
    let b = spine_core::covers::curl(url, true, &["Accept: application/vnd.github+json".to_string()])?;
    if let Some(p) = out {
        fs::write(p, &b).map_err(|e| e.to_string())?;
    }
    Ok(b)
}

pub fn latest(repo: &str) -> Result<Option<Release>, String> {
    let body = curl(&format!("https://api.github.com/repos/{repo}/releases/latest"), None).map_err(|e| {
        if e.contains("404") { "No release has been published yet.".to_string() } else { format!("Couldn't reach GitHub ({e}).") }
    })?;
    Ok(parse_release(&body))
}

pub fn check(app_dir: &Path) -> Result<UpdateInfo, String> {
    let repo = repo(app_dir).ok_or("Updates aren't set up for this copy of Spine.")?;
    let rel = latest(&repo)?;
    Ok(UpdateInfo {
        current: CURRENT.into(),
        available: rel.as_ref().map_or(false, |r| newer(&r.version, CURRENT)),
        latest: rel.as_ref().map(|r| r.version.clone()),
        notes: rel.map(|r| r.notes).unwrap_or_default(),
    })
}

/// The hex fingerprint written in a .sha256 file ("<hex>  Spine.exe" or just "<hex>").
pub fn parse_sha_file(text: &str) -> Option<String> {
    let h = text.split_whitespace().next()?.to_lowercase();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Linux: download, check and swap the AppImage file. Linux lets a running file be
/// replaced, so this is a single rename. Returns the AppImage to restart into.
#[cfg(target_os = "linux")]
pub fn install(app_dir: &Path, _exe: &Path) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let Some(target) = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).filter(|p| p.is_file()) else {
        return Err("This copy of Spine was installed as a package. Get the new version from the Releases page on GitHub.".into());
    };
    let repo = repo(app_dir).ok_or("Updates aren't set up for this copy of Spine.")?;
    let rel = latest(&repo)?.ok_or("No release found.")?;
    if !newer(&rel.version, CURRENT) {
        return Err("You already have the newest Spine.".into());
    }
    let new = target.with_file_name(".Spine.new.AppImage");
    let _ = fs::remove_file(&new);
    curl(&rel.exe_url, Some(&new)).map_err(|e| format!("Download failed ({e})."))?;
    let bytes = fs::read(&new).map_err(|e| e.to_string())?;
    let want = parse_sha_file(&String::from_utf8_lossy(&curl(&rel.sha_url, None)?)).ok_or("The release's fingerprint file is missing or damaged.")?;
    if !bytes.starts_with(b"\x7fELF") || (rel.exe_size > 0 && bytes.len() as u64 != rel.exe_size) || sha256_hex(&bytes) != want {
        let _ = fs::remove_file(&new);
        return Err("The download didn't match its fingerprint, so Spine didn't install it. Try again later.".into());
    }
    fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    fs::rename(&new, &target).map_err(|e| {
        let _ = fs::remove_file(&new);
        format!("Couldn't put the new Spine in place ({e}). Is the folder it's in read-only?")
    })?;
    Ok(target)
}

/// Download, check and put the new Spine.exe in place. Returns the new exe's path
/// (the caller restarts into it).
#[cfg(not(target_os = "linux"))]
pub fn install(app_dir: &Path, exe: &Path) -> Result<std::path::PathBuf, String> {
    if exe.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("Spine.old.exe")) {
        return Err("This window is the old Spine from before the last update. Close it and open Spine again: it's already up to date.".into());
    }
    let repo = repo(app_dir).ok_or("Updates aren't set up for this copy of Spine.")?;
    let rel = latest(&repo)?.ok_or("No release found.")?;
    if !newer(&rel.version, CURRENT) {
        return Err("You already have the newest Spine.".into());
    }
    let new = app_dir.join("Spine.new.exe");
    let _ = fs::remove_file(&new);
    curl(&rel.exe_url, Some(&new)).map_err(|e| format!("Download failed ({e})."))?;
    let bytes = fs::read(&new).map_err(|e| e.to_string())?;
    let want = parse_sha_file(&String::from_utf8_lossy(&curl(&rel.sha_url, None)?)).ok_or("The release's fingerprint file is missing or damaged.")?;
    if !bytes.starts_with(b"MZ") || (rel.exe_size > 0 && bytes.len() as u64 != rel.exe_size) || sha256_hex(&bytes) != want {
        let _ = fs::remove_file(&new);
        return Err("The download didn't match its fingerprint, so Spine didn't install it. Try again later.".into());
    }
    let old = exe.with_file_name("Spine.old.exe");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old).map_err(|e| format!("Couldn't move the current Spine aside ({e})."))?;
    if let Err(e) = fs::rename(&new, exe) {
        let _ = fs::rename(&old, exe); // put things back
        return Err(format!("Couldn't put the new Spine in place ({e})."));
    }
    Ok(exe.to_path_buf())
}

/// After an update: make sure the previous version has closed, then tidy it away.
/// (Versions before 0.3.2 could leave their window open after updating.)
pub fn cleanup(exe: &Path) {
    #[cfg(target_os = "linux")]
    if let Some(a) = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from) {
        let _ = fs::remove_file(a.with_file_name(".Spine.new.AppImage"));
    }
    let old = exe.with_file_name("Spine.old.exe");
    let _ = fs::remove_file(exe.with_file_name("Spine.new.exe"));
    if !old.exists() || exe.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("Spine.old.exe")) {
        return;
    }
    for i in 0..40 {
        if fs::remove_file(&old).is_ok() || !old.exists() {
            return;
        }
        if i == 10 {
            // Still running after a few seconds: it's a leftover window, so close it.
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("taskkill").args(["/IM", "Spine.old.exe", "/F"]).creation_flags(0x0800_0000).output();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_releases() {
        assert!(newer("v0.3.0", "0.2.9"));
        assert!(newer("1.0.0", "0.9.12"));
        assert!(!newer("v0.2.0", "0.2.0"));
        assert!(!newer("0.1.9", "0.2.0"));
        let j = format!(r#"{{"tag_name":"v0.3.0","body":"New stuff","draft":false,"prerelease":false,"assets":[
            {{"name":"Spine.exe","size":123,"browser_download_url":"https://github.com/a/spine/releases/download/v0.3.0/Spine.exe"}},
            {{"name":"Spine.exe.sha256","size":80,"browser_download_url":"https://github.com/a/spine/releases/download/v0.3.0/Spine.exe.sha256"}},
            {{"name":"Spine-x86_64.AppImage","size":123,"browser_download_url":"https://github.com/a/spine/releases/download/v0.3.0/Spine-x86_64.AppImage"}},
            {{"name":"Spine-x86_64.AppImage.sha256","size":80,"browser_download_url":"https://github.com/a/spine/releases/download/v0.3.0/Spine-x86_64.AppImage.sha256"}}]}}"#);
        let j = j.as_bytes();
        let r = parse_release(j).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.exe_size, 123);
        assert!(r.exe_url.ends_with(ASSET));
        assert!(parse_release(br#"{"tag_name":"v1","assets":[]}"#).is_none());
        assert!(valid_repo("someone/spine"));
        assert!(!valid_repo("someone/spine/x"));
        assert!(!valid_repo("bra nd/spine"));
        let h = sha256_hex(b"abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(parse_sha_file(&format!("{}  Spine.exe\r\n", h.to_uppercase())), Some(h));
        assert_eq!(parse_sha_file("nope"), None);
        assert_eq!(clean_notes("## What's new\n- **Banners** on game pages.\n- Faster.\n\n---\n\n**New to Spine?** Download it."), "Banners on game pages.\nFaster.");
    }
}
