## What's new
- **TV mode: left and right work everywhere with a controller.** On the Watch tiles (Movies, TV, Streaming, Search), the banner's buttons and the top bar, left and right now go to the next button in line. Before, Spine looked for "the nearest button on screen", which on some Windows setups found nothing, so you couldn't get from Movies to TV, Streaming or Search.

From 0.29.2:
- **TV mode fits your screen.** With Windows display scaling, a window with a title bar, or a show with a big logo and a long episode name, the banner at the top could spill over the top bar and the tiles, leaving the controller on buttons hidden under other buttons. Spine now measures the room it has and shrinks the logo and title (and drops the small label line if needed) so nothing overlaps.

From 0.29.1:
- **TV mode: easier to move around with a controller.** Up and down now always go to the next row: top bar → banner → tiles → Continue watching · your files → Continue watching · streaming, and back up the same way. **B** backs out of a row (a Continue row goes back to the tiles; the games row goes back to the consoles), and B on the Watch tiles goes to Play.

From 0.29.0:
- **TV mode: the Watch screen.** Press RT (or pick Watch at the top) to see what you're in the middle of:
  - **Continue watching · your files**: your own movies and shows, with a progress bar. A resumes right where you stopped (or starts the next episode), X starts over, Y takes it off the row.
  - **Continue watching · streaming**: titles from your addons. A opens the title's page with Resume ready, Y takes it off the row.
  - The highlighted title fills the top with its picture, how much is left and Resume / Episodes buttons.
  - Tiles for **Movies**, **TV**, **Streaming** and **Search** (straight to searching your addons).

From 0.28.1:
- TV mode's dark look on every screen.

---

**Windows:** download **Spine-Setup.exe** below and run it. No admin rights needed.

**Linux:** run `curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh`, or download **Spine-x86_64.AppImage**, allow it to run as a program (right-click → Properties → Permissions), and open it. Debian/Ubuntu/Mint: **Spine-amd64.deb**.

Already have Spine? It updates itself from Settings → Updates.
