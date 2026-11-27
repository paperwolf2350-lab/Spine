## What's new
- **Import themes directly from a link.** In **Settings → Look → Manage themes & packs…**, paste any **GitHub**, **GitLab**, **`git.es-de.org`**, **Codeberg**, or direct `.zip` link and click **Download & import**—no need to hunt for release zips.
- **Better ES-DE console artwork importing & display.** Spine now recognizes ES-DE `artwork/`, `art/`, `images/`, `hero/` and `systems/` folders (such as *Art Book Next* and *Artflix Revisited*), picks the highest-resolution art per console, raises the archive limit for large multi-system repos, and lets per-console wallpapers shine cleanly behind Home instead of being covered by the moving cover wall.

From 0.43.1:
- **Redesigned Settings menu.** Settings now opens as a cleaner, centered two-column window with an icon sidebar on the left (`LB / RB` to switch sections), grouped cards with inline toggle switches and compact segmented controls, subtle custom scrollbars, and a controller hint bar along the bottom.
- **Visual theme & Home style previews.** Built-in and custom themes now show miniature color preview tiles, Home styles show crisp icons, and theme pack import/creator tools live neatly inside a dedicated **Manage themes & packs…** sub-page so the main Look screen stays uncluttered. **Customize look** stays as a slim side drawer so you can still watch your Home screen update live behind it.

From 0.43.0:
- **Customize Spine's look (Simple Mode).** Go to **Settings → Look → Theme → Customize look** to pick from 8 curated color palettes or choose your own Background, Cards, Text, and Accent colors with a controller or mouse. Add a custom wallpaper (with darkness, blur, and moving cover-wall toggles), switch fonts or load your own font file, pick sharp/rounded/soft corners or frosted-glass cards, and choose your own Home music and UI sound style. Save your look as a reusable theme or export it as a `.zip` to share.
- **Custom Theme Packs (Advanced Mode).** Drop theme folders into Spine's `themes/` folder—or import `.zip` packs in Settings—to restyle Spine with custom CSS, colors, fonts, overall & per-console wallpapers, console icon packs, Home music, UI sound effects, creator-defined Settings toggles, and full custom mascot packs (`mascot/mascot.json`). Themes are CSS + assets only (no scripts), and pressing **Ctrl + Shift + R** resets back to default anytime. See `THEMING.md` for the full creator guide.
- **Separate Desktop & TV mode themes + Mix & Match.** Pick one theme for Desktop and a different theme (or TV Dark) for TV mode, and mix and match icon packs, Home music, sound effects, or mascots across packs.
- **Import from ES-DE, Daijishō & RetroArch.** **Import theme / pack (.zip)** and **Import external folder** automatically pull compatible console wallpapers, console icons/logos, colors, fonts, music, and sounds from ES-DE themes, Daijishō wallpaper packs, and RetroArch icon packs into standard Spine themes.

From 0.42.2:
- Fixed: on the Streaming screens, "Show more" sometimes added just one title per press. This happened when you had hidden genres or turned on a filter, because Spine fetched one page from the addon and most of it got filtered out. Now it keeps fetching until a good batch of new titles is actually showing (up to 8 pages per press).

From 0.42.1:
- **One search on the Streaming screens.** The big search box at the top now searches your streaming addons for movies and shows (it used to search your games, which was easy to mix up with the small Search tab). It's in Streaming's color so it stands out. Press X, click it, or just start typing.
- Streaming search uses the same pop-up keyboard and searches Movies and Shows together as you type. Suggestions come from titles you've watched, saved or browsed. Pick a result and Back takes you to the Search tab with your results still there.
- TV mode's Watch → Search opens the same search.

From 0.42.0:
- **A pop-up keyboard for search.** Press X (or the Search button) with your controller and a keyboard slides in next to the results, so you never need to reach for a real keyboard. Move with the D-pad or stick: A types, B deletes (or closes when the box is empty), Y adds a space, START jumps to the results. Letters are in ABC order; the "QWE" key switches to a QWERTY layout and Spine remembers your pick. With a mouse, click the Keyboard button to show or hide it.
- **Title suggestions as you type.** Type "hyr" and Hyrule Warriors shows up under the keyboard (or above the results when you type on a real keyboard). Pick one to fill it in and jump straight to the game.
- **Smarter search.** Accents and punctuation don't matter any more ("pokemon" finds Pokémon), words can be in any order ("zelda ocarina" finds The Legend of Zelda: Ocarina of Time), and the best matches come first.
- Fixed: starting a search by typing on a real keyboard could put the next letters in front of the first one.

From 0.41.x:
- Paige visits every Home style (Wave, Channels, Blades, Cube and TV mode each have their own spots).

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed. Want emulators too? Download **Spine-Emulators.exe** and run it.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**. Want emulators too? Download **Spine-Emulators-x86_64.AppImage** and open it the same way (needs Flatpak, which Mint, Fedora, SteamOS and Bazzite already have).

Already have Spine? It updates itself from Settings → Updates.
