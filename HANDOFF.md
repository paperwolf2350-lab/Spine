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

## Where things stand (Nov 2026, v0.32.0)

- Windows: the owner's PC runs Spine from `C:\Users\brand\Documents\spine`
  (portable, updates itself). ~680 games over 12 consoles + Steam/Epic.
- Linux: works on the owner's Linux Mint Cinnamon laptop (confirmed on 0.7.6, Nov 1):
  RetroArch games, Select+Start exit, Flatpak standalone emulators, Steam/Heroic
  games and movies/TV in VLC all work. If the AppImage won't open:
  `sudo apt install libfuse2t64`, or use the .deb.
- ScreenScraper: developer access approved; the owner entered keys in Settings.
  0.4.3 fixed matching (rank prefixes, CRC32, search-by-name fallback, retries old
  misses). Result on the owner's PC: 682 of 683 games have details. The one
  miss is "Horizon Chase Turbo" (a PC shortcut Steam search didn't match).

- ScreenScraper developer key is built into official releases from repository
  secrets SCREENSCRAPER_DEVID / SCREENSCRAPER_DEVPASSWORD (`config::builtin_dev_key`,
  via `option_env!` at build time). Users only add their own optional account.

Ideas already discussed (not started):
- Chromecast / casting and Trakt: dropped by the owner (Nov 4). Don't suggest them again.
- Flathub: dropped (Nov 1). Its policy (May 2026) rejects AI-written apps and AI-made
  manifests/submissions, and Spine needs to start host programs. Don't suggest it again.
- YouTube showcase video: DONE (Nov 1), ready for the owner to upload. 3:32 cut with a
  Kokoro TTS narrator (voice bm_george, speed 0.95; Apache 2.0, made in the workspace with
  kokoro-onnx + model files from its GitHub releases), owner's OBS clips, soft crossfades, end card,
  plus a 1280x720 thumbnail. Script, title ideas and the description with chapters are in the
  "6-minute cut (TTS)" tab of https://claude.ai/artifact/QmH3FRGe7udhD1cHcKxtMV. The owner
  prefers long continuous takes over quick cuts. Description says Spine was coded with Claude.

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

**0.7.6**: Linux layout. WebKitGTK (the AppImage's engine) doesn't scale `100vh` with
the root `zoom`, so `#app { height: calc(100vh / var(--z)) }` came out 1/zoom too short
and Home's flex column squeezed the hero (consoles row hidden). `#app` is now
`position: fixed` with top/right/bottom/left 0, and `#homescroll > *` don't shrink.
Checked in real WebKitGTK 2.52 under Xvfb (python3.12 + gi WebKit2 4.1) before/after.
Avoid vh/vw for layout: zoom handling differs between engines.

**0.7.7** (Nov 2): Xbox 360 is its own system (`xbox360`, folders xbox360/x360), launched by
`xenia` (xenia_canary.exe / xenia.exe; `--fullscreen {rom}`). Before, "Xbox 360" prefix-matched
`xbox` and went to xemu. Exts iso/xex/zar; an extracted game's `default.xex` takes its folder's
name. Art: libretro `Microsoft_-_Xbox_360`, ScreenScraper 33. Confirmed working on the owner's PC (Blur, Nov 2).

**0.7.8**: Retro CRT theme (`theme: "crt"`), CSS only at the end of app.css: bundled VT323 font
(`ui/fonts/`, OFL; the other fonts load from Google Fonts), `font-size-adjust: 0.52` (VT323's
x-height is 0.40), scanline + vignette overlay on `body::after`, power-on flash on `body::before`
(plays when the theme switches on; hidden with reduced motion). Plan for more looks is in ROADMAP:
next is a "Home style" setting (separate render per style, own file, game/movie pages shared)
starting with Wave (XMB-like), then Channels (Wii-like), then Blades (360-like). Own names, no
console makers' logos/fonts/sounds.

**0.8.0**: Home style setting (`home_style`: "classic" default / "wave"; Settings → Home style).
Wave lives in its own files, `ui/wave.js` + `ui/wave.css` (loaded before app.js / after app.css).
`home()` hands off to `homeWave()` when `waveOn()`. Categories = optional "Recent" + `tiles()`;
each category's list = an "All N games"/"Open list" opener + its games (Surprise me: one item).
State in `S.wave` {cat: tile id, at: {tile id: index}}; `S.sel` follows the console so Classic
and Back-from-library stay in step. Moving doesn't call render(): `waveSync` slides with CSS
transforms (fixed sizes in `WV`, keep in sync with wave.css). `press()` sends arrows/LB/RB/LT/RT
to `waveKey` on Wave home; `focusEl` skips scrollIntoView inside `.wave` (it would scroll the
hidden-overflow box). Click on an unhighlighted game highlights it (`waveClick`), wheel moves.
Background: `--wc` (console color) mixed into `--bg`, plus a canvas wave (`waveMount`, ~30 fps,
paused while playing/hidden, one still frame with reduced motion). New icon `SYS_ICON.recent`.
Tested in Playwright (Chromium) with Cream, Midnight and CRT at 1440x860 and 1920x1080.
Next styles per ROADMAP: Channels, then Blades — same pattern (own file, `home_style` value).

0.8.0 confirmed working by the owner on Windows and Mint (Nov 2).

**0.9.0**: Channels home style (`home_style: "channels"`), `ui/channels.js` + `ui/channels.css`.
4x3 pages of `tiles()` (`CH_PER_PAGE`), data-key `con-<id>` like Classic so Back/LB/RB/restoreFocus
just work; the page follows the focused `con-` key (`S.chPage`). `channelsKey` turns the page when
left/right go past a page edge; otherwise normal spatial `move()`. No topbar/hints: the curved
bottom bar has Settings (key `gear`), clock (`#clock`, updated by the existing timer) and Search
(key `searchbtn`). Tiles: a drifting strip of up to 7 covers (CSS animation, per-tile speed/offset).
Home music `BGM` (in channels.js): original tune, synthesized with Web Audio (no files), 16 bars,
100 BPM swing, F major lounge/bossa: EP chords, triangle-pluck melody (rests every other loop),
triangle bass, noise shaker/rim, soft kick, small convolver reverb. Plays only on the Channels home
and not while `S.playing`; `BGM.sync()` after every render. Setting `homeMusic` off/soft/normal
(Settings → Home music). Owner asked for music "reminiscent of but not a copy of" a console menu
theme: keep the melody and chords original. tauri.conf.json now passes WebView2
`--autoplay-policy=no-user-gesture-required` (plus Tauri's default --disable-features list) so
music starts without a click on Windows; first key/click also resumes audio as a fallback.
Preview MP3 was rendered with OfflineAudioContext in Playwright (fake currentTime).

**0.9.1**: Home music is now the owner's own songs: `ui/music/wave.mp3` and `ui/music/channels.mp3`
(owner made them; ~3:25 and ~3:04). Loudness-normalized from -14 to -20 LUFS (ffmpeg two-pass
loudnorm, TP -1.5, 160 kbps). `ui/music.js` (`BGM`) plays one looping <audio> per style
(`SONGS` map: add a file + entry for a new style), fades out off Home / while playing, resumes
where it paused. The 0.9.0 synthesized tune was removed (it's in git history). `press()` calls
`BGM.wake()` so a controller press can start audio if autoplay was held back. Linux AppImage plays
MP3 through its bundled GStreamer (plugins good/bad/libav in release.yml).

**0.10.0**: Blades home style (`home_style: "blades"`), `ui/blades.js` + `ui/blades.css`. Blades =
`waveCats()`, each blade's list = `waveItems()` (items now also carry `g`/`s`, `opener`, `random`).
State `S.blade` {cat, at, dir}; changing blade re-renders (panel slides in via `in-r`/`in-l`).
`bladesKey`: LB/RB always, left/right only while a list item has focus; up/down = normal `move()`
in a real scrolling list. `bladesMount` keeps the right-hand preview in step with focus and hover.
Tabs: `ACT.bladego`. No music (no entry in music.js SONGS). Sounds: SFX has a `blades` set
(`hiss()` filtered-noise helper: clicks, chimes, `bladeR`/`bladeL` swooshes) used for every
interface sound while Blades is the Home style. Also fixed: Channels page arrows had no action
(`ACT.chpage` was missing).

**0.10.0** confirmed by the owner (all styles "look and feel really good", Nov 2).

**0.11.0**: Cube home style (`home_style: "cube"`), `ui/cube.js` + `ui/cube.css`. Belt = `waveCats()`
minus Recent/Favorites (`cubeBelt`); top face = Recent, bottom = Favorites. Only 6 faces: side slots
s0–s3 show belt[i + d] for d = 0/+1/+2/-1 relative to the front slot (`cubeSlotCat`); `cubeSync`
repaints slots whose console changed (they're out of view). State `S.cube` {i, rot (adds up, so turns
go the short way), tilt (-1/0/1), zone 'cube'|'shelf', at}. IMPORTANT: no `preserve-3d`. WebKitGTK
without GPU acceleration flattens 3D children (the cube fell apart in Xvfb), so each face carries the
whole transform itself (`cubeFaceTransform` = cube turn + face place + translateZ) under the stage's
`perspective`, and `cubeShow` hides faces pointing away (visible faces never overlap, so draw order
doesn't matter). Top/bottom face content counter-rotates (top -spin, bottom +spin) to stay upright.
Shelf: all games in the DOM, positions from `cubeGameStyle(offset)`, images load when within ±9.
Keys in `cubeKey` (A/down enters shelf; down from belt goes to Favorites first if there are any).
Sounds: `cube` set in SFX (`cubeR`/`cubeL` = swoosh + landing thunk). Tested in Playwright and in
real WebKitGTK 2.52 (python3.12 + gir1.2-webkit2-4.1 + python3-gi-cairo under xvfb-run, snapshots
via get_snapshot) — `apt-get install gir1.2-webkit2-4.1 python3-gi-cairo` if a new workspace lacks them.

**0.11.1**: Home music on Linux. WebKitGTK can't load <audio>/<video> from Tauri's app scheme
(the element stays at readyState 0, play() rejects), so `music.js` on Linux fetches the mp3, decodes it
with `decodeAudioData` and loops it through Web Audio (`bufferPlayer`: gain-node fades, remembers the
offset on pause). Windows keeps the <audio> element (`elementPlayer`). Autoplay: wry's default
`autoplay: true` already gives WebKitGTK `AutoplayPolicy::Allow`, so Web Audio starts without a click.
Reproduced/tested with a WebKitGTK harness that registers a `tauri` URI scheme serving ui/ (no range
support) and sets the same autoplay policy. Owner reported all styles fine on Linux apart from this.
Any future in-app audio/video on Linux: don't use media elements on bundled files.

## Streaming addons (Stremio-style), started Nov 2

The owner uses Nuvio (phone/TV) with Stremio addons and a **TorBox** debrid account (key put
into each addon's setup page, so it's inside the addon's link). With debrid, addons return plain
https links VLC can play: **no torrent engine needed**. Plan agreed in 3 small releases:
0.12.0 addons + catalogs + details; 0.13 "Choose a source" + play in VLC; 0.14 resume/watched/
Continue watching/Next episode for streamed titles + controller polish.
Stance (owner agreed): Spine ships no addons; the user adds their own links (like BIOS files).
Cinemeta (Stremio's official public details addon) is only used as a details fallback.

**0.12.0**: `spine-core/src/addons.rs`: `normalize` (stremio:// -> https, adds /manifest.json),
`fetch_manifest`, `catalogs` (old extraRequired + new extra[].isRequired; `needs_input` = search/genre
catalogs, not shown as tabs yet), `resources`/`serves` (types + idPrefixes), `fetch_catalog`
(`/skip=N`), `fetch_meta` (source addon, then any addon serving meta, then Cinemeta for tt ids),
`parse_meta` (videos -> episodes, specials last). SECRET HANDLING: links live in `addons.json`
in the data folder ({url, manifest}); the UI only gets `key` = FNV fingerprint of the link
(`key_of`); every error goes through `redact()`. Never log or show `Stored.url`.
Live test against real Cinemeta: `cargo test -- --ignored live_cinemeta` (workspace can reach it).
App: `App.addons`, Boot `addons` (AddonView: key, name, version, logo, catalogs, meta/stream types),
commands `add_addon`, `remove_addon`, `move_addon`, `addon_catalog`, `addon_meta`.
UI: `ui/stream.js` + `stream.css` (state in `S_STREAM`, actions in `STREAM_ACT`, merged into ACT).
Settings → Streaming addons (`addonSettings`, add panel `addonPanel`, S.panel 'addon').
(0.12.0 put catalog tabs on the Movies/TV screens; moved to the Streaming tile in 0.12.2.)
an active tab swaps the grid (`streamGrid`, "Show more" pages) and hides rail/sort/filters.
Details page = screen `sdetail` (`sdetail()`), poster/backdrop/logo straight from the addon's
https URLs (CSP is null), images fall back to a title card on error. Episodes are focusable
buttons with `noop` for now: 0.13 makes them (and a movie's Play) open the source picker.

Tested in Chrome (Playwright, mock backend fed with real Cinemeta JSON, real posters via proxy).

**0.12.1** (owner's request: keep addon results out of the library search): "Search addons" tab
(ck `search:<movies|tv>`, pseudo-catalog in `streamActive`) on the Movies/TV screens. `streamSearchCats`
picks one searchable catalog per addon and type (prefers a search-only one). `addon_search` command ->
`addons::fetch_search` (`/catalog/<type>/<id>/search=<q>.json`). Results grouped per addon, kept after
Back; the input uses `data-enter` (press A in an input runs that action). On this tab, typed letters
and X focus the addon box (`streamSearchFocus`) instead of opening the library search.

**0.12.2** (owner's call: keep local files and addons apart, build streaming out in its own space):
addon stuff moved off the Movies/TV screens into a **Streaming** Home tile (`streamHomeTile()`,
kind 'stream', id 'streaming', shown once any addon exists; not in `browsable()`, so LB/RB in a
library never lands on it). `ACT.tile` opens screen `'stream'` (`stream()` in stream.js): Movies /
Shows switch (`S_STREAM.kind`, `.seg.skinds`), tabs = Search (when an addon can search) + every
catalog (`streamTabList`), default tab = first catalog. Keys: `streamKey` (LB/RB tabs, LT/RT kind).
Back from 'stream' -> Home on the tile (same code as leaving a library); sdetail -> 'stream'.
Home styles: the tile has `label` ("2 addons") used where `count` is null, empty cover pool;
Wave/Blades/Cube list one "Open Streaming" item (`waveItems`). Icon `SYS_ICON.streaming`.
Name clash to remember: `streamTile()` = a poster card in results, `streamHomeTile()` = the Home tile.
Future streaming work goes in stream.js/stream.css and the 'stream' / 'sdetail' screens only.

**0.13.0** (step 2, playing): `addons::fetch_streams` asks every addon that `serves` "stream" for
the type/id (episode ids like `tt0903747:2:3`, in parallel threads), `parse_streams` keeps http(s)
`url` streams (torrent-only `infoHash` results are just counted: no torrent engine, debrid only),
guesses quality/size, `cached` from "[TB+]"/⚡ vs "[TB download]"/⏳, reads
behaviorHints.proxyHeaders. App: `addon_streams` returns SourceViews with a token (`s1`...);
the link stays in `App.sources` (cleared each call). `play_stream(token, title, id)` uses
`launch::plan_stream` (VLC first, mpv if `settings.streamPlayer == "mpv"`; Flatpak `run <id>`;
`--meta-title` / `--force-media-title` so the window shows the title, not the link; UA/Referer
headers) and the same Select+Start exit + "game-exited" event (id `stream/<type>:<id>`, no
progress yet). Errors pass through `addons::redact`. UI: "Choose a source" (key `play`) on
sdetail; episode rows open sources too; `sourcesPanel()` (S.panel 'sources', S.src) groups by
addon with Ready / Not cached badges. Not yet: resume/watched/Continue watching (step 3), and a
real TorBox run (can't reach it from the workspace: owner to check on his PC).

**0.14.0** (step 3): progress for streams uses the same `progress` map, keys `stream/<type>:<id>`
(episode ids for shows, e.g. `stream/series:tt0903747:1:2`). `play_stream(token|null, key, title,
info, from_start)`: token = a fresh source; null = the source played last time, kept in
**stream-links.json** (`App.links`, key -> (time, Source), newest 300; separate file because the
links hold the debrid key; Boot only gets the keys as `stream_links`). Same VLC/mpv Tracker as
local movies. `info` (from `streamInfo()` in stream.js: item "<type>:<id>", name, poster, addon,
episode, last) is saved in spine-state.json `stream_recent` (newest 40) for Continue watching;
`stream_forget(item)` removes one. UI: `sShowNext` (resume / next / again, by latest `at`, like
local `showNext`), Resume / Start over / Other source / Mark as watched on movies, checkmarks +
progress bars on episodes (row resumes if it can, else opens sources), `streamContinueRow` above
the catalog grid (not on Search). Streaming steps 1–3 are done.

**0.15.0**: Settings split into sections: `SET_TABS` (look / library / streaming / play / system), state
`S.setTab` (`setTab()` falls back to 'look'), chip row `.settabs` (sticky) with keys `stab-<id>`, `ACT.settab`;
LB/RB switch sections while `S.panel === 'settings'` (in `press`); restoreFocus prefers `stab-<tab>`.
Notes cut to one short line each. Home tile order: setting `tileOrder` (list of tile ids) applied at the end
of `tiles()` by `orderTiles` (a tile not in the list, e.g. a new console, stays right after the tile it
normally follows), so every Home style and LB/RB follow it. Settings → Look → Home tiles (`tileOrderGroup`):
↑/↓ per tile (`ACT.tilemove`, keys `tup-<id>`/`tdn-<id>`, focus stays on the moved button),
`ACT.tilereset` saves []. `keepTilePlace()` keeps `S.sel` / `S.con` on the same tile across a reorder.
Surprise me's count skips movies/tv. Tested in Playwright (Chromium) with a mock backend.

**0.16.0**: Streaming lists. spine-state.json `stream_lists` (name -> titles, newest first; each
{item "<type>:<id>", type, id, name, poster, year, addon, addonName, at}; `stream_list_apply` keeps only
those keys, no links). "Favorites" / "Watch later" (`STREAM_BUILTIN`) vanish when emptied; own lists stay
until deleted. Commands `stream_list_set(name, info, member)`, `stream_list_delete(name)`; Boot `stream_lists`.
UI (stream.js): `streamMyTabs(kind)` = Continue watching (ck `cw`, full `streamContinue(kind, 99)`) +
Favorites + Watch later + own lists (ck `list:<name>`), each only when it has titles of that kind; tabs go
Search, yours, a `.tabsep`, catalogs; default tab is still the first catalog. `streamFind(ck, i)` resolves a
poster from a catalog/search/list (used by `sopen` and Y). Y on the Streaming screens goes to `streamY`
(before the game Y handler in `press`). Details page: Favorite / Watch later / Add to list (`slists` panel,
`streamListsPanel`, new list via `slnew`). Hearts (`sFavDot`) on posters. Tested in Playwright with a mock.
**0.16.1**: the Continue watching row above catalogs was removed (owner: doubled up with the tab).

**0.17.0**: Streaming filters. `addons::Catalog` now has `genres` (the "genre" extra's `options`,
or old-style `genres`) and `genre_required`; a catalog whose only required extra is a genre with choices
is browsable (first choice used). `catalog_url(base, typ, id, skip, genre)` -> `.../genre=X&skip=N.json`;
`addon_catalog` takes `genre`. `Item.country` (string or array) feeds the Anime guess. UI (stream.js):
`sGenre(c)` per catalog (`S_STREAM.genre[ck]`), cache key `sCacheKey(c)` = ck or `ck#genre`;
`streamFilterBar` (Genre/Year chip opens panel `sgenre` = `streamGenrePanel`; `sYearPicker` when all
choices are 4-digit years, then the decade chip hides), client filters per kind in `S_STREAM.filt[kind]`
{anime, decade, rating} via `sMatch` / `sIsAnime` (type anime, kitsu:/mal:/anilist:/anidb: ids, "Anime"
genre, or Animation + first country Japan — Japan-only would lose co-productions like Spirited Away).
With client filters on, `streamLoad` auto-loads up to 8 more pages until 24 match (`d.auto`).
Filters only on catalog tabs (not Search or your lists). Tested in Playwright with real Cinemeta pages
and against live Cinemeta (`cargo test -- --ignored live_cinemeta`, now checks a genre page).

**0.17.1**: Channels tiles with 1–2 covers (`chTileArt`) get a "featured" look: covers stand still
(right of the name label, two fan out) over the first game's hero (or its cover, heavily blurred), panning
slowly (`.ch-show`, `chpan`). 0 covers: `.ch-plain` = gradient + glow in the console color. 3+ is the
old drifting strip. Streaming tile pool = `chStreamPosters()` (stream_recent newest first, then
stream_lists; addon https URLs, not mediaUrl). Surprise me's dice = `chBadge`; `.ch-art` has
`isolation: isolate` so the dice tint stays under the name label. Tested in Playwright, Cream + Midnight.

## Spine's own video player (mpv), started Nov 2

Owner asked for intro skip / autoplay next. Agreed: mpv becomes Spine's player (owner happily
dropped VLC), dressed up by Spine's Lua script rather than embedded in the window (embedding
libmpv under a Tauri webview was judged too risky). Plan: 0.18 Up next; then Spine downloads mpv
on Windows (official Windows builds are .7z: needs a 7z reader); 0.19 Skip intro / credits from
chapters ("Opening"/"OP"/"Ending"...); 0.20 AniSkip (community anime intro times) without chapters.

**0.18.0**: Up next. `player::MPV_SCRIPT` now also takes `spine-next=<label>` and `spine-wait=<s>`
(`mpv_args(.., Some(&UpNext))`, which also adds `--keep-open=yes`). On `eof-reached` it draws an
ASS card (bottom right, 1920x1080 canvas) counting down; Enter / A = now, Esc / B / Backspace =
stay. When it runs out it writes "<pos> <len> next" and quits; `Tracker::finish` reads it and
"game-exited" carries `next: true`. The interface decides what's next: local shows `epAfter(g, i)`
(viewing order from `epOrder`, never into Movies & specials), stored in `S.playing.next`, then
`ACT.play`; streams `sUpNext` + `ACT.snext` (asks `addon_streams` for the next episode id and
`sBestSource` picks: same addon, same quality, Ready first; picks remembered in `sPicks`).
Settings (Controller & sound): `autoplayNext` (bool) + `autoplayWait` (3-60 s, default 10);
`streamPlayer` mpv/vlc. `plan_stream` now tries mpv first. Tested in real mpv 0.37 under Xvfb
(card drawn, countdown, quit with "next"; Esc keeps mpv open). Between episodes mpv closes and
reopens (a short flash of Spine).

**0.18.1**: Settings `audioLang` / `subLang` ("" = file's choice, "off" for subs, or en/ja/es/fr/de/pt/it/ko)
-> `player::lang_args` (mpv `--alang/--slang/--sid`, VLC `--audio-language/--sub-language`), added by `langs()`
in main.rs for local movies/TV and streams. Checked in real mpv with a two-audio/two-sub MKV.

**0.19.0**: Spine's player look (owner's feedback: mpv's own controls looked dated; examples were
Disney+/Netflix players). The mpv script is now its own file, `spine-core/src/spine_player.lua`
(`include_str!` as `player::MPV_SCRIPT`), started with `--osc=no --osd-bar=no --osd-on-seek=no`.
It draws everything with one ASS overlay sized to `osd-dimensions` (scale S = min(w/1920, h/1080)):
top/bottom gradients (exact-edge bands, overlaps showed as lines), back + title/sub, seek bar with
chapter gaps + hover time, round buttons (play, -10, +10, mute, next, tracks menu, fullscreen), a
tracks menu (aid/sid), a Skip pill and the Up next card. `hit()` records clickable boxes each render;
MBTN_LEFT (complex) reads `mouse-pos` fresh and re-renders before hit-testing. Chapters: `kind_of`
(intro: OP/Opening/Intro...; credits: ED/Ending/Credits/Preview). Credits with only credits after
them + an Up next = "Next episode". `PlayerUi` {title, sub, accent, skip, next, wait}: the interface
sends `look` {title, sub, accent} (`playerLook()` splits "Show · rest" at the first " · ", accent
from CSS `--accent`); setting `skipIntro` button/auto/off. Gamepad keys bound but mpv needs SDL
(`pcall(set input-gamepad)`); Select+Start still closes. Tested in mpv 0.37 under Xvfb with a
chaptered MKV (screenshots of each state). Not yet: AniSkip for files without chapters.

0.19.0 confirmed by the owner ("looks and works great", Nov 2).

**0.20.0**: AniSkip. `spine-core/src/aniskip.rs`: `Lookup` (Tmdb {tv, season, episode} for local
shows via `tv::resolve`; Imdb / Kitsu / Mal from stream keys, `from_stream_key`), `mal_episode` maps
to a MyAnimeList id + episode with Fribb's anime-list-mini.json (raw.githubusercontent, cached a week
in media/cache; `pick` = same season, biggest episode_offset below the episode: TMDB's long seasons),
`fetch` asks api.aniskip.com/v2/skip-times/<mal>/<ep> (op, ed, recap; mixed-* ignored: they overlap
the story) and `parse_skips` keeps, per kind, the result closest to our length (dropped if >20 s off).
main.rs: `skips_file()` (media/cache/mpv/skips.txt, deleted at each start), `aniskip_lookup()` thread
waits for the tracker's first length, then writes "op <start> <end>" lines. `PlayerUi.skips` ->
script-opt `spine-skips`; the Lua polls for it (2 s, up to 90 s) and uses it only when the file has
no Opening/Ending chapters. AniSkip credits with <45 s after them + Up next = "Next episode"; auto
mode skips intro/recap once. Live test: `cargo test -- --ignored live_aniskip` (both hosts reachable
from the workspace). Checked the Lua under Xvfb with a chapter-less file.

**0.20.1**: owner's test: Demon Slayer S1E3 got the button, Edgerunners S1E2 didn't (mapping was right;
AniSkip's only submission was for a 1534 s version). `parse_skips` now allows up to 120 s difference:
op/recap keep their times, ed is shifted by (our len - their len) so it stays anchored to the end.

**0.20.2**: Windows self-update failed with "Couldn't move the current Spine aside (Access is denied)"
(the previous Spine.old.exe was still there and locked). `update::install` now falls back to
Spine.old2.exe ... Spine.old19.exe and retries the rename a few times; `cleanup` removes those too;
`is_old()` recognises any of them.

AniSkip confirmed on the owner's PC (Nov 2): Demon Slayer and Edgerunners S1E2 (after 0.20.1) skip at the right time.
(The 0.20.0 -> 0.20.1 "Access is denied" was two Spine windows open; 0.20.2 handles it anyway.)

**0.20.3** (Nov 4): Fix match per special. Each "Movies & specials" row has a pencil (`.epfix`, key
`spfix-<i>`, next to the watched check; `.eprow.sp` has 3 columns); `specialPanel` shows only that file
(`S.spFix` = {id, ep}). "Not on TMDB" stores `tv::NO_MATCH` ("-") as the special query: `fetch_details`
skips the lookup and records it, EpisodeView gets `off: true` (no note, file's own title). Empty box =
Spine's own guess again. Owner found it with Attack on Titan (OADs matched a UFC event and Girls und Panzer).

Owner confirmed 0.20.3: everything TMDB has as a movie now matches.

**0.20.4**: specials also from TMDB season 0. `fetch_details` fetches season 0 for shows with any special
(`TmdbEpisode` now keeps `air`, `runtime`). Per special: Fix match pick `s0:<n>` (`tv::S0_PREFIX`) wins,
else `tv::match_special` (file's words, minus show name and OVA/OAD/Special..., must be >= 75% inside
TMDB's episode name; ties = no match), else the movie search as before. `Special` gained `episode`
(season 0 number, note says "Special · year") and `v` (1 = checked with season 0; old not-found entries
with v 0 are retried). GameView `specials` (SpecialChoice: number, name, year) feeds a pick list in
`specialPanel`. If season 0 isn't enough, next idea agreed with the owner: AniList as a backup for anime.

**0.21.0** (Nov 4): Streaming "Watched" tab. spine-state.json `stream_watched` (item "<type>:<id>" -> list-style
entry via `list_entry`, same fields as stream_lists, newest 2000). Added by `watched_note` when a streamed movie/episode
finishes (play_stream's tracker, using the play's `info`) or is ticked (`set_watched` now takes optional `info`;
`sWatchedInfo` in stream.js builds it from S.sitem). Unticking removes the title once `still_watched` finds no
watched key ("stream/<item>" or "stream/<item>:..."). `backfill_watched` at startup fills it from stream_recent.
UI: tab ck `watched` last among "your" tabs (`streamMyTabs`), lines from `sWatchedLine`; the interface mirrors
changes locally (`sWatchedMirror`, `sWatchedAfterPlay` from game-exited). Local Movies/TV: not included (owner's call).
Feature list from Nov 4, easiest first: Watched tab (done), clickable actors/directors (done, 0.22.0), controller in the mpv player, optional filter to hide genres and age ratings (owner reworded it
Nov 4; genres are easy since addons send them; age ratings need TMDB certifications, addons don't send them).

Owner confirmed 0.21.0 (Nov 4): already-watched shows appeared in Watched.

**0.22.0** (Nov 4): clickable cast & director on Streaming details pages (`.speople` chips, act `sperson`). Screen
`sperson` in stream.js; `S_STREAM.people` is a stack {name, director, from: S.sitem, fromKey, data|loading|error} so
title -> person -> title -> person and Back retrace. `spine-core/src/people.rs`: `fetch_person` (/search/person,
`pick_person` = same name, then known_for_department, then popularity; /person/{id}?append_to_response=combined_credits;
`parse_person` drops talk/news/reality and "Self"/"Himself" appearances, merges duplicate titles, sorts by vote_count),
`imdb_id` (/movie|tv/{id}/external_ids). Commands `person_credits(name, director)` (Err "no-tmdb-key" -> UI searches
addons for the name on the Search tab: `sPersonFallback`) and `tmdb_imdb(kind, tmdb)` (resolved on click, cached in
`S_STREAM.imdb`). A title opened from a person has addon null (`addon_meta` falls back to any meta addon/Cinemeta),
`fromPerson: true`; details "From" fact says "Your addons". Not testable live from the workspace (needs the owner's
TMDB key): parser tested with sample JSON, UI in Playwright with real Cinemeta meta.
Owner confirmed 0.22.0 working (Nov 4). Next: owner picks between hiding genres (then age ratings) and
controller in the mpv player; mpv download on Windows is still pending too.
Owner's order (Nov 4): hide genres (0.23.0), then Spine downloads mpv on Windows, then controller in the
mpv player (a known mpv build makes gamepad support predictable). Age ratings after that, maybe.

**0.23.0** (Nov 4): Hide genres (UI only, stream.js). Setting `hideGenres` (list of genre names as shown) in
Settings → Streaming (`hideGenresGroup`, chips `hg-<key>`, `ACT.hidegenre`). Names compared by `sGenreKey`
(lowercase letters/digits; `S_GENRE_SAME` maps scifi→sciencefiction, realitytv→reality, talkshow→talk,
kids→children). `sMatch` drops hidden titles; `sActive(f)` = filter chips or any hidden genre, so the
auto-load of extra pages (max 8) also kicks in for hidden genres; `streamCatalogView` now starts that load
itself when too few match (e.g. right after closing Settings). Genre picker leaves hidden genres out
(`sPickOpts`). Addon search hides them too, with "N hidden by genre". Not filtered: your lists, Continue
watching, Watched, person pages (TMDB credits only have genre ids). Titles with no genres always show.
Tested in Playwright with a mock fed with Cinemeta's real genre lists (its catalog pages returned 403
from the workspace on Nov 4; the manifest still loads).

**0.24.0** (Nov 4): Spine downloads mpv on Windows. `spine-core/src/mpvget.rs`: GitHub API
`repos/shinchiro/mpv-winbuild-cmake/releases/latest`, `is_plain_build` picks `mpv-x86_64-<8 digits>-git-<hex>.7z`
(not dev / v3 / aarch64 / i686), `parse_release` needs the asset's `digest` ("sha256:<hex>") or refuses;
`covers::curl_to_file` (curl -o, 30 min max on Windows) -> `tools/mpv-download.7z`, size + sha256 checked,
`unpack` with **sevenz-rust2** (default-features off, `util` only: LZMA2 + BCJ2, checks each file's CRC; py7zr
can't do BCJ2) into `tools/mpv.new`, then swapped for `tools/mpv` (old one moved aside; locked = "mpv is still
open"); `spine-mpv.json` holds the tag. `build_library` now always searches `<data>/tools` first, plus
`default_search_paths(data dir)` so Program Files etc. are searched even with no ROM/movie folders.
App: `mpv_status` {windows, path, ours, tag}, `get_mpv` (Windows only, one at a time, emits `mpv-progress`
lines, then puts the exe into `installed.found`). launch.rs: missing mpv on Windows says
"Get mpv in Settings → Controller & sound." (`install_hint`); streams too. UI: `mpvGroup` in Settings → Controller
& sound (Windows only), `needsMpv(blocked)` adds a Get mpv button on the Movies/TV tile and the detail warning;
after the download the UI reloads `boot` so the warning goes. Tested: real 20261021 archive (34 MB) unpacked
twice by the ignored test (`SPINE_MPV_7Z=<path> cargo test -- --ignored unpacks_a_real_build`), UI in
Playwright with a Windows user agent. Not testable here: the real download on Windows (owner to check).
Workspace note: release pages of other repos: WebFetch `github.com/<o>/<r>/releases/expanded_assets/<tag>`
(the API is blocked for unattached repos); release downloads themselves work with curl.
Owner confirmed (Nov 4): Get mpv downloaded and unpacked correctly on Windows.

**0.25.0** (Nov 4): controller in the mpv player. mpv only starts SDL gamepad input at launch
(`input-gamepad` is read in input init; setting it from Lua does nothing, so the old pad bindings never
worked). `player::MPV_CONF` ("input-gamepad=yes") is written as `spine-input.conf` next to the script by
`player::write_mpv_files(dir)` (both play paths) and passed with `--include=` (after --script-opts): an mpv
without SDL gamepad only warns about the line (checked: unknown options in an included file aren't fatal).
The shinchiro build has `-Dsdl2-gamepad=enabled`; Ubuntu's mpv 0.37 has the option too.
Lua (`spine_player.lua`): `bind_pad` sets `pad = true` (mouse movement clears it) and wakes the controls.
A = `confirm` (menu row / Up next / Skip / pause), B = close menu, cancel Up next, else first press only
shows the controls and a press while they're up quits (progress saved as usual), X = next episode, Y = menu
(`open_menu` focuses the playing audio track; `menu_cols` shared with `draw_menu`, focus `msel`
drawn with an accent outline), Start = pause, D-pad / left stick = `jump(±10)` or volume ±5 (OSD message),
in the menu `menu_move`; LB/RB = `jump(±60)`. `jump` stops 2 s before the end (seeking past the end with
keep-open didn't set eof-reached, so Up next never showed). Hint row (pad only) left-aligned above the seek bar:
"A Pause · B Close video · Y Audio & subtitles · X Next episode · ◀▶ 10 s · LB RB 1 min"; menu and Up next have
their own two hints. Esc keeps its old meaning (menu / Up next / leave fullscreen). Tested in real mpv 0.37 under
xvfb-run with a driver script sending `keypress GAMEPAD_*` and taking `screenshot-to-file ... window`
(needs `XDG_RUNTIME_DIR` set and `--vo=x11`; gpu vo fails there). Not testable here: a physical controller.
Owner confirmed 0.25.0 (Nov 4): controller in the player works well.
**0.26.0** (Nov 4): console icons. Owner asked about KyleBing/retro-game-console-icons (CC BY 4.0, but retro
only: no PS2/Vita/360/PC, and several with brand text); searched alternatives and picked RetroArch's own sets from
libretro/retroarch-assets (CC BY 4.0): **Monochrome** (Kivutar; white controller silhouettes) = default,
**Systematic** (baxysquare; flat top-down console drawings) = option, **Lines** = Spine's old SVGs. Files in
`ui/icons/mono|systematic/<spine id>.png` (256px, pngquant, ~330 KB total), credits in `ui/icons/CREDITS.txt` + README.
Mono pc/dos = "IBM - PC and Compatibles" (Mono's DOS and arcade icons are logos: arcade has no mono file);
Systematic has no PC; ps3 = "(PSN)" variant there. `sysIcon(id, size, set = iconSet())` returns a `<span class=
"sysicon sysimg mono|systematic">` with `--ii:url(...)` when `ICON_SETS[set]` has the id, else `sysLines` (old SVG).
`.sysimg.mono` is a CSS mask filled with currentColor (follows themes like the SVGs); `.systematic` a background
image. Setting `consoleIcons` (mono default / systematic / lines), Settings → Look → Console icons (`ACT.consoleicons`).
Movies/TV/Streaming/Favorites/etc. keep line icons. Masks don't load from file:// in Chromium: test the UI through
`python3 -m http.server` in ui/ (the app serves it from its own origin, so it's fine there). Owner's call (Nov 4):
empty console folders stay hidden (only consoles with games show).
Owner confirmed 0.26.0 (Nov 4): looks much better, happy with Monochrome as default.

**0.26.1** (Nov 4): with no player installed, Movies/TV said "Install VLC." (`launch::plan` kept the *last*
blocked launcher for movies/tv, a leftover from the VLC days). Now every system reports its first launcher,
so Movies/TV say mpv ("Get mpv in Settings → ..." + Get mpv button on Windows, "Install mpv." + Flathub link
on Linux). Test `player_hint_tests::no_player_suggests_mpv`.

**0.27.0** (Nov 4): "Start with your PC" (Settings → Updates & window, chips `as-off`/`as-on`, `ACT.autostart`).
`spine-app/src/autostart.rs`: Windows = HKCU\...\CurrentVersion\Run value "Spine" = quoted exe path (`winreg`,
Windows-only dep, already in Cargo.lock via Tauri); Linux = `~/.config/autostart/spine.desktop` (`linux::desktop_entry`
+ X-GNOME-Autostart-enabled=true; Exec = $APPIMAGE when running as one). The OS setting is the truth: Boot `autostart`
= `autostart::is_on()` (Hidden=true / enabled=false count as off); command `set_autostart(on)` returns the new state.
`autostart::refresh()` at start rewrites the entry if Spine moved. Not settable in the workspace: real Windows registry
(owner to check: Task Manager → Startup apps should list Spine).

**0.28.0** (Nov 4): TV mode, release 1 of 3 (plan: `claude/TV-MODE.md` in the claude.ai Project; mockup
https://claude.ai/artifact/XQNhYkx4VyQEw9T3tPaDpp). Setting `tvMode` (bool, Settings → Look → TV mode, chips `tvm-off/on`,
`ACT.tvmode`); `home()` hands off to `homeTv()` before the Home styles, so `home_style` is kept for when it's off; no home
music in TV mode (`music.js`). Own files `ui/tv.js` + `ui/tv.css`; bundled Lexend (`ui/fonts/Lexend.woff2`, Latin subset
of the variable font via pyftsubset, 38 KB, OFL in `Lexend-OFL.txt`). Own dark look whatever the theme: `--ta` = accent
setting or amber `#F0B45A` when none is set, `--tink` = readable text on it (`tvInk`). State `S.tv` {sec 'play'|'watch',
con, at: {con id: index}}. Play: `tvCons()` = Recent (last_played games, not media) + `tiles()` that hold games (fav,
lists, consoles; movies/tv/stream/random left out), keys `con-<id>` (so Back from a library lands on the tile);
games `tvg-<game id>` (`data-act="tvgame"`: first click picks, A/click on the picked one plays). Layout is a flex column
(top bar, `.tv-hero` takes what's left, `.tv-lower`, hints) so nothing overlaps; `.tv-bg` (banner) is placed against
the whole screen; `@media (max-height: 800px)` makes things smaller (1280x720, or 1080p at 150% scaling). Moving
through games doesn't render: `tvSync` sets the track's translateX (picked card stays in slot 2), toggles `.sel`
(grows via flex-basis/height transitions), lazy-loads covers within ±14 (`data-src`) and swaps `#tvhero`. Card steps
are measured once per render in `tvMount` (`TVSTEP`): measuring during a grow/shrink transition gave off-by-one steps.
Keys (`tvKey`, before the other Home styles in `press`): LT/RT Play⇄Watch, LB/RB console, games row ←/→ step, ↑ console,
X Details; consoles ←/→ change console (render), ↓ games, ↑ banner Play; banner ↓ consoles; Watch: B goes to Play.
Watch (placeholder for release 2): tiles for Movies/TV (`con-movies`/`con-tv`) and Streaming, opening the usual screens
(`ACT.tile`). `focusEl` skips scrollIntoView inside `.tvm`; restoreFocus uses `tvFocusKey()`. Cursor hides with
`body.kb`. Tested in Playwright (1920x1080 and 1280x720, mock) and real WebKitGTK 2.52 under Xvfb (snapshots there can
lag a transition by a frame or two: wait ~2 s before judging). Next: release 2 (Watch screen with the two Continue rows).

**0.28.1** (Nov 4, owner's request after trying 0.28.0 "would be happy to have this on my main tv"): TV mode's dark look on
every screen. `applyTheme()` sets `data-theme="tv"` while `tvOn()` (the saved `theme` setting is untouched and returns when
TV mode is off), accent = setting or `TV_ACCENT`, and `--accent-ink` = `tvInk(accent)` (removed again outside TV mode).
`:root[data-theme="tv"]` in tv.css: same colors as `.tvm` (bg #101218, surface #1a1d26/#262b38, text #f2efe8, muted
#b4b9c5) and Lexend for --display/--body; the two Midnight-only rules in app.css (herowall strip, backdrop opacity) cover
tv too. Settings → Look → Theme note says it's used when TV mode is off. Checked in Playwright: game page, library,
Settings, Streaming.

**0.29.0** (Nov 4): TV mode release 2, the Watch screen (tv.js, after the Play code). Layout like Play: top bar, `#tvwhero`
(banner of the picked Continue card), `.tv-lower` = tiles (`tvWatchTiles`: Movies/TV `con-<id>` → `ACT.tile`; Streaming;
Search `tvw-search` → `TV_ACT.tvsearch` = stream screen with `S_STREAM.tab[k] = 'search:<k>'`, only when an addon can search),
then two rows kept apart: `tvLocalCont()` (last_played movies/tv; shows via `showNext`, fully watched shows and finished
movies drop off; cards use the episode still) and `tvStreamCont()` (`streamContinue('movies'|'tv')` merged by `at`; addon
https pictures, not mediaUrl). State `S.tv.w` {row 'l'|'s', l, s, lk, sk}: each row remembers its card by key (`tvwSet`,
resolved in `tvWatchState`) because the list reorders after playing. `tvWatchSync` slides rows (`TVSTEP.l/.s`), swaps the
banner and the hints (`#tvwhints`: your files A Resume / X Start over / Y Remove; streaming A Open / Y Remove). A focusin
listener makes any focused card (up/down spatial moves, mouse) the row's pick. `TV_ACT.tvcw`: local = `ACT.play` with the
showNext episode (backend resumes; X passes from 'start'); streaming = `ACT.scontinue` + `S.sitem.fromTv`, and `back()`
returns from such an sdetail to the TV home on that card. Y = `TV_ACT.tvforget`: new command **`forget_played(id)`** (main.rs,
removes last_played[id]; resume position kept) or `stream_forget`. Picked card grows only while its row has focus
(`.tv-cw:focus-within`). Tested in Playwright (1920x1080, 1280x720) with mock progress/stream_recent data.
Next: release 3 (TV-sized game/movie pages, soft sounds, start in TV mode full-screen).

**0.29.1** (Nov 4): owner: "with a controller I can't exit whatever row I entered (e.g. Continue watching · your files)".
Not reproducible here (keyboard, simulated gamepad via a fake `navigator.getGamepads` in Playwright, WebKitGTK all moved
fine), so up/down on Watch no longer use spatial `move()`: `tvWatchNav` sends them to fixed places (top → banner main
button → last tile `S.tv.wtile` → row l card → row s card, and back), B in a Continue row → tiles, B elsewhere on Watch →
Play. Play: B in games → consoles, B in consoles → games, top bar down → banner Play. Gamepad test harness: init script
`window.__pad` {buttons[17], axes} + `navigator.getGamepads = () => [__pad]` + `document.hasFocus = () => true`, then
toggle `__pad.buttons[i].pressed` with ~90 ms holds. If the owner still gets stuck: ask Windows or Linux, D-pad or stick.

**0.29.2** (Nov 4): the owner's screenshot (Windows, 1920x1009 window at 125% scaling = 1536x807 CSS px, Auto size =
zoom 1; Demon Slayer logo + long special title) showed the real cause of "stuck in a row": the banner's content was taller
than `.tv-hero`'s room and spilled over the top bar and tiles, putting buttons on top of each other. The 0.28 short layout
was `@media (max-height: 800px)` (807 missed it, and media queries ignore the root zoom anyway). Now `tvFit()` (tv.js,
called in tvMount, after each banner swap and on resize) sets `.tvm.short` when `#tvm.offsetHeight < 840` (rules in
tv.css are `.tvm.short ...`), then steps the banner through `.fit1` (logo 64px / title 32px), `.fit2` (no label line),
`.fit3` (no logo/title) until `.tv-info` fits; `.tv-hero` also clips (`overflow: hidden`). Logo has a fixed height (so
its size is known before it loads); progress/meta text is one line with an ellipsis. Checked overlap numerically in
Playwright at 1536x807@1.25, 1920x1009, 1920x1080, 1280x720, 1366x768, 3840x2160, 1920x1080@1.5 (Play and Watch).
Test at real-world window sizes with device_scale_factor, not just full-screen 1080p.

**0.29.3** (Nov 4): owner's screen recording (Windows): on the Watch tiles, right from Movies did nothing; up/down (already
fixed places since 0.29.1) worked. So `move()` (spatial, getBoundingClientRect) fails in TV mode on his WebView2 setup;
cause not found (works in Chrome/WebKitGTK here). TV mode no longer uses `move()` at all: `tvLinear` = left/right by DOM
order inside `.tv-top`, `.tv-acts` (banner buttons) and `.tv-wtiles`; Play banner up → `tvs-play`, top bar up = edge.
`tvCur()` = activeElement if it's a data-nav in #tvm, else the `.focused` one (re-focused); with no highlight at all a
direction calls restoreFocus. Test that proves it: the gamepad harness with `Element.prototype.getBoundingClientRect`
stubbed to return zeros: every move in Play and Watch still lands. If other screens (game pages, Settings) show the same
"can't move" on his PC, `move()` itself needs a look on WebView2 (zoom + rects).

Owner confirmed 0.29.3 (Nov 4): controller navigation in TV mode works as it should (Play and Watch, at his Windows scaling).

**0.30.0** (Nov 4): TV mode release 3 (owner: skip the new sounds, current ones are fine; rest in one update). TV-sized
pages: `:root[data-theme="tv"]` overrides at the top of tv.css for .topbar, .hints, .detail (padding 72px, cover 360px,
h2 60px, desc 20px, buttons/chips/facts/cats bigger, episode rows with 240px thumbs). Sizes only, same markup, so it
covers detail (games, movies, shows) and Streaming's sdetail/sperson (both use .detail). Setting `tvFullscreen`
(Settings → Look → "Open TV mode full-screen", chips `tvfs-off/on`, `TV_ACT.tvfs`; shown only while TV mode is on):
start() goes full-screen when `fullscreen || (tvOn() && tvFullscreen)` and marks `S.boot.settings.fullscreen` locally
(not saved) so the Window button reads "Leave fullscreen"; switching it on also goes full-screen right away.
TV mode plan (claude/TV-MODE.md) is now complete.

**0.31.0** (Nov 5): Cube redesign release 1, the look (plan: claude/CUBE-REDESIGN.md in the Project; mockup
https://claude.ai/artifact/VoQgZmJBoPtHz6s7xjRuPC, "data lines" frame). cube.js/cube.css rewritten; controls, `S.cube`
state, keys and sounds unchanged. The cube is a wireframe drawn on two canvases covering `#cubehome`: `#cbback` (data dots,
world-fixed floor grid, edges with average z <= 0.05) under the panel, `#cbfront` (nearer edges, corner nodes) over it.
Projection in `cubeProject` (half-size 1, `CUBE_D` 4.284, eye `CUBE_EYE` -0.186, front square = `fs` px: matches the
mockup's 560/348 squares); `cubeTurn` = -90° round Y per `rot`, -90° round X per `tilt`. Glow = canvas shadowBlur
(two strokes per edge: dark wide, bright thin). `cubeAnimate` eases CG.y/CG.x to rot/tilt over 520 ms (back-ease; a new
turn starts from where the frame is); only draws while moving (2.2 ms/frame in software WebKitGTK). `cubeLayout` sizes
from `#cbstage` (fs = min(0.9 h, 0.55 w, 620)), sets `--u` = fs/560; CSS sizes are the mockup's px times `--U` measured
from the frame's middle. Panel = `#cbholo` (`cubeHoloHtml`: ghosts g1/g2, cover behind, panel with scanlines + corner
brackets, cover in front), re-rendered with `.in-r/l/u/d` from `S.cube.dir` when the console changes; `cubeFitName`
shrinks the name to fit (re-run on document.fonts.ready). Floor: `.cb-floor` prev/next (cubepick), tags `.cb-tag`
up/down (cubetilt; when tilted they offer the way back). `.onshelf` dims the panel/frame. Light themes: `.light` class
(bg luminance > 140) makes `--c2` darker and the panel solid. Background art: `cubeArt` draws the first hero/cover into a
48 px canvas, stretches to 384 and box-blurs with shifted copies (WebKitGTK scales canvases WITHOUT smoothing: tiny
canvases stretched by CSS or drawImage look blocky there), two `.cb-art` canvases crossfade. The old dots row is gone
(release 2 adds the console strip). Tested in Playwright (Midnight, Cream, CRT; 1440x860, 1920x1080, 1280x720; clicks,
held turns) and real WebKitGTK 2.52 under Xvfb. Next: release 2 (feel) per CUBE-REDESIGN.md.

Owner confirmed 0.31.0 (Nov 5): "so far so good, i like it".

**0.32.0** (Nov 5): Cube redesign release 2, the feel (cube.js/cube.css). Turns: `cubeAnimate` uses 300 ms instead of
520 ms when a turn comes < 320 ms after the last (`CG.lastTurn`, `CG.dur`), and never lets the frame lag more than one
quarter turn (whole quarter turns of backlog are skipped: the wireframe looks identical after each, so it's invisible;
before this a held button left it ~3 turns behind and it kept spinning after release). Panel slide-in now also scales from
.9 (shrink + turn start together, .3 s, no delay). Warp streaks: `cubeDraw` draws 34 gradient lines on `#cbback` along the
turn axis, moved by `CG.warp` (sum of per-frame movement) with strength from `CG.vy`/`CG.vx` (per-frame velocity); skipped
with reduced motion. Stage strip `#cbstrip` (`cubeStripHtml` / `cubeStripSync`): one `.cb-chip` per belt console
(cubepick), `.on` = console color + count, `.press` animation when the console changes, `.off` while tilted. Shelf rests
12 px low at .55 opacity and slides up (`transform` transition) when `.live`; shelf is 200 px tall so titles aren't cut.
Tested in Playwright (held turns: lag 0 400 ms after release; strip clicks; shelf transform sampled per frame) and
WebKitGTK under Xvfb. Cube redesign plan (claude/CUBE-REDESIGN.md) is now complete.

## Spine Emulators (separate installer), started Nov 6

Owner's choice: a separate program next to Spine, not a button inside it (plan: claude/EMULATOR-INSTALLER.md in the
Project). Windows first, install only (no controller/emulator config), never BIOS/keys/firmware, sources written into the
code (fake Eden download pages with malware exist). Releases: 1 = app + GitHub/Forgejo emulators (0.33.0); 2 = RetroArch with
cores per console + Dolphin, PPSSPP, MAME, Snes9x (non-GitHub sources); 3 = Update button; 4 = Linux via Flathub.

**0.33.0** (Nov 6): release 1. Logic in `spine-core/src/emus.rs` (so its tests run in CI on both OSes): `EMUS` = 13 `EmuDef`
(id = Spine's `launch::EMULATORS` id, folder name, consoles, group, `Source::GitHub(repo)` or `Source::Forgejo(api)`, `pick` =
file-name matcher for the 64-bit Windows build, `exes`). `latest()` asks api.github.com `/releases/latest` (or Eden's Forgejo
`git.eden-emu.dev/api/v1/repos/eden-emu/eden/releases`, newest non-prerelease; its links go to stable.eden-emu.dev, size 0).
Checks: GitHub size, GitHub `digest` sha256 when present (Cemu 2.6's Windows zip has none: older upload), and the archive's
per-file CRCs while unpacking. zip via the `zip` crate (own loop: Azahar's zip uses "\" as folder separator; rejects ".." and
":"), 7z via sevenz-rust2. Unpacks into `<Name>.new`, flattens a single top folder (Cemu_2.6/, azahar-windows-msvc-*/), checks
the program is there, swaps (old copy to `<Name>.old`, "still open" if locked). `spine-emulators.json` in the folder =
{id: {version, file, at}}; rolling tags (DuckStation "latest", Vita3K "continuous") store tag + first 12 hex of the sha256.
Folder: `%USERPROFILE%\Emulators\<Name>` (`emus::default_dir`), added to `launch::default_search_paths`.
File names checked Nov 6: Xenia's is really `xenia_canary_windows_.zip` (trailing underscore). Live test
`cargo test --lib -- --ignored live_downloads` downloads and unpacks melonDS, mGBA, Xenia, Eden, Azahar, PCSX2, Cemu, RPCS3
and checks Spine's `Installed::detect` finds each (the workspace can download release files, not use the GitHub API).
App: `spine-emus/` (Tauri 2, `Spine-Emulators.exe`, identifier app.spine.emulators, no bundle), `src/main.rs` commands `boot`
(EmuView: version/ours/elsewhere via `Installed::detect` over Spine's search paths), `install(ids)` (one at a time in a
thread; events `emu-progress` {text | got/total from watching the file size}, `emu-done`, `emu-finished`), `open_folder`,
`open_site` (explorer.exe with the address from our list). UI `spine-emus/ui/index.html` (single file, TV-mode colors,
groups, checkboxes, progress bars, "Select not installed"). On non-Windows it shows a Flathub note and won't install.
CI: windows job builds it with `tauri build --no-bundle` and publishes Spine-Emulators.exe + .sha256.
Spine: new launchers `flycast` (`-config window:fullscreen=yes {rom}`) and `mesen` (`{rom} --fullscreen`) as Standalone
choices (Dreamcast; NES, SNES, GB, GBC, GBA) — args not tested on a real PC yet. Tested: unit tests, real downloads,
Windows cross-check (`cargo check --target x86_64-pc-windows-gnu`, needs gcc-mingw-w64-x86-64 for the icon resource),
Playwright with a mock, and the real app under Xvfb (boot lists everything).

Age-rating filter: ON HOLD (owner, Nov 4), add only if users ask for it. Agreed shape if it comes back: TMDB
/find by IMDb id + release_dates / content_ratings (US), cached on disk (ratings rarely change), grid waits for
the checks on a page's first load rather than titles vanishing; a setting for titles with no rating (Kitsu ids).

**Old note (Nov 3), done in 0.24.0**: Spine downloads and sets up mpv on Windows. Owner installed it by hand
from shinchiro/mpv-winbuild-cmake GitHub releases: plain `mpv-x86_64-<date>-git-<hash>.7z` (not dev / v3 /
aarch64 / i686 / ffmpeg). Assets are .7z only, so this needs a 7z reader (e.g. the `sevenz-rust` crate) or
another source; check the release's sha256 shown on GitHub, unpack into an `mpv` folder Spine searches.
`ui/stream.js` + `stream.css` and `spine-core/src/addons.rs`; keep new streaming features there.
UI tests: recreate the Playwright mock (`window.__SPINE_MOCK__`; Chrome is at /usr/bin/google-chrome;
Cinemeta JSON can be fetched live from the workspace for test data).

Owner's credit budget is limited: start in a fresh session (cheaper), work in
small releases, and confirm scope before big steps.
