## What's new
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
