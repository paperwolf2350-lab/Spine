## What's new
- **Nintendo Switch 2 as its own console.** Put Switch 2 games in a folder called **Switch 2** (or NS2 / NX2) and they get their own tile, separate from your Switch games.
- Spine finds the new Switch 2 emulators: **Lichii** (tried first), **Eden-NX2** and **Eglantine**. Eden-NX2's program is called eden.exe like normal Eden, so keep it in a folder with "NX2" in its name (for example `Emulators\Eden-NX2`): then Spine knows which is which, and your Switch games keep opening in normal Eden.
- Switch 2 games need prod.keys and firmware from your own console, in the same folder Eden uses. Spine tells you where if they're missing.
- **Spine Emulators can install and update Lichii** on Windows, straight from its GitHub releases. These are very experimental test builds: expect crashes and graphics problems. On Linux it's greyed out with a link (no Flathub version).
- Get these emulators only from their own pages. Look-alike download sites for Switch 2 emulators exist, and one was reported to spread malware.

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed. Want emulators too? Download **Spine-Emulators.exe** and run it.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**. Want emulators too? Download **Spine-Emulators-x86_64.AppImage** and open it the same way (needs Flatpak, which Mint, Fedora, SteamOS and Bazzite already have).

Already have Spine? It updates itself from Settings → Updates.
