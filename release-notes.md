## What's new
- Linux fix: Spine's AppImage no longer passes its own internal settings to the programs it starts. That stopped updates ("can't reach GitHub") and made emulators close right away on some systems, like Linux Mint.
- Linux: downloads use Spine's built-in downloader, so they work even where the system's curl doesn't.
- If an emulator closes right away, Spine shows what it reported (since 0.4.4).

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**.

Already have Spine? It updates itself from Settings → Updates.
