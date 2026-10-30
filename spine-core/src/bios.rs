//! BIOS and system files some consoles need before any game will start.
//!
//! Spine only reports what it can check with confidence: it looks in the places
//! each emulator reads by default (portable folder first, then the user folder),
//! and if none of the files it needs are there, it says exactly which file goes
//! where. It never blocks a launch on this, because people sometimes point an
//! emulator somewhere unusual.

use crate::launch::{cfg_path, LaunchPlan, EMULATORS, FLATPAKS};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BiosIssue {
    /// One sentence for the user.
    pub message: String,
    /// The folder the file should go in.
    pub folder: String,
}

/// The Flatpak app a plan runs, if it's "flatpak run <id> ...".
fn plan_flatpak(plan: &LaunchPlan) -> Option<&str> {
    (plan.program.file_name()?.to_str()? == "flatpak" && plan.args.first().map(String::as_str) == Some("run")).then(|| plan.args.get(1).map(String::as_str))?
}

/// Which emulator a plan uses, as an id ("pcsx2") or "retroarch:<core>".
pub fn plan_key(plan: &LaunchPlan) -> Option<String> {
    let exe = plan.program.file_name()?.to_string_lossy().to_lowercase();
    let id = if let Some(fp) = plan_flatpak(plan) {
        FLATPAKS.iter().find(|(_, f)| *f == fp)?.0
    } else {
        EMULATORS.iter().find(|e| e.exes.contains(&exe.as_str()) || (exe.ends_with(".appimage") && exe.starts_with(e.id)))?.id
    };
    let emu = EMULATORS.iter().find(|e| e.id == id)?;
    if emu.id == "retroarch" {
        let i = plan.args.iter().position(|a| a == "-L")?;
        let core = Path::new(plan.args.get(i + 1)?).file_stem()?.to_string_lossy().to_lowercase();
        return Some(format!("retroarch:{}", core.trim_end_matches("_libretro")));
    }
    Some(emu.id.to_string())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}
/// Linux folders under the home folder; a Flatpak keeps its own under ~/.var/app/<id>.
fn linux_dir(flatpak: Option<&str>, native: &str, in_flatpak: &str) -> Option<PathBuf> {
    let h = home()?;
    Some(match flatpak {
        Some(id) => h.join(".var/app").join(id).join(in_flatpak),
        None => h.join(native),
    })
}
fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}
fn documents() -> Option<PathBuf> {
    home().map(|h| h.join("Documents"))
}

/// RetroArch's "system" folder: from retroarch.cfg if set, else its default. A
/// portable RetroArch keeps its config next to the program; on Linux an installed
/// one uses ~/.config/retroarch, and the Flatpak its own copy of that.
pub fn retroarch_system_dir(exe: &Path) -> PathBuf {
    retroarch_system_dir_for(exe, None)
}

fn retroarch_system_dir_for(exe: &Path, flatpak: Option<&str>) -> PathBuf {
    let exe_dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    let config = if flatpak.is_some() || (cfg!(target_os = "linux") && !exe_dir.join("retroarch.cfg").exists() && !exe_dir.join("system").is_dir()) {
        linux_dir(flatpak, ".config/retroarch", "config/retroarch").unwrap_or(exe_dir.clone())
    } else {
        exe_dir.clone()
    };
    cfg_path(&config.join("retroarch.cfg"), "system_directory", &exe_dir).unwrap_or_else(|| config.join("system"))
}

fn has_any(dir: &Path, names: &[&str]) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    rd.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        names.iter().any(|w| n == *w)
    })
}

/// A folder holding at least one file of `min_bytes` or more (BIOS dumps have odd names).
fn has_big_file(dir: &Path, min_bytes: u64) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    rd.flatten().any(|e| e.metadata().map_or(false, |m| m.is_file() && m.len() >= min_bytes))
}

