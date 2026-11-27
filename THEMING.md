# Making Themes for Spine

Spine themes are plain folders (or `.zip` archives) of **CSS, JSON, fonts, images, and audio**—no executable scripts, so community themes are always safe to share and install.

You can make:
- A **full theme** (custom colors, CSS styling, fonts, wallpapers, console icons, music, sounds, and even a custom mascot)
- A **standalone pack** (for example, just a Console Icon Pack, a Music Pack, a Sound Effects Pack, or a Custom Mascot Pack)

Users can mix and match sub-packs (icons, music, sounds, mascot) on top of any theme, and pick separate themes for **Desktop** and **TV mode**.

---

## Quick Start

### Simple Mode (no files or code needed)
1. Open Spine and go to **Settings → Look → Theme → Customize look**.
2. Pick a curated color palette—or customize Background, Cards, Text, and Accent with controller-friendly swatches, color pickers, or hex codes.
3. Add a background wallpaper (with blur, darkness, and cover-wall controls), switch fonts or load your own `.ttf` / `.woff2` font file, choose Sharp / Rounded / Soft corners or Frosted glass cards, and pick your own Home music & UI sound style.
4. Type a name under **Save & share** and click **Save as theme** (or **Save & Export `.zip`**) to turn your look into a real, shareable Spine theme pack.

### Advanced Mode (full CSS & asset packs)
1. Open Spine and go to **Settings → Look → Theme**.
2. Click **Create starter theme**. Spine creates a ready-to-edit folder inside your `themes/` folder and activates it.
3. Click **Open themes folder** to view the files:
   - **Windows (portable):** `<Spine folder>\themes\<theme-id>\`
   - **Linux:** `~/.local/share/spine/themes/<theme-id>/`
4. Edit `theme.json` or `theme.css`, then click **Reload themes** in Settings to see your changes immediately.
5. Click **Export** on your theme card to package it as a shareable `.zip`.

> **Safety net:** If custom colors or CSS ever hide a button or make text unreadable, press **`Ctrl + Shift + R`** anywhere in Spine to immediately reset back to the default theme.

---

## Folder Structure

Every folder inside `themes/` is a theme pack. Everything except `theme.json` (or `theme.css` / `mascot/mascot.json`) is optional:

```text
themes/
  my-cool-theme/
    theme.json          # Metadata, colors, base palette, and optional Settings toggles
    theme.css           # Full custom stylesheet (overrides any part of Spine)
    preview.png         # Optional thumbnail shown in Settings (png, jpg, or webp)
    fonts/              # .woff2, .woff, .ttf, .otf (auto-registered as @font-face)
    wallpapers/         # default.jpg + per-console wallpapers (snes.jpg, ps2.png, ...)
    icons/              # Per-console icons (snes.png, ps2.svg, favorites.png, ...)
    music/              # default.mp3 + per-style songs (classic, wave, channels, blades, cube, tv)
    sounds/             # UI sound effects (move.wav, select.wav, back.wav, launch.ogg, ...)
    mascot/             # Full custom mascot replacement (mascot.json + pose images)
