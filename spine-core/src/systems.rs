//! The system registry: which consoles Spine knows about, which folder names
//! map to them, which file types count as games, and how to launch them.
//!
//! Folder names follow ES-DE / EmuDeck / RetroBat conventions first, with
//! loose aliases on top so that "PS2 isos" or "Nintendo 64" still work.

use serde::Serialize;

/// How a system's games are launched by default.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Launcher {
    /// RetroArch with the named libretro core.
    RetroArch(&'static str),
    /// A standalone emulator, by id in `launch::EMULATORS`.
    Standalone(&'static str),
    /// Opened by the operating system (Steam .url links, shortcuts).
    ShellOpen,
}

#[derive(Debug, Clone, Serialize)]
pub struct System {
    /// Stable id, e.g. "ps2".
    pub id: &'static str,
    /// Display name, e.g. "PlayStation 2".
    pub name: &'static str,
    /// Short badge label, e.g. "PS2".
    pub short: &'static str,
    /// Folder names that map to this system (compared after normalising).
    pub folders: &'static [&'static str],
    /// File extensions that count as a game, lower case, without the dot.
    /// Order matters: earlier formats win when the same game exists twice.
    pub exts: &'static [&'static str],
    /// Default launcher, then fallbacks.
    pub launchers: &'static [Launcher],
    /// Accent colour for the UI.
    pub color: &'static str,
}

use Launcher::*;

