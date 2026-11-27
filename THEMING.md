# How to Make Your Own Spine Theme

Spine themes are simple folders (or `.zip` files) of **JSON, CSS, fonts, images, and audio**—no programming required and no executable scripts, so community themes are always safe to share and install.

Want to try a finished example first? Install Spine's **Stained Glass** example theme in one click:
1. Open **Settings → Look → Manage themes & packs…**
2. Paste this link into **Import from link** and click **Download & import**:
   ```text
   https://raw.githubusercontent.com/paperwolf2350-lab/Spine/main/examples/stained-glass.zip
   ```
3. You can read every file inside it in [`examples/stained-glass/`](examples/stained-glass/).

---

## Path 1: No-Code Customization (2 minutes)

You don't need to touch a single file to create and share your own theme:
1. Go to **Settings → Look → Theme** and click **Customize look**.
2. Pick a curated palette—or customize **Background**, **Cards**, **Text**, and **Accent** with a controller or mouse.
3. Add a **Background wallpaper** (adjust darkness, blur, and the moving cover wall), choose a **Font** (or load your own `.ttf` / `.woff2` file), pick **Corners** (Sharp / Rounded / Soft) and **Card style** (Solid / Frosted glass), and pick your own **Home music** & **Sound style**.
4. Under **Save & share**, type a name and click **Save as theme** (or **Save & Export `.zip`**).

---

## Path 2: Step-by-Step Theme Pack Tutorial (Using *Stained Glass*)

Ready to go further with custom CSS, artwork, custom Settings switches, console icons, sound effects, or a custom mascot? Follow these 6 steps.

