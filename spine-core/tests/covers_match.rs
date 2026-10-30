//! Matches every game in the fixture library against real libretro box-art
//! lists (saved in fixtures/art_lists) and reports what was found.

use spine_core::covers::{libretro_repos, ArtIndex};
use std::path::Path;

#[test]
fn match_rate_on_real_library() {
    if !fixtures_present() {
        return; // the sample library is only on the developer\'s machine
    }
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let lib = spine_core::scan(&base.join("fixtures/Roms")).unwrap();
    let (mut found, mut total) = (0, 0);
    let mut misses = Vec::new();
    for s in &lib.systems {
        let repos = libretro_repos(s.id);
        let lists: Vec<_> = repos.iter().filter_map(|r| {
            std::fs::read_to_string(base.join(format!("fixtures/art_lists/{r}.txt"))).ok()
                .map(|t| (*r, t.lines().map(String::from).collect::<Vec<_>>()))
        }).collect();
        if lists.is_empty() { continue; }
        let idx = ArtIndex::new(lists);
        let mut sys_found = 0;
        for g in &s.games {
            total += 1;
            match idx.find(g) {
                Some(e) => { found += 1; sys_found += 1; if std::env::var("SHOW").is_ok() { println!("  {:<50} -> {}", g.title, e.name); } }
                None => misses.push(format!("{} ({})", g.title, s.short)),
            }
        }
        println!("{:<26} {:>3}/{:<3}", s.name, sys_found, s.games.len());
    }
    println!("TOTAL {found}/{total}\nMISSES:\n  {}", misses.join("\n  "));
    assert!(found * 100 / total >= 90, "match rate too low: {found}/{total}");
}

fn fixtures_present() -> bool {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/Roms").is_dir()
}