pub static SYSTEMS: &[System] = &[
    // ---- Nintendo
    System { id: "nes", name: "Nintendo Entertainment System", short: "NES", folders: &["nes", "famicom", "nintendo", "nintendoentertainmentsystem"], exts: &["nes", "unf", "unif", "zip", "7z"], launchers: &[RetroArch("mesen"), RetroArch("nestopia")], color: "#B23A2E" },
    System { id: "fds", name: "Famicom Disk System", short: "FDS", folders: &["fds", "famicomdisksystem"], exts: &["fds", "zip", "7z"], launchers: &[RetroArch("mesen")], color: "#9E3A2E" },
    System { id: "snes", name: "Super Nintendo", short: "SNES", folders: &["snes", "sfc", "snesna", "superfamicom", "supernintendo"], exts: &["sfc", "smc", "fig", "swc", "bs", "zip", "7z"], launchers: &[RetroArch("snes9x"), RetroArch("bsnes"), RetroArch("mesen-s")], color: "#5B4BC0" },
    System { id: "n64", name: "Nintendo 64", short: "N64", folders: &["n64", "nintendo64"], exts: &["z64", "n64", "v64", "zip", "7z"], launchers: &[RetroArch("mupen64plus_next"), RetroArch("parallel_n64"), Standalone("rmg"), Standalone("simple64")], color: "#237A52" },
    System { id: "gb", name: "Game Boy", short: "GB", folders: &["gb", "gameboy"], exts: &["gb", "zip", "7z"], launchers: &[RetroArch("gambatte"), RetroArch("mgba")], color: "#5E6B3A" },
    System { id: "gbc", name: "Game Boy Color", short: "GBC", folders: &["gbc", "gameboycolor"], exts: &["gbc", "gb", "zip", "7z"], launchers: &[RetroArch("gambatte"), RetroArch("mgba"), RetroArch("sameboy")], color: "#7A3F8F" },
    System { id: "gba", name: "Game Boy Advance", short: "GBA", folders: &["gba", "gameboyadvance"], exts: &["gba", "zip", "7z"], launchers: &[RetroArch("mgba"), RetroArch("vba_next"), RetroArch("gpsp"), Standalone("mgba")], color: "#2F5FBF" },
    System { id: "nds", name: "Nintendo DS", short: "DS", folders: &["nds", "ds", "nintendods"], exts: &["nds", "zip", "7z"], launchers: &[RetroArch("melondsds"), Standalone("melonds")], color: "#4A4F5C" },
    System { id: "n3ds", name: "Nintendo 3DS", short: "3DS", folders: &["n3ds", "3ds", "nintendo3ds"], exts: &["3ds", "cci", "cxi", "cia", "3dsx", "app"], launchers: &[Standalone("azahar"), RetroArch("citra")], color: "#B5252F" },
    System { id: "virtualboy", name: "Virtual Boy", short: "VB", folders: &["virtualboy", "vb"], exts: &["vb", "vboy", "zip", "7z"], launchers: &[RetroArch("mednafen_vb")], color: "#8E1F1F" },
    System { id: "gc", name: "GameCube", short: "GC", folders: &["gc", "gamecube", "ngc", "nintendogamecube"], exts: &["rvz", "iso", "gcm", "gcz", "ciso", "wbfs", "dol", "m3u"], launchers: &[Standalone("dolphin"), RetroArch("dolphin")], color: "#5B21B6" },
    System { id: "wii", name: "Wii", short: "Wii", folders: &["wii", "nintendowii"], exts: &["rvz", "wbfs", "iso", "wia", "gcz", "ciso", "wad", "dol"], launchers: &[Standalone("dolphin")], color: "#5D7A8C" },
    System { id: "wiiu", name: "Wii U", short: "Wii U", folders: &["wiiu", "nintendowiiu"], exts: &["wua", "wux", "wud", "rpx"], launchers: &[Standalone("cemu")], color: "#0E7490" },
    System { id: "switch", name: "Nintendo Switch", short: "Switch", folders: &["switch", "nintendoswitch", "nsw"], exts: &["xci", "nsp", "nca", "nro", "nso"], launchers: &[Standalone("eden"), Standalone("ryujinx")], color: "#C8231F" },
    // ---- Sega
    System { id: "mastersystem", name: "Master System", short: "SMS", folders: &["mastersystem", "sms", "segamastersystem", "mark3"], exts: &["sms", "bin", "zip", "7z"], launchers: &[RetroArch("genesis_plus_gx")], color: "#3B4A8C" },
    System { id: "genesis", name: "Sega Genesis", short: "Genesis", folders: &["genesis", "megadrive", "megadrivejp", "segagenesis", "md"], exts: &["md", "gen", "smd", "bin", "zip", "7z"], launchers: &[RetroArch("genesis_plus_gx"), RetroArch("picodrive"), RetroArch("blastem")], color: "#1F2937" },
    System { id: "gamegear", name: "Game Gear", short: "GG", folders: &["gamegear", "gg", "segagamegear"], exts: &["gg", "zip", "7z"], launchers: &[RetroArch("genesis_plus_gx")], color: "#2E3540" },
    System { id: "segacd", name: "Sega CD", short: "Sega CD", folders: &["segacd", "megacd", "megacdjp"], exts: &["chd", "cue", "iso", "m3u"], launchers: &[RetroArch("genesis_plus_gx")], color: "#33415C" },
    System { id: "sega32x", name: "Sega 32X", short: "32X", folders: &["sega32x", "32x", "sega32xjp", "sega32xna"], exts: &["32x", "bin", "zip", "7z"], launchers: &[RetroArch("picodrive")], color: "#3D3D52" },
    System { id: "saturn", name: "Sega Saturn", short: "Saturn", folders: &["saturn", "saturnjp", "segasaturn"], exts: &["chd", "cue", "m3u", "iso", "ccd", "mds"], launchers: &[RetroArch("mednafen_saturn"), RetroArch("yabasanshiro")], color: "#2E4370" },
    System { id: "dreamcast", name: "Dreamcast", short: "DC", folders: &["dreamcast", "dc", "segadreamcast"], exts: &["chd", "gdi", "cdi", "cue", "m3u"], launchers: &[RetroArch("flycast")], color: "#C25A1A" },
    // ---- Sony
    System { id: "psx", name: "PlayStation", short: "PS1", folders: &["psx", "ps1", "playstation", "playstation1", "sonyplaystation"], exts: &["chd", "m3u", "cue", "pbp", "ecm", "iso", "img", "ccd", "bin"], launchers: &[Standalone("duckstation"), RetroArch("swanstation"), RetroArch("mednafen_psx_hw")], color: "#5C6270" },
    System { id: "ps2", name: "PlayStation 2", short: "PS2", folders: &["ps2", "playstation2", "sonyplaystation2"], exts: &["chd", "iso", "cso", "zso", "gz", "m3u", "cue", "bin", "elf"], launchers: &[Standalone("pcsx2"), RetroArch("pcsx2")], color: "#1E3A8A" },
    System { id: "ps3", name: "PlayStation 3", short: "PS3", folders: &["ps3", "playstation3"], exts: &["ps3", "iso", "eboot"], launchers: &[Standalone("rpcs3")], color: "#22252B" },
    System { id: "psp", name: "PlayStation Portable", short: "PSP", folders: &["psp", "playstationportable"], exts: &["chd", "iso", "cso", "pbp", "elf"], launchers: &[Standalone("ppsspp"), RetroArch("ppsspp")], color: "#33363D" },
    System { id: "psvita", name: "PlayStation Vita", short: "Vita", folders: &["psvita", "vita", "playstationvita"], exts: &["psvita"], launchers: &[Standalone("vita3k")], color: "#1B4F8A" },
    // ---- Others
    System { id: "pcengine", name: "PC Engine / TurboGrafx-16", short: "PCE", folders: &["pcengine", "tg16", "turbografx16", "turbografx", "supergrafx"], exts: &["pce", "sgx", "zip", "7z"], launchers: &[RetroArch("mednafen_pce")], color: "#B0472B" },
    System { id: "pcenginecd", name: "PC Engine CD", short: "PCE-CD", folders: &["pcenginecd", "tgcd", "tg-cd", "turbografxcd"], exts: &["chd", "cue", "ccd", "m3u"], launchers: &[RetroArch("mednafen_pce")], color: "#94402A" },
    System { id: "neogeo", name: "Neo Geo", short: "NEO", folders: &["neogeo"], exts: &["zip", "7z"], launchers: &[RetroArch("fbneo")], color: "#8C6A12" },
    System { id: "ngpc", name: "Neo Geo Pocket", short: "NGP", folders: &["ngp", "ngpc", "neogeopocket", "neogeopocketcolor"], exts: &["ngp", "ngc", "zip", "7z"], launchers: &[RetroArch("mednafen_ngp")], color: "#6E5A1A" },
    System { id: "arcade", name: "Arcade", short: "Arcade", folders: &["arcade", "mame", "fbneo", "fba", "cps", "cps1", "cps2", "cps3", "mame-advmame"], exts: &["zip", "7z"], launchers: &[RetroArch("fbneo"), RetroArch("mame")], color: "#7A2E5A" },
    System { id: "atari2600", name: "Atari 2600", short: "2600", folders: &["atari2600", "2600"], exts: &["a26", "bin", "zip", "7z"], launchers: &[RetroArch("stella")], color: "#7A4B2E" },
    System { id: "atarilynx", name: "Atari Lynx", short: "Lynx", folders: &["atarilynx", "lynx"], exts: &["lnx", "zip", "7z"], launchers: &[RetroArch("handy")], color: "#6B4F2E" },
    System { id: "wonderswan", name: "WonderSwan", short: "WS", folders: &["wonderswan", "wonderswancolor", "wsc", "ws"], exts: &["ws", "wsc", "zip", "7z"], launchers: &[RetroArch("mednafen_wswan")], color: "#4A5A6B" },
    System { id: "dos", name: "MS-DOS", short: "DOS", folders: &["dos", "msdos", "pc"], exts: &["exe", "com", "bat", "conf", "zip"], launchers: &[RetroArch("dosbox_pure")], color: "#3A3A3A" },
    System { id: "xbox", name: "Xbox", short: "Xbox", folders: &["xbox"], exts: &["iso", "xiso"], launchers: &[Standalone("xemu")], color: "#2E6B2E" },
    // ---- PC
    System { id: "pc", name: "PC Games", short: "PC", folders: &["steam", "pcgames", "windows", "epic", "gog"], exts: &["url", "lnk"], launchers: &[ShellOpen], color: "#1B2838" },
];