### Step 1 — Create your theme folder
1. Open **Settings → Look → Manage themes & packs…**
2. Click **Create starter theme**. Spine creates an editable folder and activates it right away.
3. Click **Open themes folder** to see your files:
   - **Windows (portable):** `<Spine folder>\themes\<your-theme>\`
   - **Linux:** `~/.local/share/spine/themes/<your-theme>/`

Whenever you save an edit to any file, click **Reload themes** in Settings to see it live.

> **Safety net:** If custom CSS ever hides a button or breaks the layout, press **`Ctrl + Shift + R`** anywhere in Spine to immediately reset to the default theme.

---

### Step 2 — Set up `theme.json`
`theme.json` tells Spine your theme's name, base palette, colors, and any custom toggles you want to show in Settings. Here is how [`examples/stained-glass/theme.json`](examples/stained-glass/theme.json) is put together:

```json
{
  "name": "Stained Glass",
  "author": "Your Name",
  "version": "1.0.0",
  "description": "Cathedral jewel-tone glass panes framed in dark leadwork.",
  "base": "midnight",
  "preview": "preview.svg",
  "colors": {
    "bg": "#0A0B14",
    "surface": "#141626",
    "surface2": "#1D2036",
    "text": "#F6EFE2",
    "muted": "#B6AEC4",
    "line": "#2D304D",
    "accent": "#E6A83A",
    "focus": "#F6EFE2",
    "display": "Georgia, serif",
    "body": "'Inter', system-ui, sans-serif"
  },
  "style": {
    "radius": "rounded",
    "wallpaperDim": 0.32,
    "wallpaperBlur": 2,
    "coverWall": false
  }
}
```

* **`base`**: Which built-in Spine theme to start from (`"cream"`, `"midnight"`, `"crt"`, `"glass-light"`, `"glass-dark"`, or `"tv"`).
* **`preview`**: Optional thumbnail image (`preview.png`, `preview.jpg`, `preview.webp`, or `preview.svg`) shown on your theme tile in Settings.
* **`colors`**: Every entry becomes a CSS variable on `:root` (`"bg"` → `--bg`, `"accent"` → `--accent`, etc.).
* **`style`**: Optional built-in helpers: `radius` (`"sharp"`, `"rounded"`, `"soft"`), `glass` (`true`/`false`), `wallpaperDim` (`0.0`–`0.95`), `wallpaperBlur` (`0`–`40`), `coverWall` (`true`/`false`), and `soundStyle` (`"default"`, `"blades"`, `"cube"`).

---

### Step 3 — Add a background wallpaper (overall or per-console)
Drop any `.jpg`, `.png`, `.webp`, `.gif`, or `.svg` into a `wallpapers/` folder inside your theme:
* `wallpapers/default.svg` (or `default.jpg`) — shown behind Spine everywhere (like Stained Glass's mosaic in [`examples/stained-glass/wallpapers/default.svg`](examples/stained-glass/wallpapers/default.svg)).
* `wallpapers/snes.jpg`, `wallpapers/ps2.png`, `wallpapers/gc.webp`, `wallpapers/switch.jpg`, `wallpapers/movies.jpg` — swaps automatically whenever that console is highlighted or open!

---

### Step 4 — Style Spine in `theme.css`
`theme.css` loads **after** Spine's built-in stylesheets, so you can restyle any screen. For example, in [`examples/stained-glass/theme.css`](examples/stained-glass/theme.css):

```css
/* Make console tiles look like backlit stained-glass panes */
.console {
  border-radius: 16px 16px 12px 12px;
  background: color-mix(in srgb, var(--chip) 74%, #090A12) !important;
  border: 3px solid #07080F;
  box-shadow: inset 0 0 0 1px rgba(255, 232, 185, 0.28), 0 12px 26px rgba(0, 0, 0, 0.5);
}

/* Style TV mode specifically using :root[data-tv="on"] */
:root[data-tv="on"] .tv-con {
  backdrop-filter: blur(18px);
}
```

**Helpful tips:**
* **Relative URLs work automatically:** `url('fonts/MyFont.woff2')` and `url('wallpapers/bg.jpg')` resolve inside your theme folder.
* **Avoid `100vh` / `100vw`:** Spine scales the UI with root `zoom`, which handles `vh`/`vw` differently between Windows and Linux. Use `inset: 0` or `%` instead.

---

### Step 5 — Add your own Settings toggles (`options`)
Want users of your theme to switch color moods or turn effects on/off without editing CSS? Declare `options` in `theme.json`:

```json
"options": [
  {
    "id": "mood",
    "label": "Color mood",
    "note": "Changes the jewel-tone lighting.",
    "type": "choice",
    "choices": [
      { "value": "cathedral", "label": "Cathedral" },
      { "value": "moonlight", "label": "Moonlight" },
      { "value": "sunburst", "label": "Sunburst" }
    ],
    "default": "cathedral"
  },
  {
    "id": "lead",
    "label": "Lead framing",
    "type": "toggle",
    "default": true
  }
]
```

Spine automatically displays those controls in **Settings → Look** when your theme is active, and sets `data-opt-<id>` attributes on `:root` so you can style them in `theme.css`:

```css
:root[data-opt-mood="moonlight"] {
  --accent: #7EB6FF;
}
:root[data-opt-lead="on"] .console {
  border: 3px solid #07080F;
}
```

---

### Step 6 — Export & share!
Open **Settings → Look → Manage themes & packs…** and click **Export** on your theme card. Spine zips it up cleanly so anyone can install it via **Import theme / pack (.zip)** or **Import from link** (if you host it on GitHub, GitLab, or Codeberg).

---

## Spine CSS Cheat Sheet

Use these main selectors in `theme.css` so you don't have to hunt through Spine's source files:

| What you want to style | Selector |
|---|---|
| Core color & font variables | `:root` (`--bg`, `--surface`, `--surface2`, `--text`, `--muted`, `--line`, `--accent`, `--accent-ink`, `--focus`, `--display`, `--body`) |
| Top bar, search box, settings button | `.topbar`, `.brand`, `.searchbox`, `.clock`, `.iconbtn` |
| Bottom button hint bar | `.hints` |
| Home hero & console cards | `.hero`, `.hero-text h2`, `.herowall`, `.console`, `.console.sel` |
| Continue playing / watching cards | `.recent .item` |
| Game cover cards & A–Z rail | `.cover`, `.tiles .game`, `.rail`, `.gamesbar` |
| Game / Movie / Show detail page | `.detail`, `.detail h2`, `.fact`, `.ep`, `.cat` |
| Buttons, chips & segmented bars | `.btn`, `.btn.primary`, `.chip`, `.chip.on`, `.seg`, `.set-seg` |
| Keyboard / controller focus ring | `body.kb [data-nav].focused, body.kb [data-nav]:focus-visible` |
| Settings modal & Customize drawer | `.set-win`, `.set-nav`, `.set-tab.on`, `.set-card`, `.panel .sheet` |
| TV mode screens & pills | `:root[data-tv="on"]`, `.tvm`, `.tv-con`, `.tv-game`, `.tv-btn`, `.tv-switch` |
| Background wallpaper layer | `#theme-bg .tbg-img`, `#theme-bg .tbg-scrim` |
| Toast notifications | `.toast` |

---

## Optional Asset Folders

You can include any combination of these folders inside your theme—or make a standalone pack with **only** one of them (for example, a standalone Console Icon Pack or Mascot Pack):

```text
themes/
  my-cool-theme/
    theme.json
    theme.css
    preview.png         # Optional thumbnail (png, jpg, webp, svg)
    fonts/              # .woff2, .woff, .ttf, .otf (auto-registered as @font-face)
    wallpapers/         # default.<ext> + per-console wallpapers (snes.jpg, ps2.png, ...)
    icons/              # Per-console icons (snes.png, ps2.svg, favorites.png, ...)
    music/              # default.mp3 + per-style (classic, wave, channels, blades, cube, tv)
    sounds/             # UI sound effects (move, tick, edge, select, back, open, favOn, favOff, spin, launch, welcome, bladeR, bladeL, cubeR, cubeL)
    mascot/             # Full custom mascot replacement (mascot.json + pose images)
```

### `fonts/`
Drop `.woff2`, `.woff`, `.ttf`, or `.otf` files here. Spine automatically registers each file as `@font-face` using the filename (without extension)—for example, `fonts/Arcade.woff2` can be used right away as `'Arcade'` in `colors.display` or `theme.css`.

### `icons/`
Supported formats: `.png`, `.svg`, `.webp`, `.jpg`. Set `"iconMode": "mono"` in `theme.json` if your icons are single-color silhouettes that should tint with the text color (default `"color"` keeps original colors).
Console IDs: `nes`, `fds`, `snes`, `n64`, `gb`, `gbc`, `gba`, `nds`, `n3ds`, `virtualboy`, `gc`, `wii`, `wiiu`, `switch`, `switch2`, `mastersystem`, `genesis`, `gamegear`, `segacd`, `sega32x`, `saturn`, `dreamcast`, `psx`, `ps2`, `ps3`, `psp`, `psvita`, `pcengine`, `pcenginecd`, `neogeo`, `ngpc`, `arcade`, `atari2600`, `atarilynx`, `wonderswan`, `dos`, `xbox`, `xbox360`, `pc`, `movies`, `tv`, `streaming`, `favorites`, `random`, `collection`, `recent`. Common aliases (`ps1`, `gamecube`, `megadrive`, `famicom`, etc.) work automatically too.

### `music/` & `sounds/`
Supported formats: `.mp3`, `.ogg`, `.wav`, `.m4a`.
* `music/default.<ext>` plays on Home for any style without its own file; add `classic`, `wave`, `channels`, `blades`, `cube`, or `tv` for per-style songs.
* Any event file in `sounds/` overrides Spine's built-in sound; omitted events keep Spine's defaults.

### Custom Mascot Packs (`mascot/`)
A custom mascot pack completely replaces Paige—her name, poses, blink frames, and dialogue lines. Put `mascot.json` and your pose images (`.webp`, `.png`, `.gif`, or `.svg`) inside `mascot/`:

```json
{
  "name": "Nova",
  "poses": {
    "sit_wave": { "file": "sit_wave.webp", "blink": "sit_wave_blink.webp", "ar": 0.45, "face": [0.5, 0.14], "seat": 0.55 },
    "cheer":    { "file": "cheer.webp", "ar": 0.42, "face": [0.5, 0.18] },
    "doze":     { "file": "doze.webp",  "ar": 0.90, "face": [0.55, 0.2] },
    "bye":      { "file": "bye.webp",   "ar": 0.50, "face": [0.48, 0.1] }
  },
  "lines": {
    "first_hi":  ["Hi! I'm {name}.", "Ready to jump into your library?"],
    "morning":   ["Good morning! Ready to play?"],
    "afternoon": ["Good afternoon! What are we playing today?"],
    "evening":   ["Good evening! Time to unwind."],
    "late_night":["Up late at {time}? Just one more level."],
    "surprise":  ["How about {title}?", "{name}'s pick: {title}!"],
    "hang":      ["Great collection!", "Just hanging out."],
    "idle_read": ["Take your time choosing."],
    "idle_doze": ["Zzz..."],
    "bye":       ["See you next time!", "Catch you later!"],
    "bye_night": ["Good night!", "Get some rest!"]
  }
}
```

* **`ar`**: Image width ÷ height. **`face`**: `[x, y]` (`0.0`–`1.0`) where the speech bubble tail points. **`seat`**: fraction of height where `sit_wave` sits on a bar/tile.
* Supported pose names: `sit_wave`, `peek`, `point`, `think`, `cheer`, `doze`, `surprise`, `hang`, `flustered`, `lean`, `lie`, `bye`, `thumbs_up`, `peek_side`, `read`. You don't have to draw all 15—undefined poses gracefully reuse your own defined poses.
* Placeholders you can use in `lines`: `{name}`, `{title}`, `{time}`, `{days}`, `{hours}`.

---

## Importing from ES-DE, Daijishō & RetroArch

You can also bring in compatible art, colors, fonts, icons, music, and sounds from other game launcher ecosystems—no manual renaming needed:

* **Repository or `.zip` links:** Under **Settings → Look → Manage themes & packs…**, paste any **GitHub**, **GitLab**, **`git.es-de.org`**, **Codeberg**, or direct `.zip` link into **Import from link** and click **Download & import**.
* **`.zip` packs:** Click **Import theme / pack (.zip)**. Spine automatically detects whether the `.zip` is a native Spine theme, an **ES-DE / EmulationStation theme**, a **Daijishō wallpaper pack**, or a **RetroArch console icon pack**.
* **Unzipped folders:** Click **Import external folder** and select any ES-DE theme folder, Daijishō wallpaper folder, or RetroArch icon folder on your PC.

Everything is converted into a clean, standard Spine theme pack inside your `themes/` folder, so you can further tweak it in **Customize look**, mix and match its icons/music/sounds with other themes, or export it as a `.zip`.

---

## Packaging & Security Rules

* Zip either the theme files directly OR a single top-level folder containing them—Spine's importer handles both.
* Only safe static file types are accepted (`.json`, `.css`, `.txt`, `.md`, images, fonts, and audio). Scripts (`.js`, `.html`) and executables are always rejected on import and never served by Spine.
