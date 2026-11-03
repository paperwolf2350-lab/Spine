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

## Where things stand (Nov 2026, v0.7.6)

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

**Next session**: ask how AniSkip does on the owner's anime, then the Windows mpv download (or what's next)., then the mpv download on Windows (or 0.19 intro skip). Streaming code lives in
`ui/stream.js` + `stream.css` and `spine-core/src/addons.rs`; keep new streaming features there.
UI tests: recreate the Playwright mock (`window.__SPINE_MOCK__`; Chrome is at /usr/bin/google-chrome;
Cinemeta JSON can be fetched live from the workspace for test data).

Owner's credit budget is limited: start in a fresh session (cheaper), work in
small releases, and confirm scope before big steps.
