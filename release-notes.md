## What's new
- **Spine Emulators: Change folder.** Keep your emulators somewhere else, like `D:\Emulators` on an external drive: press **Change folder** and pick it. Spine Emulators installs and updates there from then on, and Spine searches that folder too.
- **Emulators you put there yourself count.** Any emulator in its own folder inside the emulators folder (for example `D:\Emulators\pcsx2-v2.4\pcsx2-qt.exe`) shows as installed, and its updates go into that same folder. Spine Emulators can't tell which version you installed by hand, so it offers each one an update once; after that it keeps track.
- **Updates keep your files.** Settings, saves, keys and BIOS files inside an emulator's folder now stay when it updates (before, only RetroArch's did).

From 0.37.x:
- Nintendo Switch 2 console: Lichii, Eden-NX2 and Eglantine; .nup files and exefs/romfs game folders; Lichii in Spine Emulators.

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed. Want emulators too? Download **Spine-Emulators.exe** and run it.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**. Want emulators too? Download **Spine-Emulators-x86_64.AppImage** and open it the same way (needs Flatpak, which Mint, Fedora, SteamOS and Bazzite already have).

Already have Spine? It updates itself from Settings → Updates.