/// Lower-case and keep only letters and digits: "PS2 isos" -> "ps2isos".
pub fn norm_folder(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// Find the system a folder belongs to.
///
/// Exact alias matches win. Failing that, a folder whose name *starts with*
/// an alias of 3+ characters matches ("ps2isos" -> ps2, "nesroms" -> nes).
/// Longer aliases are tried first so "ps2isos" never lands on "ps".
pub fn system_for_folder(name: &str) -> Option<&'static System> {
    let n = norm_folder(name);
    if n.is_empty() {
        return None;
    }
    if let Some(s) = SYSTEMS.iter().find(|s| s.folders.iter().any(|f| norm_folder(f) == n)) {
        return Some(s);
    }
    let mut best: Option<(&'static System, usize)> = None;
    for s in SYSTEMS {
        for f in s.folders {
            let f = norm_folder(f);
            if f.len() >= 3 && n.starts_with(&f) && best.map_or(true, |(_, l)| f.len() > l) {
                best = Some((s, f.len()));
            }
        }
    }
    best.map(|(s, _)| s)
}

pub fn system_by_id(id: &str) -> Option<&'static System> {
    SYSTEMS.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_matching() {
        assert_eq!(system_for_folder("ps2").unwrap().id, "ps2");
        assert_eq!(system_for_folder("PS2 isos").unwrap().id, "ps2");
        assert_eq!(system_for_folder("Nintendo 64").unwrap().id, "n64");
        assert_eq!(system_for_folder("psx").unwrap().id, "psx");
        assert_eq!(system_for_folder("nes roms").unwrap().id, "nes");
        assert_eq!(system_for_folder("snesna").unwrap().id, "snes");
        assert_eq!(system_for_folder("Wii U").unwrap().id, "wiiu");
        assert_eq!(system_for_folder("wii").unwrap().id, "wii");
        assert_eq!(system_for_folder("gamecube").unwrap().id, "gc");
        assert!(system_for_folder("random downloads").is_none());
        assert!(system_for_folder("emulators").is_none());
    }
}
