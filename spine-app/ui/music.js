// Home music: each Home style can have its own song (ui/music/<style>.mp3),
// played on a loop while you're on Home. It fades out when you open a library
// or start a game or movie, and fades back in where it left off when you return.
// The files are already mixed quiet (about -20 LUFS); Settings → Home music sets
// Off/Soft/Normal. Loaded before app.js.
//
// Two ways to play: Windows streams the file with an <audio> element. Linux's
// engine (WebKitGTK) can't stream media from inside the app (the element never
// loads), so there the whole song is fetched, decoded and played through Web Audio.

const BGM = (() => {
  const SONGS = { wave: 'music/wave.mp3', channels: 'music/channels.mp3' };
  const level = () => ({ off: 0, soft: 0.55, normal: 1 })[settings().homeMusic || 'soft'] ?? 0.55;
  const linux = !navigator.userAgent.includes('Windows') && !navigator.userAgent.includes('Mac');

  // <audio> player (Windows).
  function elementPlayer() {
    const el = new Audio();
    el.loop = true; el.preload = 'auto'; el.volume = 0;
    let fadeT = 0;
    return {
      load(url) { clearInterval(fadeT); el.pause(); el.volume = 0; el.src = url; },
      playing: () => !el.paused,
      play() { const p = el.play(); if (p && p.catch) p.catch(() => {}); },
      fade(to, secs) {
        clearInterval(fadeT);
        const from = el.volume, start = performance.now();
        fadeT = setInterval(() => {
          const k = Math.min(1, (performance.now() - start) / (secs * 1000));
          el.volume = Math.max(0, Math.min(1, from + (to - from) * k));
          if (k >= 1) { clearInterval(fadeT); if (to === 0) el.pause(); }
        }, 40);
      },
    };
  }

  // Web Audio player (Linux): decoded song in memory, looped, volume on a gain node.
  function bufferPlayer() {
    let ctx = null, gain = null, buf = null, src = null, url = null, offset = 0, startedAt = 0, wantPlay = false, stopT = 0;
    const setup = () => {
      if (ctx) return ctx;
      const AC = window.AudioContext || window.webkitAudioContext;
      if (!AC) return null;
      ctx = new AC(); gain = ctx.createGain(); gain.gain.value = 0; gain.connect(ctx.destination);
      return ctx;
    };
    const start = () => {
      if (src || !buf || !wantPlay) return;
      if (ctx.state === 'suspended') ctx.resume().catch(() => {});
      src = ctx.createBufferSource(); src.buffer = buf; src.loop = true; src.connect(gain);
      src.start(0, offset % buf.duration);
      startedAt = ctx.currentTime - (offset % buf.duration);
    };
    const halt = () => {
      if (!src) return;
      offset = (ctx.currentTime - startedAt) % buf.duration; // remember where we were
      try { src.stop(); } catch (e) { /* already stopped */ }
      src.disconnect(); src = null;
    };
    return {
      load(u) {
        if (!setup()) return;
        halt(); buf = null; offset = 0; url = u;
        fetch(u).then((r) => r.arrayBuffer()).then((b) => ctx.decodeAudioData(b)).then((decoded) => {
          if (url !== u) return; // switched style while loading
          buf = decoded; start();
        }).catch(() => {});
      },
      playing: () => wantPlay,
      play() { if (!setup()) return; clearTimeout(stopT); wantPlay = true; start(); if (ctx.state === 'suspended') ctx.resume().catch(() => {}); },
      fade(to, secs) {
        if (!ctx) return;
        const t = ctx.currentTime;
        gain.gain.cancelScheduledValues(t);
        gain.gain.setValueAtTime(gain.gain.value, t);
        gain.gain.linearRampToValueAtTime(to, t + secs);
        clearTimeout(stopT);
        if (to === 0) { wantPlay = false; stopT = setTimeout(halt, secs * 1000 + 50); } else { wantPlay = true; start(); }
      },
    };
  }

  const player = linux ? bufferPlayer() : elementPlayer();
  let song = null, want = false;
  return {
    // Called after every render.
    sync() {
      const next = SONGS[settings().home_style] || null;
      want = !!next && S.screen === 'home' && !S.playing && !document.hidden && level() > 0 && systems().length > 0;
      if (!want) { if (player.playing()) player.fade(0, 0.8); return; }
      if (next !== song) { song = next; player.load(next); } // switched style: new song from the top
      if (!player.playing()) player.play();
      player.fade(level(), 1.5);
    },
    // Sound can be held back until the first click, key or button press; try again then.
    wake() { if (want) { player.play(); player.fade(level(), 1.5); } },
  };
})();
document.addEventListener('visibilitychange', () => BGM.sync());
['pointerdown', 'keydown'].forEach((ev) => document.addEventListener(ev, () => BGM.wake(), { passive: true }));
