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
- Anything Spine starts must go through `spine_core::hostenv::command` (or
  `hostenv::sanitize`): inside the AppImage, LD_LIBRARY_PATH/GIO/GTK/GST settings
  point at bundled libraries and crash host programs (this broke updates and
  RetroArch on Mint until 0.4.5). On Linux, downloads use the built-in ureq
  downloader first.
- `ureq` is a Linux-only dependency of spine-core: don't use it in shared code
  (0.7.0 first failed on Windows CI). Check with `cargo check --tests --target x86_64-pc-windows-gnu`.
- The owner's ROM files have ranking prefixes ("02. Super Mario 64 (USA).z64").
  Anything that matches by file name must use `covers::strip_rank`.

## Where things stand (Nov 2026, v0.7.5)

- Windows: the owner's PC runs Spine from `C:\Users\brand\Documents\spine`
  (portable, updates itself). ~680 games over 12 consoles + Steam/Epic.
- Linux: works on the owner's Linux Mint Cinnamon laptop (v0.4.5): games launch
  through RetroArch. Not yet confirmed there: the Select+Start exit, Flatpak
  standalone emulators, Steam/Heroic games. If the AppImage won't open:
  `sudo apt install libfuse2t64`, or use the .deb.
- ScreenScraper: developer access approved; the owner entered keys in Settings.
  0.4.3 fixed matching (rank prefixes, CRC32, search-by-name fallback, retries old
  misses). Result on the owner's PC: 682 of 683 games have details. The one
  miss is "Horizon Chase Turbo" (a PC shortcut Steam search didn't match).

- ScreenScraper developer key is built into official releases from repository
  secrets SCREENSCRAPER_DEVID / SCREENSCRAPER_DEVPASSWORD (`config::builtin_dev_key`,
  via `option_env!` at build time). Users only add their own optional account.

Ideas already discussed (not started):
- List Spine on Flathub for one-click Linux installs.
- YouTube showcase video. The script is drafted as a Claude Doc ("Spine — YouTube
  Showcase Script", https://claude.ai/artifact/QmH3FRGe7udhD1cHcKxtMV). The owner's
  mic is broken, so the voice will be TTS. The owner records the footage with OBS
  (no mic needed) and Claude helps with the rest: rewriting the script for TTS,
  suggesting voices, a shot list, captions, title/description/chapters, and edit
  advice. Recording tips: mute trailer audio (copyright claims), don't show
  Settings → Scrapers with keys filled in, let covers load first, and test
  Select+Start on Linux before filming it.

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

**Step 1 is done (v0.5.0, proof of concept)**:
- `spine-core/src/movies.rs`: scanner (name/year cleanup, skips extras, samples,
  trailers, S01E02 files and Season folders; duplicates keep the bigger file),
  TMDB lookup (`fetch_details`; API key or Read Access Token both work), poster →
  cover, backdrop → hero, English logo → logo, details → info JSON incl. `runtime`.
- Movies are one more system, id `movies` (`systems.rs`), launched by the
  `mpv` / `vlc` entries in `launch.rs` (Program Files, /usr/bin, Flatpak).
- App: `movie_roots` in spine-state.json (Settings → Movies), `add_movie_root` /
  `remove_movie_root`, `tmdb_key` in Scrapers. `fetch_media` runs TMDB first and
  keeps movies out of all game-art sources (SteamGridDB would give game covers).
  TMDB key errors reach the UI as a `fetch-error` event.
- UI: Movies tile on Home, "movie/watched" wording, runtime on the detail page;
  "My own video", "Fix name or cover" and achievements are hidden for movies.
- Confirmed working on the owner's PC (Oct 31): folders, TMDB posters/details
  and playback all as intended.

**Step 2 is done (v0.6.0)**: TV shows.
- `spine-core/src/tv.rs`: each folder in a TV folder is a show; same-named folders
  merge (WIND BREAKER S01 + Wind Breaker S02). `Game.episodes` holds the files in
  viewing order, grouped "Season N" / arc name / "Movies & specials". Seasons come
  from S01E02, "Season N"/"S02" folders, "Show II" folders, or a folder named like
  the show (season 1). Absolute numbering inside a season folder is renumbered
  (JJK Season 2 025–047 → 1–23). NCOP/NCED/OP1/ED1 and "Extras" folders skipped.
  Unnumbered files (movies in show folders) go to "Movies & specials".
- `spine-scan <folder> --tv` prints how a folder is sorted. Checked against the
  owner's full D:\anime listing (14 shows, all correct except arc order).
- TMDB: `/search/tv` → `/tv/{id}` (poster, backdrop, logo, info) → `/tv/{id}/season/{n}`
  for each season on disk; saved as `media/info/tv/<slug>.seasons.json`, stills at
  `media/stills/tv/<slug>/s{n}e{m}.jpg`. Arcs (no season number) get no TMDB episode data.
- App: `tv_roots` (Settings → TV), `add_tv_root` / `remove_tv_root`, `play` takes
  an optional `episode` index. GameView carries `episodes` (EpisodeView).
- UI: TV tile, show page with season chips and an episode list (thumbnail, S1 · E3,
  title, two-line summary). Play button plays the first episode of the chosen season.
- Owner's anime: D:\anime (external drive), many EMBER / [DB] / [Anime Time] releases.

Step 3 is split in two releases (owner's choice). **0.7.0** (Oct 31): resume,
watched and Next episode, VLC only (the owner uses VLC). `spine-core/src/player.rs`:
VLC gets `--extraintf=http --http-host=127.0.0.1 --http-port=<free> --http-password=<random>`
(+ `--start-time`), Spine polls `/requests/status.json` every 3 s and saves the last
position on exit in spine-state.json `progress` (movie id, or "<show id>|<file name>"
→ {pos, len, watched, at}). Watched = past 90% or within the last 60 s; under 30 s
isn't resumable. `play` takes `from_start` (Start over); `set_watched` ticks/unticks.
UI: `showNext` picks Resume / Next / Play from the most recent `at`; round check per
episode, progress bar on thumbnails; movies get Resume + Start over + Mark as watched.
Tested against real VLC 3.0.20 on Linux (start-time, status, password, play-to-end).
**0.7.1**: "Continue watching" row on Home (from last_played, movies/tv only; shows
use `showNext`; fully watched shows and finished movies drop off; clicking opens the
page with Resume focused). "Continue playing" is games only. TV/Movies list header
says shows/movies; filter and sort say "watched" (`sortLabel`).
**0.7.2**: search box says "titles" when movies/shows exist; detail hint uses `things()`.
README has screenshots in `docs/screenshots/` (owner's PC, resized to 1600x900;
the movie file name was painted out — keep release names out of public images).
Movies & TV step 3 is done. Not done: mpv resume (would need `--input-ipc-server`),
Surprise me can still pick a movie, "Fix match" for whole shows.
**0.6.1** (from the owner's real TMDB data, Oct 31): TMDB lists many anime as one
long season (Jujutsu Kaisen S1 = 59 eps, no S2; Wind Breaker S1 = 25). `tv::resolve`
maps each file to a TMDB (season, episode): exact match; else for season N>1 that
TMDB lacks, the file's own absolute number (`Episode.absolute`, "025") or the count
after earlier seasons on disk. Arcs: `Seasons.list` (TMDB's season list) feeds a
"Which season is this?" picker; answers live in spine-state.json `arc_seasons`
("<show id>|<arc>" → {season, start}); `set_arc_season`. Matched arcs sort by season.
Stills are now named by TMDB numbering. Checked on the owner's PC: Godzilla S.P,
Kimetsu S1, Goblin Slayer S1/S2 matched fine.
Owner confirmed the arc picker fixed Demon Slayer.
**0.6.2**: specials (unnumbered files in a show folder) are searched among TMDB
*movies* (`tv::special_queries`: full clean file name without "Movie", then the part
after the show name); results in `Seasons.specials` by file name (tmdb 0 = not found),
picture at `stills/tv/<slug>/special-<file>.jpg`. "Fix match" on the Movies &
specials tab stores a typed name in spine-state.json `special_queries`
("<show id>|<file>") and `forget_special` clears the old result.
Confirmed on the owner's PC (Oct 31): all three specials (JJK 0, Goblin's Crown,
Demon Slayer Mugen Train) matched automatically, no Fix match needed.
Remaining ideas: "Fix match" for whole shows and for the Movies shelf.
Known gaps for later: renaming a movie doesn't re-run the TMDB lookup; Surprise me
can pick a movie; "Continue playing" lists movies too.

**0.7.3** (Nov 1): A–Z column fix. With Interface size above 100% (incl. Auto on
big screens) `document.documentElement.style.zoom` is set; newer WebView2/Chromium
returns zoomed getBoundingClientRect values while scrollTop stays unzoomed, so
`ACT.jump` landed on the wrong letter. It now divides by
`box.getBoundingClientRect().height / box.offsetHeight`. Any new code that mixes
rect positions with scrollTop/scrollLeft needs the same conversion.

**0.7.4** (Nov 1): Fix match for whole shows: `set_show_query` stores a typed name in
spine-state.json `show_queries` (show id -> name) and `tv::forget_show` deletes the
show's TMDB info, seasons.json, downloaded cover/hero/logo and stills (own cover kept),
so the next `fetch_media` looks it up again; `tv::split_year` turns "Name (2024)"
into a year filter. UI: "Fix match" under a show's cover (`showPanel`).
Resume in mpv: Spine writes `player::MPV_SCRIPT` (Lua) to media/cache/mpv and passes
`--script=` + `--script-opts=spine-progress=%<len>%<file>` + `--start=`; the script
writes "<pos> <len>" every 2 s and on unload; read like VLC's status (`Tracker` in
main.rs). Flatpak mpv gets `--filesystem=` for that folder. Tested with real mpv 0.37
(Linux). Surprise me (`ACT.random` scope 'all') skips movies/tv.

**0.7.5**: Fix match for movies too: same `show_queries` map, `set_show_query`,
`tv::forget_show` and `showPanel`; `movies::fetch_details` takes a `typed(id)` name
(split with `tv::split_year`).

Owner's credit budget is limited: start in a fresh session (cheaper), work in
small releases, and confirm scope before big steps.
