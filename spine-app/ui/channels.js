// Channels: a Home style with a grid of rounded tiles, one per console, each
// with its covers slowly drifting by, a clock along the bottom and gentle
// background music (Settings → Home style → Channels). Loaded before app.js.

const channelsOn = () => settings().home_style === 'channels';
const CH_PER_PAGE = 12; // 4 across, 3 down

function chPageOf(i) { return Math.floor(i / CH_PER_PAGE); }

function chTileArt(t) {
  const pool = t.kind === 'sys' ? t.sys.games.map((g) => ({ s: t.sys, g })) : t.kind === 'fav' ? favItems() : t.kind === 'coll' ? collItems(t.coll)
    : systems().filter((x) => !isMovies(x)).flatMap((x) => x.games.map((g) => ({ s: x, g })));
  const withCovers = pool.filter((x) => x.g.cover);
  const step = t.kind === 'random' ? 29 : Math.max(1, Math.floor(withCovers.length / 7));
  const pick = withCovers.filter((_, i) => i % step === 0).slice(0, 7);
  if (pick.length < 3) return `<span class="ch-plain" style="background:${t.color}">${sysIcon(tileIcon(t), 96)}</span>`;
  const imgs = pick.map((x) => `<img src="${mediaUrl(x.g.cover)}" alt="" loading="lazy" decoding="async">`).join('');
  // Every tile drifts at its own pace and starting point so the grid never moves in lockstep.
  const h = hash(t.id);
  return `<span class="ch-strip" style="animation-duration:${38 + (h % 17)}s;animation-delay:${-(h % 40)}s">${imgs}${imgs}</span>`
    + (t.kind === 'random' ? `<span class="ch-badge">${sysIcon('random', 64)}</span>` : '');
}

function homeChannels() {
  const T = tiles();
  // Keep the page that holds the console you're on (Back from a library, LB/RB).
  const want = S.focusKey?.startsWith('con-') ? T.findIndex((t) => `con-${t.id}` === S.focusKey) : -1;
  const pages = Math.max(1, Math.ceil(T.length / CH_PER_PAGE));
  if (want >= 0) S.chPage = chPageOf(want);
  else if (S.chPage == null) S.chPage = chPageOf(Math.min(S.sel, T.length - 1));
  S.chPage = Math.min(S.chPage, pages - 1);
  const first = S.chPage * CH_PER_PAGE;
  const slots = [];
  for (let k = 0; k < CH_PER_PAGE; k++) {
    const i = first + k, t = T[i];
    if (!t) { slots.push('<span class="ch-tile empty" aria-hidden="true"></span>'); continue; }
    const count = t.count != null ? `${t.count.toLocaleString()} ${t.kind === 'sys' ? things(t.sys, t.count) : t.count === 1 ? 'game' : 'games'}` : 'Random pick';
    slots.push(`<button class="ch-tile" style="--chip:${t.color}" data-nav data-key="con-${esc(t.id)}" data-act="tile" data-i="${i}" aria-label="${esc(t.name)}, ${esc(count)}">
        <span class="ch-art">${chTileArt(t)}</span>
        <span class="ch-name"><span class="n">${esc(t.short)}</span><span class="c">${esc(count)}</span></span>
        ${t.kind === 'sys' && t.sys.blocked ? `<span class="ch-warn" title="${esc(t.sys.blocked)}">!</span>` : ''}</button>`);
  }
  const now = new Date();
  const time = now.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const date = now.toLocaleDateString([], { weekday: 'short', month: 'numeric', day: 'numeric' });
  const arrow = (d, on) => on ? `<button class="ch-page ${d < 0 ? 'prev' : 'next'}" data-act="chpage" data-d="${d}" tabindex="-1" aria-label="${d < 0 ? 'Previous' : 'Next'} page">
      <svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="${d < 0 ? 'M15 5l-7 7 7 7' : 'M9 5l7 7-7 7'}"/></svg></button>` : '';
  const offline = (S.boot.roots || []).some((r) => !r.online);
  return `<main class="main channels" id="channels">
      <div class="ch-grid">${slots.join('')}</div>
      ${arrow(-1, S.chPage > 0)}${arrow(1, S.chPage < pages - 1)}
      ${pages > 1 ? `<div class="ch-dots">${Array.from({ length: pages }, (_, p) => `<span class="${p === S.chPage ? 'on' : ''}"></span>`).join('')}</div>` : ''}
      <footer class="ch-bar">
        <button class="ch-round" data-nav data-key="gear" data-act="settings" aria-label="Settings">${ICON.gear}<span>Settings</span></button>
        <div class="ch-clock"><span class="t" id="clock">${time}</span><span class="d" id="chdate">${date}${offline ? ' · a drive is unplugged' : ''}</span></div>
        <button class="ch-round" data-nav data-key="searchbtn" data-act="search" aria-label="Search">${ICON.search}<span>Search</span></button>
      </footer>
    </main>`;
}

