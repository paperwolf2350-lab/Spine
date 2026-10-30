//! Runs the scanner on a copy of a real library's folder layout
//! (file names only, every file empty) and checks the tricky cases.

use std::path::Path;

fn library() -> spine_core::Library {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/Roms");
    spine_core::scan(&root).expect("fixture should scan")
}

fn game<'a>(lib: &'a spine_core::Library, sys: &str, title: &str) -> &'a spine_core::Game {
    lib.systems
        .iter()
        .find(|s| s.id == sys)
        .and_then(|s| s.games.iter().find(|g| g.title == title))
        .unwrap_or_else(|| panic!("{sys}: \"{title}\" not found"))
}

#[test]
fn only_consoles_with_games_show_up() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let lib = library();
    let ids: Vec<_> = lib.systems.iter().map(|s| s.id).collect();
    assert_eq!(ids, ["n64", "gbc", "gba", "n3ds", "gc", "wiiu", "switch", "genesis", "ps2", "psp", "psvita", "pc"]);
    assert!(lib.systems.iter().all(|s| !s.games.is_empty()));
}

#[test]
fn rankings_are_stripped_and_sorted() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let lib = library();
    let n64 = lib.systems.iter().find(|s| s.id == "n64").unwrap();
    assert!(n64.games.iter().all(|g| !g.title.starts_with(char::is_numeric) || g.title.starts_with("1080") || g.title.starts_with("007")));
    game(&lib, "n64", "The Legend of Zelda: Ocarina of Time");
    let keys: Vec<_> = n64.games.iter().map(|g| g.sort_key.clone()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn wii_u_duplicates_merge_and_prefer_wua() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let lib = library();
    for t in ["Mario Kart 8", "ZombiU", "The Legend of Zelda: The Wind Waker HD", "Need for Speed: Most Wanted U - A Criterion Game"] {
        let g = game(&lib, "wiiu", t);
        assert_eq!(g.format, "wua", "{t}");
        assert_eq!(g.versions.len(), 2, "{t}");
    }
    let botw = game(&lib, "wiiu", "The Legend of Zelda Breath of the Wild");
    assert_eq!(botw.format, "wua");
    assert_eq!((botw.updates, botw.dlc), (1, 1));
    // Similar names that are different games stay apart.
    game(&lib, "wiiu", "Bayonetta");
    game(&lib, "wiiu", "Bayonetta 2");
    game(&lib, "wiiu", "New Super Mario Bros. U");
    game(&lib, "wiiu", "New Super Mario Bros. U + New Super Luigi U");
    game(&lib, "gbc", "Bubble Bobble");
    game(&lib, "gbc", "Classic Bubble Bobble");
}

#[test]
fn discs_updates_and_installers() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let lib = library();
    assert_eq!(game(&lib, "ps2", "Tom and Jerry in War of the Whiskers").format, "cue");
    assert!(!game(&lib, "ps2", "Ico").warnings.is_empty());
    assert!(game(&lib, "psp", "Ratchet & Clank: Size Matters").path.to_string_lossy().ends_with("Size Matters (USA).iso"));
    let za = game(&lib, "switch", "Pokémon Legends Z-A");
    assert_eq!(za.updates, 1);
    assert_eq!(lib.systems.iter().find(|s| s.id == "switch").unwrap().games.len(), 12);
    game(&lib, "psvita", "Gravity Rush");
    assert!(lib.notes.iter().any(|n| n.contains(".rar")));
}


fn fixtures_present() -> bool {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/Roms").is_dir()
}
