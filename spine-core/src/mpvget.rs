//! Get mpv on Windows (0.24): Spine downloads the latest plain x86_64 build from
//! shinchiro/mpv-winbuild-cmake's GitHub releases (the build the owner picked by hand),
//! checks it against the sha256 GitHub shows for the file, unpacks the .7z and puts it in
//! `<data folder>/tools/mpv/`, where the emulator search finds `mpv.exe`.
//! The .7z uses LZMA2 + BCJ2: `sevenz-rust2` handles that and checks every file's CRC.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const REPO: &str = "shinchiro/mpv-winbuild-cmake";
/// Written next to mpv.exe: which release Spine installed.
pub const STAMP: &str = "spine-mpv.json";

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub tag: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Lowercase hex, from GitHub's "digest": "sha256:<hex>".
    pub sha256: String,
}

/// The plain 64-bit build: "mpv-x86_64-20261021-git-e471496c56.7z".
/// Not "mpv-dev-..." (libraries), "-v3-" (newer CPUs only), aarch64 or i686.
pub fn is_plain_build(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("mpv-x86_64-").and_then(|r| r.strip_suffix(".7z")) else { return false };
    let mut parts = rest.split('-');
    let (Some(date), Some("git"), Some(hash), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
    date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) && !hash.is_empty() && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The plain build from a GitHub "latest release" answer. None if it isn't there or
/// GitHub gives no sha256 for it (Spine won't install a file it can't check).
pub fn parse_release(body: &[u8]) -> Option<Asset> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    let tag = v["tag_name"].as_str()?.to_string();
    v["assets"].as_array()?.iter().find_map(|a| {
        let name = a["name"].as_str()?;
        if !is_plain_build(name) {
            return None;
        }
        let hex = a["digest"].as_str()?.strip_prefix("sha256:")?.to_ascii_lowercase();
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(Asset { tag: tag.clone(), name: name.to_string(), url: a["browser_download_url"].as_str()?.to_string(), size: a["size"].as_u64().unwrap_or(0), sha256: hex })
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Where Spine keeps the mpv it downloads.
pub fn folder(data_dir: &Path) -> PathBuf {
    data_dir.join("tools").join("mpv")
}

/// The release tag of the mpv Spine installed (None if Spine didn't install one).
pub fn installed_tag(data_dir: &Path) -> Option<String> {
    let t = fs::read_to_string(folder(data_dir).join(STAMP)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    v["tag"].as_str().map(String::from)
}

/// Unpack a checked archive into `tools/mpv`: first into `tools/mpv.new`, then swap,
/// so a failed unpack never leaves a half mpv behind. Returns the path of mpv.exe.
pub fn unpack(data_dir: &Path, archive: &Path, tag: &str) -> Result<PathBuf, String> {
    let tools = data_dir.join("tools");
    let new = tools.join("mpv.new");
    let _ = fs::remove_dir_all(&new);
    fs::create_dir_all(&new).map_err(|e| format!("Couldn't make a folder for mpv ({e})."))?;
    if let Err(e) = sevenz_rust2::decompress_file(archive, &new) {
        let _ = fs::remove_dir_all(&new);
        return Err(format!("Couldn't unpack mpv ({e})."));
    }
    if !new.join("mpv.exe").is_file() {
        let _ = fs::remove_dir_all(&new);
        return Err("The download didn't contain mpv.exe.".into());
    }
    let _ = fs::write(new.join(STAMP), serde_json::json!({ "tag": tag }).to_string());
    let dest = folder(data_dir);
    if dest.exists() {
        // mpv.exe can be locked while a video plays: move it aside instead of failing.
        let old = tools.join("mpv.old");
        let _ = fs::remove_dir_all(&old);
        if fs::rename(&dest, &old).is_err() {
            let _ = fs::remove_dir_all(&new);
            return Err("mpv is still open. Close the video and try again.".into());
        }
        let _ = fs::remove_dir_all(&old);
    }
    fs::rename(&new, &dest).map_err(|e| format!("Couldn't put mpv in place ({e})."))?;
    Ok(dest.join("mpv.exe"))
}

/// The whole job: ask GitHub, download, check, unpack. `step` gets short status lines
/// for the interface ("Downloading mpv (33 MB)…").
pub fn install(data_dir: &Path, step: &dyn Fn(&str)) -> Result<(PathBuf, String), String> {
    step("Asking GitHub for the latest mpv…");
    let gh = ["Accept: application/vnd.github+json".to_string()];
    let body = crate::covers::curl(&format!("https://api.github.com/repos/{REPO}/releases/latest"), true, &gh).map_err(|e| format!("Couldn't reach GitHub ({e})."))?;
    let a = parse_release(&body).ok_or("GitHub didn't list a 64-bit mpv build with a fingerprint. Try again later.")?;
    let tools = data_dir.join("tools");
    fs::create_dir_all(&tools).map_err(|e| format!("Couldn't make the tools folder ({e})."))?;
    step(&format!("Downloading mpv ({} MB)…", (a.size + 500_000) / 1_000_000));
    let archive = tools.join("mpv-download.7z");
    let _ = fs::remove_file(&archive);
    crate::covers::curl_to_file(&a.url, &archive, 1800).map_err(|e| format!("Download failed ({e})."))?;
    step("Checking the download…");
    let bytes = fs::read(&archive).map_err(|e| e.to_string())?;
    let ok = (a.size == 0 || bytes.len() as u64 == a.size) && sha256_hex(&bytes) == a.sha256;
    drop(bytes);
    if !ok {
        let _ = fs::remove_file(&archive);
        return Err("The download didn't match GitHub's fingerprint, so Spine didn't use it. Try again.".into());
    }
    step("Unpacking mpv…");
    let r = unpack(data_dir, &archive, &a.tag);
    let _ = fs::remove_file(&archive);
    r.map(|p| (p, a.tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_plain_build() {
        assert!(is_plain_build("mpv-x86_64-20261021-git-e471496c56.7z"));
        for n in ["mpv-x86_64-v3-20261021-git-e471496c56.7z", "mpv-dev-x86_64-20261021-git-e471496c56.7z", "mpv-aarch64-20261021-git-e471496c56.7z", "mpv-i686-20261021-git-e471496c56.7z", "ffmpeg-x86_64-git-9a861e968.7z", "mpv-x86_64-20261021-git-e471496c56.zip"] {
            assert!(!is_plain_build(n), "{n}");
        }
    }

    #[test]
    fn reads_the_release() {
        let hex = "4a355f3e89c83fabc730d33fb4e759dfec747806ed2287b61a90d1681b83e8d3";
        let body = format!(
            r#"{{"tag_name":"20261021","assets":[
            {{"name":"mpv-x86_64-v3-20261021-git-e471496c56.7z","size":2,"digest":"sha256:{hex}","browser_download_url":"https://x/v3.7z"}},
            {{"name":"mpv-dev-x86_64-20261021-git-e471496c56.7z","size":2,"digest":"sha256:{hex}","browser_download_url":"https://x/dev.7z"}},
            {{"name":"mpv-x86_64-20261021-git-e471496c56.7z","size":34025957,"digest":"sha256:{}","browser_download_url":"https://x/plain.7z"}}]}}"#,
            hex.to_uppercase()
        );
        let a = parse_release(body.as_bytes()).unwrap();
        assert_eq!(a.url, "https://x/plain.7z");
        assert_eq!(a.size, 34025957);
        assert_eq!(a.sha256, hex);
        assert_eq!(a.tag, "20261021");
        // No fingerprint: not used.
        assert!(parse_release(br#"{"tag_name":"1","assets":[{"name":"mpv-x86_64-20261021-git-e4.7z","size":1,"browser_download_url":"https://x"}]}"#).is_none());
    }

    #[test]
    fn hashes() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn bad_archive_leaves_nothing_behind() {
        let d = std::env::temp_dir().join(format!("spine-mpvget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("tools")).unwrap();
        let bad = d.join("tools").join("bad.7z");
        fs::write(&bad, b"not a 7z").unwrap();
        assert!(unpack(&d, &bad, "x").is_err());
        assert!(!d.join("tools").join("mpv.new").exists());
        assert!(!folder(&d).exists());
        let _ = fs::remove_dir_all(&d);
    }

    /// Real archive, if one is lying around: SPINE_MPV_7Z=/path/to/mpv-x86_64-....7z
    #[test]
    #[ignore]
    fn unpacks_a_real_build() {
        let Ok(p) = std::env::var("SPINE_MPV_7Z") else { return };
        let d = std::env::temp_dir().join(format!("spine-mpvreal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let exe = unpack(&d, Path::new(&p), "t").unwrap();
        assert!(fs::read(&exe).unwrap().starts_with(b"MZ"));
        assert_eq!(installed_tag(&d).as_deref(), Some("t"));
        // Again over the top (an update).
        unpack(&d, Path::new(&p), "t2").unwrap();
        assert_eq!(installed_tag(&d).as_deref(), Some("t2"));
        let _ = fs::remove_dir_all(&d);
    }
}