/// A folder setting from an emulator's .ini ("Bios = bios" or an absolute path),
/// relative to that emulator's data folder.
fn ini_folder(ini: &Path, section: &str, key: &str, data_dir: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(ini).ok()?;
    let mut in_section = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_section = l.eq_ignore_ascii_case(&format!("[{section}]"));
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        if k.trim().eq_ignore_ascii_case(key) {
            let v = v.trim().trim_matches('"');
            if v.is_empty() {
                return None;
            }
            let p = PathBuf::from(v);
            return Some(if p.is_absolute() { p } else { data_dir.join(p) });
        }
    }
    None
}

fn is_portable(exe_dir: &Path) -> bool {
    ["portable.txt", "portable.ini"].iter().any(|f| exe_dir.join(f).exists())
}

fn issue(message: String, folder: &Path) -> Option<BiosIssue> {
    Some(BiosIssue { message, folder: folder.display().to_string() })
}

/// Anything missing for this console with this emulator, or None if it looks ready
/// (or Spine can't tell).
pub fn check(system: &str, plan: &LaunchPlan) -> Option<BiosIssue> {
    if plan.blocked.is_some() {
        return None;
    }
    let key = plan_key(plan)?;
    let fp = plan_flatpak(plan);
    let linux = cfg!(target_os = "linux") || fp.is_some();
    let exe_dir = plan.program.parent()?.to_path_buf();
    let ra = |names: &[&str], what: &str, sub: Option<&str>| -> Option<BiosIssue> {
        let mut dir = retroarch_system_dir_for(&plan.program, fp);
        if let Some(s) = sub {
            dir = dir.join(s);
        }
        if has_any(&dir, names) {
            return None;
        }
        issue(format!("{what} needs a BIOS file. Put {} in this folder.", one_of(names)), &dir)
    };
    match (system, key.as_str()) {
        ("psx", "duckstation") => {
            let mut data = if fp.is_some() { vec![] } else { vec![exe_dir.clone()] };
            if linux && !is_portable(&exe_dir) {
                data.extend(linux_dir(fp, ".local/share/duckstation", "data/duckstation"));
            } else if !is_portable(&exe_dir) {
                data.extend(documents().map(|d| d.join("DuckStation")));
                data.extend(env_dir("LOCALAPPDATA").map(|d| d.join("DuckStation")));
            }
            let mut dirs: Vec<PathBuf> = data.iter().filter_map(|d| ini_folder(&d.join("settings.ini"), "BIOS", "SearchDirectory", d)).collect();
            dirs.extend(data.iter().map(|d| d.join("bios")));
            if dirs.iter().any(|d| has_big_file(d, 500_000)) {
                return None;
            }
            let shown = dirs.first().cloned().filter(|_| dirs.len() > data.len()).unwrap_or_else(|| data.last().unwrap_or(&exe_dir).join("bios"));
            issue("DuckStation needs a PlayStation BIOS (for example scph5501.bin) dumped from your console. Put it in this folder.".into(), &shown)
        }
        ("psx", k) if k.starts_with("retroarch:") => ra(&["scph5501.bin", "scph5500.bin", "scph5502.bin", "scph1001.bin", "scph7001.bin", "scph101.bin", "psxonpsp660.bin"], "PlayStation", None),
        ("ps2", "pcsx2") => {
            let mut data = if fp.is_some() { vec![] } else { vec![exe_dir.clone()] };
            if linux && !is_portable(&exe_dir) {
                data.extend(linux_dir(fp, ".config/PCSX2", "config/PCSX2"));
            } else if !is_portable(&exe_dir) {
                data.extend(documents().map(|d| d.join("PCSX2")));
            }
            let mut dirs: Vec<PathBuf> = data.iter().filter_map(|d| ini_folder(&d.join("inis").join("PCSX2.ini"), "Folders", "Bios", d)).collect();
            dirs.extend(data.iter().map(|d| d.join("bios")));
            if dirs.iter().any(|d| has_big_file(d, 1_000_000)) {
                return None;
            }
            let shown = dirs.first().cloned().filter(|_| dirs.len() > data.len()).unwrap_or_else(|| data.last().unwrap_or(&exe_dir).join("bios"));
            issue("PCSX2 needs a PlayStation 2 BIOS dumped from your console (a file around 4 MB, like SCPH-70012.bin). Put it in this folder.".into(), &shown)
        }
        ("ps2", "retroarch:pcsx2") => {
            let dir = retroarch_system_dir_for(&plan.program, fp).join("pcsx2").join("bios");
            if has_big_file(&dir, 1_000_000) {
                return None;
            }
            issue("The PCSX2 core needs a PlayStation 2 BIOS dumped from your console. Put it in this folder.".into(), &dir)
        }
        ("segacd", _) => ra(&["bios_cd_u.bin", "bios_cd_e.bin", "bios_cd_j.bin"], "Sega CD", None),
        ("saturn", "retroarch:mednafen_saturn") => ra(&["sega_101.bin", "mpr-17933.bin"], "Sega Saturn", None),
        ("pcenginecd", _) => ra(&["syscard3.pce"], "PC Engine CD", None),
        ("switch", "eden") => {
            let stem = plan.program.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_else(|| "eden".into());
            let name = ["citron", "sudachi"].into_iter().find(|n| stem.starts_with(n)).unwrap_or(if stem.starts_with("eden") { "eden" } else { stem.as_str() }).to_string();
            let mut dirs = vec![exe_dir.join("user").join("keys")];
            if linux {
                dirs.extend(home().map(|h| h.join(".local/share").join(&name).join("keys")));
            } else {
                dirs.extend(env_dir("APPDATA").map(|d| d.join(&name).join("keys")));
            }
            if dirs.iter().any(|d| d.join("prod.keys").is_file()) {
                return None;
            }
            let shown = if exe_dir.join("user").is_dir() { dirs[0].clone() } else { dirs.last().cloned().unwrap_or(dirs[0].clone()) };
            issue("Switch games need prod.keys (and firmware) from your own console. Put prod.keys in this folder.".into(), &shown)
        }
        ("switch", "ryujinx") => {
            let mut dirs = vec![exe_dir.join("portable").join("system")];
            if linux {
                dirs.extend(linux_dir(fp, ".config/Ryujinx/system", "config/Ryujinx/system"));
            } else {
                dirs.extend(env_dir("APPDATA").map(|d| d.join("Ryujinx").join("system")));
            }
            if dirs.iter().any(|d| d.join("prod.keys").is_file()) {
                return None;
            }
            let shown = if exe_dir.join("portable").is_dir() { dirs[0].clone() } else { dirs.last().cloned().unwrap_or(dirs[0].clone()) };
            issue("Switch games need prod.keys (and firmware) from your own console. Put prod.keys in this folder.".into(), &shown)
        }
        ("ps3", "rpcs3") => {
            let mut dirs = vec![exe_dir.join("dev_flash")];
            if linux {
                dirs.extend(linux_dir(fp, ".config/rpcs3/dev_flash", "config/rpcs3/dev_flash"));
            } else {
                dirs.extend(env_dir("APPDATA").map(|d| d.join("rpcs3").join("dev_flash")));
            }
            if dirs.iter().any(|d| d.join("vsh").join("module").is_dir()) {
                return None;
            }
            issue("RPCS3 needs the PlayStation 3 system software installed. Download PS3UPDAT.PUP from Sony, then in RPCS3 choose File → Install Firmware.".into(), dirs.last().unwrap_or(&exe_dir))
        }
        _ => None,
    }
}