```

---

## `theme.json` Reference

```json
{
  "name": "Neon Arcade",
  "author": "Your Name",
  "version": "1.0.0",
  "description": "Dark arcade cabinet look with glowing focus rings.",
  "base": "midnight",
  "preview": "preview.png",
  "iconMode": "color",
  "colors": {
    "bg": "#0D0E16",
    "surface": "#171928",
    "surface2": "#21253B",
    "text": "#F5F7FF",
    "muted": "#9AA3C2",
    "line": "#2D3250",
    "accent": "#FF3B8B",
    "focus": "#F5F7FF",
    "display": "'Orbitron', sans-serif",
    "body": "'Inter', sans-serif"
  },
  "style": {
    "radius": "rounded",
    "glass": true,
    "wallpaperDim": 0.45,
    "wallpaperBlur": 4,
    "coverWall": true
  },
  "options": [
    {
      "id": "scanlines",
      "label": "CRT scanlines",
      "note": "Adds subtle scanlines over the whole window.",
      "type": "toggle",
      "default": true
    },
    {
      "id": "glow",
      "label": "Glow intensity",
      "type": "choice",
      "choices": [
        { "value": "subtle", "label": "Subtle" },
        { "value": "bold", "label": "Bold" }
      ],
      "default": "subtle"
    }
  ]
}
```

### Fields
- **`base`**: Built-in palette loaded underneath your theme: `"cream"`, `"midnight"`, `"crt"`, or `"tv"`.
- **`iconMode`**: `"color"` (default, keeps your icon images' original colors) or `"mono"` (uses image alpha as a mask tinted with the text color, like Spine's Monochrome icons).
- **`colors`**: Sets CSS custom properties on `:root` (`"bg"` → `--bg`, `"accent"` → `--accent`, etc.).
- **`style`**: Optional built-in visual helpers:
  - `radius`: `"sharp"`, `"rounded"`, or `"soft"`
  - `glass`: `true` / `false` (frosted translucent cards & panels)
  - `wallpaperDim`: `0.0` to `0.95` (darkness scrim over `wallpapers/`)
  - `wallpaperBlur`: `0` to `40` (blur in px on `wallpapers/`)
  - `coverWall`: `true` / `false` (show/hide Classic Home's moving cover wall)
- **`options`**: Custom toggles (`"type": "toggle"`) or multi-choice switches (`"type": "choice"`) that automatically appear in **Settings → Look** whenever your theme is active!

---

## Writing `theme.css`

`theme.css` loads **last**, so you can style any screen, Home style, or TV mode element.

- **Relative URLs work automatically:** `url('fonts/MyFont.woff2')` and `url('wallpapers/bg.jpg')` resolve inside your theme folder.
- **Custom Settings options become attributes on `:root`:**
  - A toggle with `"id": "scanlines"` sets `:root[data-opt-scanlines="on"]` or `"off"`.
  - A choice with `"id": "glow"` sets `:root[data-opt-glow="subtle"]` or `"bold"`.
- **Desktop vs. TV mode targeting:**
  - TV mode sets `:root[data-tv="on"]`, so you can write TV-specific rules without breaking Desktop (and vice versa with `:root:not([data-tv="on"])`).
- **Key CSS variables used across Spine:**
  - `--bg`, `--surface`, `--surface2`, `--text`, `--muted`, `--line`, `--accent`, `--accent-ink`, `--focus`, `--shadow`, `--display`, `--body`
  - TV mode also uses `--tbg`, `--tsurf`, `--tsurf2`, `--ttext`, `--tmuted`, `--ta`, `--tink` (which inherit from `--bg`, `--surface`, etc. by default).
- **Important layout tip:** Avoid `100vh`/`100vw` because Spine scales the interface with root `zoom` (WebKitGTK on Linux and WebView2 on Windows handle `vh`/`vw` differently under zoom). Use `inset: 0` or `%` instead.

---

## Asset Folders

### `fonts/`
Drop `.woff2`, `.woff`, `.ttf`, or `.otf` files here. Spine automatically registers each file as a `@font-face` using the filename (without extension) as the font family name—for example, `fonts/Arcade.woff2` can immediately be used as `'Arcade'` in `colors.display` or `theme.css`.

### `wallpapers/`
Supported formats: `.jpg`, `.jpeg`, `.png`, `.webp`, `.gif`.
- `default.<ext>` (or `background.<ext>` / `bg.<ext>`) — shown across the app.
- `<console-id>.<ext>` — shown whenever that console is selected or open (for example: `snes.jpg`, `ps2.png`, `gc.webp`, `switch.jpg`, `movies.jpg`, `tv.jpg`, `streaming.jpg`). Common aliases like `gamecube`, `megadrive`, and `ps1` also work automatically.

### `icons/`
Supported formats: `.png`, `.svg`, `.webp`, `.jpg`.
- Name files by Spine console ID (`nes`, `snes`, `n64`, `gb`, `gbc`, `gba`, `nds`, `n3ds`, `gc`, `wii`, `wiiu`, `switch`, `switch2`, `genesis`, `dreamcast`, `saturn`, `psx`, `ps2`, `ps3`, `psp`, `psvita`, `xbox`, `xbox360`, `arcade`, `pc`, `movies`, `tv`, `streaming`, `favorites`, `random`, `collection`, `recent`).
- Common folder aliases (`ps1`, `gamecube`, `megadrive`, `famicom`, etc.) are automatically matched too.

### `music/`
Supported formats: `.mp3`, `.ogg`, `.wav`, `.m4a`.
- `default.<ext>` — plays on Home for any Home style that doesn't have its own track.
- Per-style tracks: `classic.<ext>`, `wave.<ext>`, `channels.<ext>`, `blades.<ext>`, `cube.<ext>`, `tv.<ext>`.

### `sounds/`
Supported formats: `.wav`, `.mp3`, `.ogg`, `.m4a`.
Any sound event you include overrides Spine's synthesized sound (events you omit fall back to Spine's default):
- `move`, `tick`, `edge`, `select`, `back`, `open`, `favOn`, `favOff`, `spin`, `launch`, `welcome`, `bladeR`, `bladeL`, `cubeR`, `cubeL`.

---

## Custom Mascot Packs (`mascot/`)

A custom mascot pack completely replaces Paige—her name, poses, blink frames, and dialogue lines. Put a `mascot.json` and your pose images (`.webp`, `.png`, `.gif`, or `.svg`) inside `mascot/`:

```json
{
  "name": "Nova",
  "poses": {
    "sit_wave": {
      "file": "sit_wave.webp",
      "blink": "sit_wave_blink.webp",
      "ar": 0.45,
      "face": [0.5, 0.14],
      "seat": 0.55
    },
    "cheer": { "file": "cheer.webp", "ar": 0.42, "face": [0.5, 0.18] },
    "doze":  { "file": "doze.webp",  "ar": 0.90, "face": [0.55, 0.2] },
    "bye":   { "file": "bye.webp",   "ar": 0.50, "face": [0.48, 0.1] }
  },
  "lines": {
    "first_hi": ["Hi! I'm {name}.", "Ready to jump into your library?"],
    "morning": ["Good morning! Ready to play?"],
    "afternoon": ["Good afternoon! What are we playing today?"],
    "evening": ["Good evening! Time to unwind."],
    "late_night": ["Up late at {time}? Just one more level."],
    "surprise": ["How about {title}?", "{name}'s pick: {title}!"],
    "hang": ["Great collection!", "Just hanging out."],
    "idle_read": ["Take your time choosing."],
    "idle_doze": ["Zzz..."],
    "bye": ["See you next time!", "Catch you later!"],
    "bye_night": ["Good night!", "Get some rest!"]
  }
}
```

### Pose fields
- **`file`**: Image filename inside `mascot/`.
- **`blink`** *(optional)*: Eyes-closed image shown briefly every few seconds.
- **`ar`**: Image aspect ratio (`width / height`), used to size and anchor the character cleanly.
- **`face`**: `[x, y]` from `0.0` to `1.0` where the speech bubble tail should point.
- **`seat`** *(optional, for `sit_wave`)*: Fraction of the image height where the character sits on a tile/bar (default `0.55`).
- **Pose names Spine uses:** `sit_wave`, `peek`, `point`, `think`, `cheer`, `doze`, `surprise`, `hang`, `flustered`, `lean`, `lie`, `bye`, `thumbs_up`, `peek_side`, `read`. You don't have to draw all 15—any moment asking for a pose you didn't define automatically uses one of your own defined poses instead.

### Dialogue placeholders
You can use `{name}`, `{title}`, `{time}`, `{days}`, and `{hours}` inside custom lines.

---

## Importing from ES-DE, Daijishō & RetroArch

You can also bring in compatible art, colors, fonts, icons, music, and sounds from other game launcher ecosystems—no manual renaming needed:

- **Repository or `.zip` links:** Under **Settings → Look → Manage themes & packs…**, paste any **GitHub**, **GitLab**, **`git.es-de.org`**, **Codeberg**, or direct `.zip` link into **Import from link** and click **Download & import**.
- **`.zip` packs:** Click **Import theme / pack (.zip)**. Spine automatically detects whether the `.zip` is a native Spine theme, an **ES-DE / EmulationStation theme**, a **Daijishō wallpaper pack**, or a **RetroArch console icon pack**.
- **Unzipped folders:** Click **Import external folder** and select any ES-DE theme folder, Daijishō wallpaper folder, or RetroArch icon folder on your PC.

What Spine imports automatically:
- **ES-DE / Batocera / RetroBat themes:** Theme name, palette colors from `theme.xml` / `capabilities.xml`, per-console and default wallpapers, console logos/icons, bundled fonts, background music, and UI sound effects.
- **Daijishō wallpaper packs:** Pack name/author and every console wallpaper from `index.json` (or by filename).
- **RetroArch icon packs:** Console icons (automatically skipping `-content` cartridge variants and utility icons, and enabling monochrome tinting for Monochrome packs).

Everything is converted into a clean, standard Spine theme pack inside your `themes/` folder, so you can further tweak it in **Customize look**, mix and match its icons/music/sounds with other themes, or export it as a `.zip`.

---

## Packaging & Security Rules

- Zip either the theme files directly OR a single top-level folder containing them—Spine's **Import theme (.zip)** handles both.
- Only safe static file types are accepted (`.json`, `.css`, `.txt`, `.md`, images, fonts, and audio). Scripts (`.js`, `.html`) and executables are always rejected on import and never served by Spine.
