## What's new
- **Streaming filters:** a row of filters above every addon catalog in the Streaming tile.
  - **Genre** (or **Year**, on catalogs like Cinemeta's "New"): the addon sends only those titles, so it covers the whole catalog, not just what's on screen.
  - **Anime only**, **decade** (2020s, 2010s… Before 1980) and **minimum rating** (★ 6+, 7+, 8+): Spine keeps loading more of the catalog until it finds enough matches. "Anime only" is a best guess from genre and country; an anime catalog addon is exact.
  - Filters stay set while you switch catalogs; **Clear filters** resets them.
- Fixed: chosen items in list pickers showed two checkmarks.

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**.

Already have Spine? It updates itself from Settings → Updates.
