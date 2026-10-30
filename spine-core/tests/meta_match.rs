//! Categories for the fixture library from real libretro metadat files.
use std::path::Path;

#[test]
fn categories_for_real_library() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let lib = spine_core::scan(&base.join("fixtures/Roms")).unwrap();
    let tmp = std::env::temp_dir().join("spine-meta-test");
    let _ = std::fs::remove_dir_all(&tmp);
    // Leave PC games out (they need SteamSpy over the network).
    let mut lib2 = lib.clone();
    lib2.systems.retain(|s| s.id != "pc");
    let all = spine_core::meta::build(&lib2, &tmp, Some(&base.join("fixtures/metadat")), &|_, _, _| {});
    if let Ok(p) = std::env::var("SPINE_DUMP") { std::fs::write(p, serde_json::to_string(&all).unwrap()).unwrap(); }
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for s in &lib2.systems {
        let with = s.games.iter().filter(|g| all.get(&g.id).map_or(false, |m| !m.genres.is_empty())).count();
        println!("{:<26} genre {:>3}/{:<3} franchise {:>3}", s.name, with, s.games.len(), s.games.iter().filter(|g| all.get(&g.id).and_then(|m| m.franchise.as_ref()).is_some()).count());
    }
    for m in all.values() { for g in &m.genres { *counts.entry(g.clone()).or_default() += 1; } }
    println!("genres: {:?}", counts);
    let gs = lib2.systems.iter().find(|s| s.id == "gba").unwrap().games.iter().find(|g| g.title == "Golden Sun: The Lost Age").unwrap();
    let m = &all[&gs.id];
    println!("Golden Sun TLA: {:?}", m);
    assert!(m.genres.contains(&"RPG".to_string()));
    assert!(all.len() > 300);
}

fn fixtures_present() -> bool {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/Roms").is_dir()
}
