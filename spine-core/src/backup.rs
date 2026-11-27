//! Backup & Restore for Spine's personal data (`spine-state.json`, `themes/`,
//! `media/custom/`, and optionally `credentials.txt` + `addons.json`).
//!
//! Re-downloadable scraper caches (`media/covers/`, `media/heroes/`, etc.) are
//! intentionally excluded so backups stay compact, fast, and portable.

use crate::themes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RESTORE_FILES: usize = 5_000;
const MAX_RESTORE_BYTES: u64 = 500 * 1024 * 1024; // 500 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u32,
    pub created_at: u64,
    pub include_secrets: bool,
    pub themes: usize,
    pub custom_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReport {
    pub themes: usize,
    pub custom_files: usize,
    pub included_secrets: bool,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub themes: usize,
    pub custom_files: usize,
    pub restored_secrets: bool,
}

fn is_safe_custom_media_ext(ext: &str) -> bool {
    themes::is_safe_asset_ext(ext)
        || matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "m4v" | "webm")
}

fn add_dir_filtered(
    root: &Path,
    dir: &Path,
    zip_prefix: &str,
    zw: &mut zip::ZipWriter<fs::File>,
    opts: zip::write::SimpleFileOptions,
    ext_ok: &dyn Fn(&str) -> bool,
) -> Result<usize, String> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for ent in entries {
        let p = ent.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            count += add_dir_filtered(root, &p, zip_prefix, zw, opts, ext_ok)?;
        } else if p.is_file() {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !ext_ok(ext) {
                continue;
            }
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?;
            let zip_name = format!("{zip_prefix}/{}", rel.to_string_lossy().replace('\\', "/"));
            zw.start_file(&zip_name, opts).map_err(|e| e.to_string())?;
            let mut f = fs::File::open(&p).map_err(|e| e.to_string())?;
            io::copy(&mut f, zw).map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

/// Create a `.zip` backup of Spine's personal data inside `data_dir`.
pub fn create_backup(
    data_dir: &Path,
    dest_zip: &Path,
    include_secrets: bool,
) -> Result<BackupReport, String> {
    let state_path = data_dir.join("spine-state.json");
    if !state_path.is_file() {
        return Err("No Spine settings or library data found to back up yet.".into());
    }

    let tmp_zip = dest_zip.with_extension("zip.tmp");
    let file = fs::File::create(&tmp_zip).map_err(|e| format!("Couldn't create backup file: {e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. spine-state.json
    zw.start_file("spine-state.json", opts).map_err(|e| e.to_string())?;
    zw.write_all(&fs::read(&state_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    // 2. Installed themes/
    let themes_dir = data_dir.join("themes");
    let theme_count = themes::scan_themes(&themes_dir).len();
    add_dir_filtered(&themes_dir, &themes_dir, "themes", &mut zw, opts, &themes::is_safe_asset_ext)?;

    // 3. media/custom/ (custom covers, custom videos, Simple Mode wallpapers/fonts/music)
    let custom_dir = data_dir.join("media").join("custom");
    let custom_files = add_dir_filtered(
        &custom_dir,
        &custom_dir,
        "media/custom",
        &mut zw,
        opts,
        &is_safe_custom_media_ext,
    )?;

    // 4. Optional private files (API keys + streaming addon links)
    let mut included_secrets = false;
    if include_secrets {
        for fname in ["credentials.txt", "addons.json"] {
            let p = data_dir.join(fname);
            if p.is_file() {
                zw.start_file(fname, opts).map_err(|e| e.to_string())?;
                zw.write_all(&fs::read(&p).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                included_secrets = true;
            }
        }
    }

    // 5. Manifest
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = BackupManifest {
        version: 1,
        created_at,
        include_secrets: included_secrets,
        themes: theme_count,
        custom_files,
    };
    zw.start_file("backup.json", opts).map_err(|e| e.to_string())?;
    zw.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    zw.finish().map_err(|e| format!("Couldn't finish backup zip: {e}"))?;
    let _ = fs::remove_file(dest_zip);
    fs::rename(&tmp_zip, dest_zip).map_err(|e| format!("Couldn't save backup file: {e}"))?;
    let bytes = fs::metadata(dest_zip).map(|m| m.len()).unwrap_or(0);

    Ok(BackupReport {
        themes: theme_count,
        custom_files,
        included_secrets,
        bytes,
    })
}

fn is_allowed_restore_path(rel: &Path) -> bool {
    let comps: Vec<&str> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();
    if comps.is_empty() || comps.iter().any(|s| s.starts_with('.') || s.contains(':')) {
        return false;
    }
    match comps[0] {
        "spine-state.json" | "backup.json" | "credentials.txt" | "addons.json" => comps.len() == 1,
        "themes" => {
            comps.len() >= 3
                && rel
                    .extension()
                    .and_then(|e| e.to_str())
                    .map_or(false, themes::is_safe_asset_ext)
        }
        "media" => {
            comps.len() >= 3
                && comps[1] == "custom"
                && rel
                    .extension()
                    .and_then(|e| e.to_str())
                    .map_or(false, is_safe_custom_media_ext)
        }
        _ => false,
    }
}

/// Restore a Spine `.zip` backup into `data_dir`.
/// Validates the archive before modifying anything and saves a safety copy
/// (`spine-state.before-restore.json`) of the current state first.
pub fn restore_backup(data_dir: &Path, zip_path: &Path) -> Result<RestoreReport, String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("Couldn't open backup file: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Not a valid .zip file: {e}"))?;

    if archive.len() == 0 {
        return Err("This backup file is empty.".into());
    }
    if archive.len() > MAX_RESTORE_FILES {
        return Err(format!("Backup contains too many files (max {MAX_RESTORE_FILES})."));
    }

    // Pass 1: validate every entry & verify spine-state.json is valid JSON before touching disk.
    let mut state_bytes: Option<Vec<u8>> = None;
    let mut total_size: u64 = 0;
    let mut valid_indices = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("Bad zip entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            return Err("Backup archive contains an unsafe file path.".into());
        };
        if !is_allowed_restore_path(&rel) {
            return Err(format!(
                "Backup contains an unexpected or disallowed file ({}).",
                rel.to_string_lossy()
            ));
        }
        total_size = total_size.saturating_add(entry.size());
        if total_size > MAX_RESTORE_BYTES {
            return Err("Backup archive is too large (max 500 MB extracted).".into());
        }
        if rel == Path::new("spine-state.json") {
            let mut buf = Vec::new();
            (&mut entry)
                .take(32 * 1024 * 1024)
                .read_to_end(&mut buf)
                .map_err(|e| format!("Couldn't read spine-state.json from backup: {e}"))?;
            let parsed: serde_json::Value = serde_json::from_slice(&buf)
                .map_err(|_| "spine-state.json inside this backup is damaged.")?;
            if !parsed.is_object() {
                return Err("spine-state.json inside this backup is not valid.".into());
            }
            state_bytes = Some(buf);
        } else if rel != Path::new("backup.json") {
            valid_indices.push((i, rel));
        }
    }

    let state_bytes = state_bytes.ok_or("This .zip is not a Spine backup (missing spine-state.json).")?;

    fs::create_dir_all(data_dir).map_err(|e| format!("Couldn't prepare Spine folder: {e}"))?;

    // Save a safety copy of the current state before replacing it.
    let current_state = data_dir.join("spine-state.json");
    if current_state.is_file() {
        let _ = fs::copy(&current_state, data_dir.join("spine-state.before-restore.json"));
    }

    // Write spine-state.json atomically.
    let tmp_state = data_dir.join("spine-state.json.tmp");
    fs::write(&tmp_state, &state_bytes).map_err(|e| format!("Couldn't write settings: {e}"))?;
    fs::rename(&tmp_state, &current_state).map_err(|e| format!("Couldn't replace settings: {e}"))?;

    // Extract themes, media/custom, and optional secrets.
    let mut restored_themes: BTreeSet<String> = BTreeSet::new();
    let mut custom_files = 0usize;
    let mut restored_secrets = false;
    let mut written: u64 = state_bytes.len() as u64;

    for (idx, rel) in valid_indices {
        let mut entry = archive.by_index(idx).map_err(|e| e.to_string())?;
        let dest = data_dir.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        let mut limited = (&mut entry).take(MAX_RESTORE_BYTES.saturating_sub(written) + 1);
        let n = io::copy(&mut limited, &mut out).map_err(|e| e.to_string())?;
        written = written.saturating_add(n);
        if written > MAX_RESTORE_BYTES {
            return Err("Backup archive exceeded size limit while restoring.".into());
        }

        let comps: Vec<&str> = rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect();
        match comps.first().copied() {
            Some("themes") if comps.len() >= 2 => {
                restored_themes.insert(comps[1].to_string());
            }
            Some("media") => {
                custom_files += 1;
            }
            Some("credentials.txt") | Some("addons.json") => {
                restored_secrets = true;
            }
            _ => {}
        }
    }

    Ok(RestoreReport {
        themes: restored_themes.len(),
        custom_files,
        restored_secrets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("spine-backup-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn backup_and_restore_roundtrip_respects_secrets_flag() {
        let src = tmp("src");
        let dst = tmp("dst");
        fs::write(src.join("spine-state.json"), r#"{"favorites":["snes/smw"],"settings":{"theme":"midnight"}}"#).unwrap();
        fs::write(src.join("credentials.txt"), "tmdb_key=SECRET123\n").unwrap();
        fs::write(src.join("addons.json"), r#"[{"url":"https://secret"}]"#).unwrap();
        fs::create_dir_all(src.join("media/custom/snes")).unwrap();
        fs::write(src.join("media/custom/snes/smw.png"), b"png").unwrap();
        fs::create_dir_all(src.join("media/covers/snes")).unwrap();
        fs::write(src.join("media/covers/snes/cached.jpg"), b"bigcache").unwrap();
        themes::create_starter_theme(&src.join("themes")).unwrap();

        // 1. Default backup (no secrets)
        let zip1 = src.join("backup-safe.zip");
        let rep = create_backup(&src, &zip1, false).unwrap();
        assert_eq!(rep.themes, 1);
        assert_eq!(rep.custom_files, 1);
        assert!(!rep.included_secrets);

        // Pre-existing state in dst gets saved as safety copy before restore
        fs::write(dst.join("spine-state.json"), r#"{"favorites":["old"]}"#).unwrap();
        let rrep = restore_backup(&dst, &zip1).unwrap();
        assert_eq!(rrep.themes, 1);
        assert_eq!(rrep.custom_files, 1);
        assert!(!rrep.restored_secrets);
        assert!(dst.join("spine-state.before-restore.json").is_file());
        assert!(dst.join("media/custom/snes/smw.png").is_file());
        assert!(!dst.join("media/covers/snes/cached.jpg").exists(), "cache should not be backed up");
        assert!(!dst.join("credentials.txt").exists(), "secrets must not be included by default");
        assert!(fs::read_to_string(dst.join("spine-state.json")).unwrap().contains("snes/smw"));

        // 2. Opt-in backup with secrets
        let zip2 = src.join("backup-full.zip");
        let rep2 = create_backup(&src, &zip2, true).unwrap();
        assert!(rep2.included_secrets);
        let rrep2 = restore_backup(&dst, &zip2).unwrap();
        assert!(rrep2.restored_secrets);
        assert!(dst.join("credentials.txt").is_file());
        assert!(dst.join("addons.json").is_file());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dst);
    }

    #[test]
    fn rejects_invalid_or_unsafe_backup_zips() {
        let dir = tmp("bad");
        // Missing spine-state.json
        let bad1 = dir.join("not-a-backup.zip");
        {
            let f = fs::File::create(&bad1).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            zw.start_file("backup.json", zip::write::SimpleFileOptions::default()).unwrap();
            zw.write_all(b"{}").unwrap();
            zw.finish().unwrap();
        }
        assert!(restore_backup(&dir, &bad1).is_err());

        // Disallowed file inside backup
        let bad2 = dir.join("evil-backup.zip");
        {
            let f = fs::File::create(&bad2).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let o = zip::write::SimpleFileOptions::default();
            zw.start_file("spine-state.json", o).unwrap();
            zw.write_all(b"{}").unwrap();
            zw.start_file("themes/x/evil.js", o).unwrap();
            zw.write_all(b"alert(1)").unwrap();
            zw.finish().unwrap();
        }
        assert!(restore_backup(&dir, &bad2).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
