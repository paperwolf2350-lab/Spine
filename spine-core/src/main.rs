//! spine-scan — try the Spine scanner on a real ROM folder.
//!
//!   spine-scan D:\Roms                     summary of what was found
//!   spine-scan D:\Roms --list gc           every GameCube game
//!   spine-scan D:\Roms --out library.json  save the full library
//!   spine-scan D:\Roms --emulators D:\ES-DE\Emulators
//!   spine-scan D:\Roms --launch "gc/mario-kart-double-dash"
//!   spine-scan D:\Roms --covers             download missing box art

use spine_core::covers;
use spine_core::launch::{self, Installed};
use std::io::Write;
use spine_core::systems;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut list: Option<String> = None;
    let mut launch_id: Option<String> = None;
    let mut extra_search: Vec<PathBuf> = Vec::new();
    let mut want_covers = false;
    let mut media: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--list" => list = args.next(),
            "--launch" => launch_id = args.next(),
            "--emulators" => extra_search.extend(args.next().map(PathBuf::from)),
            "--covers" => want_covers = true,
            "--media" => media = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("usage: spine-scan <ROM folder> [--list <console>] [--out library.json] [--covers] [--media <folder>] [--emulators <folder>] [--launch <game id>]");
                return ExitCode::SUCCESS;
            }
            _ if root.is_none() => root = Some(PathBuf::from(a)),
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(root) = root else {
        eprintln!("usage: spine-scan <ROM folder> [--list <console>] [--out library.json] [--covers] [--media <folder>] [--emulators <folder>] [--launch <game id>]");
        return ExitCode::from(2);
    };

    let started = std::time::Instant::now();
    let mut lib = match spine_core::scan(&root) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Couldn't read {}: {e}", root.display());
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed();

    // Box art lives next to the program unless --media says otherwise.
    let media = media.unwrap_or_else(|| {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("media"))).unwrap_or_else(|| PathBuf::from("media"))
    });
    covers::attach_existing(&mut lib, &media);

    // Accounts the user typed into credentials.txt (next to the program).
    let cred_path = media.parent().map(|d| d.join("credentials.txt")).unwrap_or_else(|| PathBuf::from("credentials.txt"));
    spine_core::config::Credentials::ensure_template(&cred_path, "");
    let creds = spine_core::config::Credentials::load(&cred_path);

    let progress = |done: usize, total: usize, title: &str| {
        let t: String = title.chars().take(50).collect();
        print!("\r  {done:>4}/{total:<4} {t:<50}");
        let _ = std::io::stdout().flush();
    };
    let mut report_lines: Vec<String> = Vec::new();
    if want_covers {
        // 1. Free covers: libretro collection + Steam.
        println!("Fetching box art into {} ...", media.display());
        let r = covers::fetch_covers(&mut lib, &media, &progress);
        println!("\r{:<70}", "");
        report_lines.push(format!("Free cover collection + Steam: {} new, {} already saved.", r.downloaded, r.already));
        for f in &r.failed {
            report_lines.push(format!("  failed (will retry next time): {f}"));
        }

        // 2. ScreenScraper: names, descriptions, years, and covers for what's still missing.
        if creds.screenscraper_ready() {
            println!("Looking up game details on ScreenScraper (one game at a time; the first run takes a while)...");
            let d = spine_core::screenscraper::fetch_details(&lib, &media, &creds, &progress);
            println!("\r{:<70}", "");
            covers::attach_existing(&mut lib, &media);
            report_lines.push(format!("ScreenScraper: {} looked up, {} found, {} new covers.", d.looked_up, d.found, d.covers));
            if let Some(why) = d.stopped {
                report_lines.push(format!("  stopped early: {why}"));
            }
        } else if creds.ss_user.is_some() {
            let missing = if creds.ss_password.is_none() { "password and developer key" } else { "developer key" };
            report_lines.push(format!("ScreenScraper: skipped, credentials.txt is missing the {missing}."));
        }

        // 3. SteamGridDB for anything still without a cover.
        match &creds.steamgriddb_key {
            Some(key) => {
                println!("Fetching remaining covers from SteamGridDB...");
                let r = covers::fetch_sgdb_covers(&mut lib, &media, key, &progress);
                println!("\r{:<70}", "");
                report_lines.push(format!("SteamGridDB: {} new covers.", r.downloaded));
                if let Some(f) = r.failed.first().filter(|f| f.contains("API key")) {
                    report_lines.push(format!("  {f}"));
                } else {
                    for f in &r.failed {
                        report_lines.push(format!("  failed (will retry next time): {f}"));
                    }
                }
            }
            None => report_lines.push("SteamGridDB: skipped, no API key in credentials.txt yet.".into()),
        }
        covers::attach_existing(&mut lib, &media);
        let missing: Vec<String> = lib.systems.iter().flat_map(|s| s.games.iter().filter(|g| g.cover.is_none()).map(move |g| format!("{} ({})", g.title, s.short))).collect();
        if !missing.is_empty() {
            report_lines.push(format!("Still without a cover ({}):", missing.len()));
            report_lines.extend(missing.into_iter().map(|m| format!("    {m}")));
        }
    }

    let mut search = extra_search;
    search.extend(launch::default_search_paths(&root));
    let installed = Installed::detect(&search);

    if let Some(id) = launch_id {
        let Some((sys, game)) = lib.systems.iter().find_map(|s| s.games.iter().find(|g| g.id == id).map(|g| (s, g))) else {
            eprintln!("No game with id \"{id}\". Use --list <console> to see ids.");
            return ExitCode::FAILURE;
        };
        let def = systems::system_by_id(sys.id).unwrap();
        let plan = launch::plan(def, game, &installed);
        if let Some(why) = &plan.blocked {
            eprintln!("Can't launch {} with {} yet: {why}", game.title, plan.emulator);
            return ExitCode::FAILURE;
        }
        println!("Launching {} with {}", game.title, plan.emulator);
        println!("  {} {}", plan.program.display(), plan.args.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(" "));
        return match launch::launch(&plan) {
            Ok(mut child) => {
                let t = std::time::Instant::now();
                let _ = child.wait();
                println!("Played for {} min.", t.elapsed().as_secs() / 60);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Can't launch yet: {e}");
                ExitCode::FAILURE
            }
        };
    }

    println!("Scanned {} in {:.2}s: {} games on {} consoles\n", root.display(), elapsed.as_secs_f64(), lib.game_count, lib.systems.len());
    println!("{:<30} {:>6} {:>7}  {:<28} {}", "Console", "Games", "Covers", "Launches with", "Status");
    println!("{}", "-".repeat(98));
    for s in &lib.systems {
        let def = systems::system_by_id(s.id).unwrap();
        let plan = launch::plan(def, &s.games[0], &installed);
        let status = match &plan.blocked {
            None => "ready".to_string(),
            Some(why) => why.clone(),
        };
        let with_cover = s.games.iter().filter(|g| g.cover.is_some()).count();
        println!("{:<30} {:>6} {:>7}  {:<28} {}", s.name, s.games.len(), with_cover, plan.emulator, status);
    }

    let dup_games: Vec<_> = lib.systems.iter().flat_map(|s| s.games.iter()).filter(|g| g.versions.len() > 1).collect();
    if !dup_games.is_empty() {
        println!("\nGames found in more than one file (shown once, best copy launches):");
        for g in dup_games {
            let fmts: Vec<_> = g.versions.iter().map(|v| v.format.as_str()).collect();
            println!("  {}  [{}]", g.title, fmts.join(" + "));
        }
    }
    let extras: Vec<_> = lib.systems.iter().flat_map(|s| s.games.iter()).filter(|g| g.updates + g.dlc > 0).collect();
    if !extras.is_empty() {
        println!("\nUpdates and DLC attached to their games:");
        for g in extras {
            println!("  {}  ({} update, {} DLC)", g.title, g.updates, g.dlc);
        }
    }
    let warned: Vec<_> = lib.systems.iter().flat_map(|s| s.games.iter()).filter(|g| !g.warnings.is_empty()).collect();
    if !warned.is_empty() {
        println!("\nGames that may need attention:");
        for g in warned {
            println!("  {}: {}", g.title, g.warnings.join(" "));
        }
    }
    {
        let have = lib.systems.iter().flat_map(|s| s.games.iter()).filter(|g| g.cover.is_some()).count();
        let info = lib.systems.iter().flat_map(|s| s.games.iter()).filter(|g| g.info.is_some()).count();
        println!("\nBox art: {have} of {} games have a cover. Details (description, year, genre): {info} games.", lib.game_count);
        for l in &report_lines {
            println!("  {l}");
        }
    }
    if !lib.notes.is_empty() {
        println!("\nNotes:");
        for n in &lib.notes {
            println!("  {n}");
        }
    }

    if let Some(id) = list {
        match lib.systems.iter().find(|s| s.id == id || s.short.eq_ignore_ascii_case(&id)) {
            Some(s) => {
                println!("\n{} ({} games)", s.name, s.games.len());
                let mut letter = ' ';
                for g in &s.games {
                    if g.letter != letter {
                        letter = g.letter;
                        println!("  — {letter} —");
                    }
                    println!("    {:<58} {:<6} {}", g.title, g.format, g.id);
                }
            }
            None => eprintln!("\nNo console \"{id}\" in this library."),
        }
    }

    if let Some(path) = out {
        let json = serde_json::to_string_pretty(&lib).unwrap();
        if let Err(e) = std::fs::write(&path, json) {
            eprintln!("Couldn't write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("\nSaved library to {}", path.display());
    }
    ExitCode::SUCCESS
}
