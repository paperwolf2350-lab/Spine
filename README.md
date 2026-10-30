# Spine

A game library for your PC that feels like a console. Point it at your ROM
folder and it sorts every game by console and A–Z, finds box art, details and
trailers, and starts each game with the right emulator. Steam, Epic and GOG
games show up too. Everything works with a controller.

## Get it

Download **Spine-Setup.exe** from the latest release on this page and run it.
No admin rights needed; Spine gets a Start menu entry and a desktop shortcut.
It updates itself: when a new version is out, Settings → Updates offers it.

Prefer no installer? Download **Spine.exe** instead and put it in its own folder
(for example `Documents\Spine`). Spine keeps its settings, covers and favorites
next to the program, so that folder is all it needs.

Windows 10 or 11. Your emulators can live anywhere common (an `Emulators`
folder next to your ROMs, Program Files, AppData).

## What it does

- **Organizes everything:** consoles from folder names, clean titles, duplicates
  merged, updates and DLC attached, several ROM folders and drives, unplugged
  drives remembered.
- **Looks good:** covers from libretro, Steam, SteamGridDB and ScreenScraper,
  trailers on game pages, four layouts, themes, a moving cover wall per console.
- **Plays:** the right emulator per console, a different one per game if you
  like, playtime tracking, hold Select + Start to quit back to Spine.
- **Helps:** missing BIOS files named with where they go, links to official
  emulator downloads, fix a wrong name or cover in a few clicks.
- **Extras:** categories you can click ("every Roguelite you own"), your own
  lists, Surprise me, RetroAchievements on game pages.

Spine never ships BIOS files, keys or games. Use dumps of hardware and games
you own.

## Build it yourself

Install Rust (https://rustup.rs) and Visual Studio's C++ build tools, then:

```
cd spine-app
cargo build --release
```

The app is `spine-app` (Tauri 2, plain HTML/CSS/JS interface in `spine-app/ui`).
The library scanning, matching and launching live in `spine-core`, which also
builds a command-line tool, `spine-scan`.

## Releasing

Bump `version` in `spine-app/Cargo.toml` (and `spine-core/Cargo.toml` and
`spine-app/tauri.conf.json` to match) and push to `main`. GitHub Actions
builds Spine.exe, fingerprints it, and publishes release `v<version>`.