fn one_of(names: &[&str]) -> String {
    match names {
        [one] => one.to_string(),
        [first, rest @ ..] if rest.len() == 1 => format!("{first} or {}", rest[0]),
        [first, ..] => format!("{first} (or another region's version)"),
        [] => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn flatpak_pcsx2_bios_folder() {
        let p = plan(Path::new("flatpak"), &["run", "net.pcsx2.PCSX2", "-batch", "--", "x.iso"]);
        assert_eq!(plan_key(&p).as_deref(), Some("pcsx2"));
        let home = std::env::var("HOME").unwrap();
        let bios = PathBuf::from(&home).join(".var/app/net.pcsx2.PCSX2/config/PCSX2/bios");
        if !bios.exists() {
            assert_eq!(check("ps2", &p).unwrap().folder, bios.display().to_string());
        }
        let ra = plan(Path::new("flatpak"), &["run", "org.libretro.RetroArch", "-f", "-L", "/x/genesis_plus_gx_libretro.so", "g.chd"]);
        assert_eq!(plan_key(&ra).as_deref(), Some("retroarch:genesis_plus_gx"));
        assert!(check("segacd", &ra).unwrap().folder.ends_with(".var/app/org.libretro.RetroArch/config/retroarch/system"));
    }

    fn plan(program: &Path, args: &[&str]) -> LaunchPlan {
        LaunchPlan { emulator: "x".into(), program: program.to_path_buf(), args: args.iter().map(|s| s.to_string()).collect(), blocked: None }
    }

    #[test]
    fn retroarch_folder_and_files() {
        let base = std::env::temp_dir().join(format!("spine-bios-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let ra = base.join("RetroArch");
        fs::create_dir_all(ra.join("cores")).unwrap();
        fs::write(ra.join("retroarch.exe"), b"").unwrap();
        let core = ra.join("cores").join("genesis_plus_gx_libretro.dll");
        let p = plan(&ra.join("retroarch.exe"), &["-f", "-L", core.to_str().unwrap(), "game.chd"]);
        assert_eq!(plan_key(&p).as_deref(), Some("retroarch:genesis_plus_gx"));
        let i = check("segacd", &p).unwrap();
        assert!(i.message.contains("bios_cd_u.bin"), "{}", i.message);
        assert!(i.folder.ends_with("system"));
        fs::create_dir_all(ra.join("system")).unwrap();
        fs::write(ra.join("system").join("BIOS_CD_U.BIN"), b"x").unwrap();
        assert_eq!(check("segacd", &p), None);
        // A system folder set in retroarch.cfg, relative to RetroArch.
        fs::write(ra.join("retroarch.cfg"), "system_directory = \":\\bios\"\n").unwrap();
        assert_eq!(retroarch_system_dir(&ra.join("retroarch.exe")), ra.join("bios"));
        assert!(check("segacd", &p).is_some());
        // Consoles that need nothing.
        assert_eq!(check("genesis", &p), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pcsx2_portable() {
        let base = std::env::temp_dir().join(format!("spine-bios2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("bios")).unwrap();
        fs::write(base.join("pcsx2-qt.exe"), b"").unwrap();
        fs::write(base.join("portable.ini"), b"").unwrap();
        let p = plan(&base.join("pcsx2-qt.exe"), &["-batch", "--", "x.iso"]);
        let i = check("ps2", &p).unwrap();
        assert_eq!(i.folder, base.join("bios").display().to_string());
        // A BIOS folder set in PCSX2's own settings wins.
        fs::create_dir_all(base.join("inis")).unwrap();
        fs::write(base.join("inis").join("PCSX2.ini"), "[UI]\nx = 1\n[Folders]\nBios = ../ps2bios\n").unwrap();
        assert_eq!(check("ps2", &p).unwrap().folder, base.join("../ps2bios").display().to_string());
        fs::write(base.join("bios").join("SCPH-70012.bin"), vec![0u8; 4 * 1024 * 1024]).unwrap();
        assert_eq!(check("ps2", &p), None);
        let _ = fs::remove_dir_all(&base);
    }
}
