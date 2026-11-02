// Home music: each Home style can have its own song (ui/music/<style>.mp3),
// played on a loop while you're on Home. It fades out when you open a library
// or start a game or movie, and fades back in when you return. The files are
// already mixed quiet (about -20 LUFS); Settings → Home music sets Off/Soft/Normal.
// Loaded before app.js.

const BGM = (() => {
  const SONGS = { wave: 'music/wave.mp3', channels: 'music/channels.mp3' };
  const level = () => ({ off: 0, soft: 0.55, normal: 1 })[settings().homeMusic || 'soft'] ?? 0.55;
  const el = new Audio();
  el.loop = true;
  el.preload = 'auto';
  el.volume = 0;
  let song = null, want = false, fadeT = 0;

  // Smoothly move the volume to `to`; pause once it reaches silence.
  function fade(to, secs) {
    clearInterval(fadeT);
    const from = el.volume, start = performance.now();
    fadeT = setInterval(() => {
      const k = Math.min(1, (performance.now() - start) / (secs * 1000));
      el.volume = Math.max(0, Math.min(1, from + (to - from) * k));
      if (k >= 1) { clearInterval(fadeT); if (to === 0) el.pause(); }
    }, 40);
  }
  function play() {
    const p = el.play();
    if (p && p.catch) p.catch(() => {}); // held back until the first click or key press; wake() retries
  }
  return {
    // Called after every render.
    sync() {
      const style = settings().home_style;
      const next = SONGS[style] || null;
      want = !!next && S.screen === 'home' && !S.playing && !document.hidden && level() > 0 && systems().length > 0;
      if (!want) { if (!el.paused) fade(0, 0.8); return; }
      if (next !== song) { // switched style: start the new song from the top
        song = next; clearInterval(fadeT); el.pause(); el.volume = 0; el.src = next;
      }
      if (el.paused) play();
      fade(level(), 1.5);
    },
    wake() { if (want && el.paused) { play(); fade(level(), 1.5); } },
  };
})();
document.addEventListener('visibilitychange', () => BGM.sync());
['pointerdown', 'keydown'].forEach((ev) => document.addEventListener(ev, () => BGM.wake(), { passive: true }));
