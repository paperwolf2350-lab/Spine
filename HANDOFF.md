# Spine — handoff for the next session

Read this first. It explains what Spine is, how it's built and shipped, where
things stand, and the next feature (Movies & TV) with a ready-made plan.

## What Spine is

A console-style game library for PC (Windows and Linux). Point it at a ROM
folder and it sorts games by console and A–Z, gets covers, banners, logos,
screenshots, trailers and descriptions, and starts each game with the right
emulator. Steam / Epic / GOG games join in. Works with mouse, keyboard and
controller. It updates itself from this repository's GitHub releases.

The owner is not a programmer: explain in plain words, test before shipping,
and never ask them to type secrets into chat (they enter keys in
Settings → Scrapers themselves).

## Layout

- `spine-core/` — Rust library: scanning (`scan.rs`, `title.rs`, `systems.rs`),
  covers & art (`covers.rs`, `extras.rs`), categories & PC details (`meta.rs`),
  ScreenScraper (`screenscraper.rs`), RetroAchievements, BIOS checks (`bios.rs`),
  emulator detection & launching (`launch.rs`, incl. Flatpak on Linux), PC stores
  (`stores.rs`: Steam, Epic, GOG, Heroic). Also builds the `spine-scan` CLI.
- `spine-app/` — Tauri 2 desktop app. `src/main.rs` (commands, state, media
  server on `spine://`), `src/update.rs` (self-update), `src/pad.rs` (Select+Start
  exit: XInput on Windows, evdev on Linux), `src/linux.rs` (data dir, menu entry).
  Interface is plain HTML/CSS/JS in `spine-app/ui/` (no build step): `app.js`, `app.css`.
- `release-notes.md` — shown in each release and (above the `---`) in Settings → Updates.
- `install.sh` — one-line Linux installer.
- `.github/workflows/release.yml` — builds and publishes releases (see below).
- `ROADMAP.md` — done / next.

Data lives next to Spine.exe on Windows (portable), `~/.local/share/spine` on Linux:
`spine-state.json` (favorites, playtime, settings, lists...), `credentials.txt`
(scraper accounts — never commit, never print), `media/` (covers, heroes, logos,
shots, videos, info, lists, cache).

## How to ship a change

1. Make the change; run `cargo test` in `spine-core` and `spine-app`.
2. UI changes: test with Playwright against the mock (the old `uitest/` folder
   isn't in the repo; recreate a small mock: `window.__SPINE_MOCK__` with
   `call(cmd,args)` / `on(ev,cb)`, and `window.__BOOT__` = a Boot object).
3. Write `release-notes.md` (What's new, then `---`, then install text — keep it).
4. Bump the version in `spine-app/Cargo.toml`, `spine-core/Cargo.toml` (and the
   `spine-core = { version = ... }` dependency line), and `spine-app/tauri.conf.json`.
5. Push to `main`. GitHub Actions tests on Linux + Windows, builds Spine.exe,
   Spine-Setup.exe (NSIS), Spine-x86_64.AppImage and Spine-amd64.deb, each with a
   `.sha256`, and publishes release `v<version>`. Users get it via Settings → Updates.
   Pushing tags from the Claude session is blocked; that's why releases key off
   the version on `main`.
6. Watch the run: `https://api.github.com/repos/paperwolf2350-lab/spine/actions/runs?per_page=1`.

Gotchas learned the hard way:
- Tests that use `HOME` or Linux paths must be `#[cfg(unix)]` (Windows CI fails otherwise).
- Steam and ScreenScraper are blocked from the Claude workspace; test parsers with
  sample JSON and verify on the owner's PC.
- Media elements removed from the page keep playing: `silenceDetached()` in app.js
  handles it — keep using `liveMedia` for any new video/audio.
- The owner's ROM files have ranking prefixes ("02. Super Mario 64 (USA).z64").
  Anything that matches by file name must use `covers::strip_rank`.

## Where things stand (end of Oct 2026, v0.4.3)

- Windows: the owner's PC runs Spine from `C:\Users\brand\Documents\spine`
  (portable, updates itself). ~680 games over 12 consoles + Steam/Epic.
- Linux: released and tested in a virtual screen; not yet on real hardware. The
  owner is buying a Linux Mint Cinnamon laptop to test (install with the
  one-liner in README; if the AppImage won't open: `sudo apt install libfuse2t64`
  or use the .deb).
- ScreenScraper: developer access approved; the owner entered keys in Settings.
  0.4.3 fixed matching (rank prefixes, CRC32, search-by-name fallback, retries old
  misses). **First thing to do:** ask how the re-scan went, then count results per
  console (files in `media/info/<console>/`: ~27-byte files are "not found"
  markers, larger ones have details).

Ideas already discussed (not started):
- Build the ScreenScraper developer credentials into releases as a GitHub
  Actions secret (owner adds the secret himself), so friends only need their own
  free ScreenScraper account.
- List Spine on Flathub for one-click Linux installs.

## Next feature: Movies & TV

The owner wants a media section: movies and series organized like the games,
with posters/cover art, details, and seasons/episodes. Agreed approach:

**Metadata: TMDB** (themoviedb.org), not IMDb (no app-friendly API).
- Free API key, entered by the owner in Settings → Scrapers (same pattern as
  SteamGridDB: stored in credentials.txt, never echoed back).
- Movies: `/search/movie?query=&year=` → `/movie/{id}` (overview, release_date,
  genres, runtime, poster_path, backdrop_path), `/movie/{id}/images` (logos).
- TV: `/search/tv` → `/tv/{id}` (seasons) → `/tv/{id}/season/{n}` (episodes:
  name, overview, still_path, air_date).
- Images: `https://image.tmdb.org/t/p/w500{poster_path}`, `original` for backdrops.
- Cache everything under `media/` like game art; "not found" markers like games.

**Finding and organizing files** (new scanner, similar to ROM scanning):
- Movies: `Movie Name (2010).mkv` or `Movie Name (2010)/Movie Name (2010).mkv`.
- Shows: `Show Name/Season 01/Show Name S01E02 - Title.mkv`; also `1x02` and
  `S01E02` anywhere in the name. Strip release junk (1080p, x264, BluRay, group tags).
- Media folders are separate from ROM folders (Settings → Movies & TV folders),
  with the same unplugged-drive handling as ROM folders.

**Playing**: launch an external player, like an emulator.
- Prefer **mpv** (or **VLC**): detect on Windows (Program Files) and Linux
  (Flatpak `io.mpv.Mpv` / `org.videolan.VLC`, or installed).
- The built-in WebView player can't handle many MKV/HEVC files — don't rely on it.
- Resume: mpv `--start=<seconds>` and `--input-ipc-server` to read the position
  on exit (VLC: `--start-time`); store per-file position + watched flag in
  spine-state.json. Select+Start exit already works for any launched process.

**Interface**:
- Home: a "Movies" tile and a "TV" tile beside the consoles; "Continue watching"
  row next to "Continue playing".
- Movie page: backdrop banner, logo, overview, year, runtime, genres (clickable
  categories like games), Play / Resume.
- Show page: season picker, episode list with thumbnails, watched checkmarks,
  "Next episode" button.
- Same layouts, A–Z rail, search, sounds and controller navigation as games.

**Suggested first step (keeps it small)**: movies only from one folder, TMDB
posters + details, opening in mpv/VLC. Then TV seasons/episodes, then resume
and Continue watching.

Owner's credit budget is limited: start in a fresh session (cheaper), work in
small releases, and confirm scope before big steps.