// Left/right past the edge of a page turns the page. Returns true when handled.
function channelsKey(btn) {
  if (btn !== 'left' && btn !== 'right') return false;
  const cur = document.activeElement;
  if (!cur?.classList?.contains('ch-tile')) return false;
  const T = tiles();
  const i = +cur.dataset.i, col = (i % CH_PER_PAGE) % 4;
  const pages = Math.ceil(T.length / CH_PER_PAGE);
  const d = btn === 'right' ? 1 : -1;
  if ((d > 0 && col !== 3 && T[i + 1]) || (d < 0 && col !== 0)) return false; // normal move inside the page
  const p = S.chPage + d;
  if (p < 0 || p >= pages) return false; // let the normal move say "edge"
  const row = Math.floor((i % CH_PER_PAGE) / 4);
  let j = p * CH_PER_PAGE + row * 4 + (d > 0 ? 0 : 3);
  j = Math.min(j, T.length - 1);
  S.chPage = p; S.focusKey = `con-${T[j].id}`;
  SFX.play('tick');
  render();
  return true;
}

// ---------------------------------------------------------------------------
// Home music: an original, easygoing lounge tune played by tiny synthesized
// instruments (no audio files). Electric piano chords, a plucky melody, soft
// bass and a shaker, with a light swing. Plays only on the Channels home, and
// stops while a game or movie is running. Settings → Sounds → Home music.
// ---------------------------------------------------------------------------
const BGM = (() => {
  const BPM = 100, SWING = 0.09, BARS = 16;
  // [bass root (MIDI), chord notes (MIDI)] per half bar: two entries per bar.
  const H = (b, c) => [b, c];
  const Fmaj7 = H(41, [53, 57, 60, 64]), Dm9 = H(38, [53, 57, 60, 64]), Gm7 = H(43, [53, 58, 62, 65]), C9s = H(36, [58, 62, 65]);
  const Bbmaj7 = H(46, [57, 62, 65]), Am7 = H(45, [55, 60, 64]), C7 = H(36, [52, 58, 62]);
  const Dbmaj7 = H(37, [53, 56, 60]), Cm7 = H(36, [55, 58, 63]), F7 = H(41, [57, 60, 63]), Bbm6 = H(46, [55, 61, 65]), D7 = H(38, [54, 57, 60]);
  const CHORDS = [
    Fmaj7, Fmaj7, Dm9, Dm9, Gm7, Gm7, C9s, C9s, Bbmaj7, Bbmaj7, Am7, Am7, Gm7, Gm7, C7, C7,
    Dbmaj7, Dbmaj7, Cm7, F7, Bbmaj7, Bbmaj7, Bbm6, Bbm6, Am7, Am7, D7, D7, Gm7, Gm7, C9s, C7,
  ];
  // Melody per bar: [beat, MIDI note, length in beats]. Written for Spine.
  const MEL = [
    [[0, 72, 1], [1, 76, 0.5], [1.5, 77, 0.5], [2, 79, 1.5], [3.5, 76, 0.5]],
    [[0, 74, 1.5], [1.5, 72, 0.5], [2, 69, 2]],
    [[0.5, 70, 0.5], [1, 74, 0.5], [1.5, 77, 1], [2.5, 74, 0.5], [3, 72, 1]],
    [[0, 74, 3], [3.5, 67, 0.5]],
    [[0, 69, 1], [1, 74, 0.5], [1.5, 77, 0.5], [2, 81, 1.5], [3.5, 79, 0.5]],
    [[0, 76, 1.5], [1.5, 72, 0.5], [2, 74, 0.5], [2.5, 76, 1.5]],
    [[0, 77, 0.5], [0.5, 76, 0.5], [1, 74, 1], [2, 70, 1], [3, 72, 0.5], [3.5, 74, 0.5]],
    [[0, 72, 2.5], [3, 70, 0.5], [3.5, 69, 0.5]],
    [[0, 72, 1], [1, 77, 0.5], [1.5, 80, 1.5], [3, 79, 1]],
    [[0, 79, 0.5], [0.5, 75, 1], [1.5, 72, 0.5], [2, 75, 0.5], [2.5, 77, 1.5]],
    [[0, 74, 1], [1, 77, 0.5], [1.5, 81, 1], [2.5, 79, 0.5], [3, 77, 1]],
    [[0, 73, 1.5], [1.5, 77, 0.5], [2, 79, 2]],
    [[0, 76, 0.5], [0.5, 79, 0.5], [1, 84, 1.5], [2.5, 81, 0.5], [3, 79, 1]],
    [[0, 78, 1.5], [1.5, 74, 0.5], [2, 72, 0.5], [2.5, 69, 1.5]],
    [[0, 70, 0.5], [0.5, 74, 0.5], [1, 77, 0.5], [1.5, 81, 1], [2.5, 79, 0.5], [3, 77, 0.5], [3.5, 74, 0.5]],
    [[0, 76, 2], [2, 79, 0.5], [2.5, 76, 0.5], [3, 72, 1]],
  ];
  const level = () => ({ off: 0, soft: 0.5, normal: 1 })[settings().homeMusic || 'soft'] ?? 0.5;
  const hz = (m) => 440 * Math.pow(2, (m - 69) / 12);
  const beat = 60 / BPM;
  let ctx = null, out = null, verb = null, noise = null, timer = 0, on = false, startAt = 0, nextStep = 0;

  function setup() {
    if (ctx) return ctx;
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC();
    out = ctx.createGain(); out.gain.value = 0;
    const lp = ctx.createBiquadFilter(); lp.type = 'lowpass'; lp.frequency.value = 6000;
    out.connect(lp); lp.connect(ctx.destination);
    // A small room: decaying noise as the reverb's echo.
    verb = ctx.createConvolver();
    const len = Math.floor(ctx.sampleRate * 1.6), ir = ctx.createBuffer(2, len, ctx.sampleRate);
    for (let ch = 0; ch < 2; ch++) { const d = ir.getChannelData(ch); for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, 3); }
    verb.buffer = ir;
    const wet = ctx.createGain(); wet.gain.value = 0.22;
    verb.connect(wet); wet.connect(out);
    noise = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const nd = noise.getChannelData(0); for (let i = 0; i < nd.length; i++) nd[i] = Math.random() * 2 - 1;
    return ctx;
  }
  function send(node, vol, reverb = 0.5) {
    const g = ctx.createGain(); g.gain.value = vol;
    node.connect(g); g.connect(out);
    if (reverb) { const r = ctx.createGain(); r.gain.value = reverb; g.connect(r); r.connect(verb); }
    return g;
  }
  function env(g, t, peak, attack, decay) {
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(peak, t + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t + attack + decay);
  }
  // Electric piano: a sine with a quick bell-like shimmer from a second sine.
  function keys(m, t, len, vol) {
    const o = ctx.createOscillator(), b = ctx.createOscillator(), g = ctx.createGain(), bg = ctx.createGain();
    o.type = 'sine'; o.frequency.value = hz(m);
    b.type = 'sine'; b.frequency.value = hz(m) * 4;
    env(g, t, vol, 0.01, len + 0.4);
    env(bg, t, vol * 0.18, 0.005, 0.25);
    o.connect(g); b.connect(bg); bg.connect(g); send(g, 1, 0.6);
    o.start(t); b.start(t); o.stop(t + len + 0.5); b.stop(t + 0.35);
  }
  // Melody: a soft pluck, like a toy marimba, with a quiet octave sparkle.
  function pluck(m, t, len, vol) {
    const o = ctx.createOscillator(), h = ctx.createOscillator(), g = ctx.createGain(), hg = ctx.createGain();
    o.type = 'triangle'; o.frequency.value = hz(m);
    h.type = 'sine'; h.frequency.value = hz(m) * 2;
    const d = Math.min(0.9, 0.25 + len * beat * 0.6);
    env(g, t, vol, 0.006, d); env(hg, t, vol * 0.25, 0.004, d * 0.5);
    o.connect(g); h.connect(hg); hg.connect(g); send(g, 1, 0.7);
    o.start(t); h.start(t); o.stop(t + d + 0.05); h.stop(t + d + 0.05);
  }
  function bass(m, t, len, vol) {
    const o = ctx.createOscillator(), g = ctx.createGain();
    o.type = 'triangle'; o.frequency.value = hz(m);
    env(g, t, vol, 0.01, len * beat * 0.9);
    o.connect(g); send(g, 1, 0);
    o.start(t); o.stop(t + len * beat + 0.1);
  }
  function hit(t, vol, freq, q, len) {
    const s = ctx.createBufferSource(), f = ctx.createBiquadFilter(), g = ctx.createGain();
    s.buffer = noise; f.type = 'bandpass'; f.frequency.value = freq; f.Q.value = q;
    env(g, t, vol, 0.002, len);
    s.connect(f); f.connect(g); send(g, 1, 0.15);
    s.start(t, Math.random() * 0.5, len + 0.05);
  }
  function kick(t, vol) {
    const o = ctx.createOscillator(), g = ctx.createGain();
    o.frequency.setValueAtTime(110, t); o.frequency.exponentialRampToValueAtTime(48, t + 0.12);
    env(g, t, vol, 0.003, 0.22);
    o.connect(g); send(g, 1, 0);
    o.start(t); o.stop(t + 0.3);
  }
  // Step n is the n-th eighth note since the music started; off-beats swing a little.
  const timeOf = (n) => startAt + (n / 2 + (n % 2 ? SWING : 0)) * beat;

  function step(n) {
    const bar = Math.floor(n / 8) % BARS, b = (n % 8) / 2, loop = Math.floor(n / (BARS * 8));
    const t = timeOf(n);
    const [root, chord] = CHORDS[bar * 2 + (b >= 2 ? 1 : 0)];
    if (b === 0 || b === 2) bass(root, t, 1.5, 0.22);
    if (b === 1.5 || b === 3.5) bass(root + 7, t, 0.5, 0.15);
    if (b === 0 || b === 2.5) chord.forEach((m, k) => keys(m, t + k * 0.012, b === 0 ? 1.2 : 1, 0.05));
    if (b === 1.5) chord.forEach((m) => keys(m, t, 0.3, 0.03));
    hit(t, b % 1 ? 0.05 : 0.03, 7000, 1.2, 0.05); // shaker on every eighth
    if (b === 1.5 || b === 3) hit(t, 0.06, 1800, 6, 0.04); // rim click
    if (b === 0 || (b === 2.5 && bar % 2)) kick(t, 0.18);
    // The melody rests every other time round, so it doesn't wear thin.
    if (loop % 2 === 0) MEL[bar].forEach(([mb, m, len]) => {
      if (mb >= b && mb < b + 0.5) pluck(m, startAt + ((n - (n % 8)) / 2 + mb + (mb % 1 === 0.5 ? SWING : 0)) * beat, len, 0.11);
    });
  }
  function pump() {
    if (!on || !ctx) return;
    while (timeOf(nextStep) < ctx.currentTime + 0.35) {
      try { step(nextStep); } catch (e) { /* audio device went away */ }
      nextStep++;
    }
  }
  function start() {
    if (!setup()) return;
    if (ctx.state === 'suspended') ctx.resume().catch(() => {});
    if (on) return fade();
    on = true; nextStep = 0; startAt = ctx.currentTime + 0.15;
    clearInterval(timer); timer = setInterval(pump, 100); pump();
    fade();
  }
  function stop() {
    if (!on || !ctx) return;
    on = false;
    out.gain.cancelScheduledValues(ctx.currentTime);
    out.gain.setTargetAtTime(0, ctx.currentTime, 0.25);
    clearInterval(timer);
    setTimeout(() => { if (!on && ctx.state === 'running') ctx.suspend().catch(() => {}); }, 1500);
  }
  function fade() {
    out.gain.cancelScheduledValues(ctx.currentTime);
    out.gain.setTargetAtTime(0.9 * level(), ctx.currentTime, 0.6);
  }
  return {
    // Called after every render: plays when you're on the Channels home.
    sync() {
      const want = channelsOn() && S.screen === 'home' && !S.playing && !document.hidden && level() > 0 && systems().length > 0;
      if (want) { if (!on) start(); else fade(); } else stop();
    },
    // Browsers hold sound back until the first click or key press; try again then.
    wake() { if (on && ctx?.state === 'suspended') ctx.resume().catch(() => {}); },
  };
})();
document.addEventListener('visibilitychange', () => BGM.sync());
['pointerdown', 'keydown'].forEach((ev) => document.addEventListener(ev, () => BGM.wake(), { passive: true }));
