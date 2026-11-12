// ---------------------------------------------------------------------------
// The mascot. She drops by now and then (welcome back, play recaps, milestones,
// little tips, reading while you're idle) and sits on, leans against, lies on or
// peeks over real parts of the screen. Settings → Look → Mascot sets how often.
//
// She lives in her own layer (#mascot, outside #app) so render() never wipes her
// and she never takes focus: controller and keyboard keep working while she's
// there, and any press sends her off. Art: ui/mascot/<pose>.webp (and
// <pose>_blink.webp for the poses that blink).
// ---------------------------------------------------------------------------
const MASCOT = (() => {
  // ar = width / height. face = where her face is (0..1 of the picture), for the
  // speech bubble. blink: there's a <pose>_blink.webp with only the eyes closed.
  const POSE = {
    sit_wave: { ar: 0.405, face: [0.45, 0.12], blink: true, seat: 0.55 },
    peek: { ar: 1.45, face: [0.5, 0.36], blink: true },
    point: { ar: 0.416, face: [0.42, 0.08], blink: true },
    think: { ar: 0.282, face: [0.5, 0.08], blink: true },
    cheer: { ar: 0.332, face: [0.5, 0.2] },
    doze: { ar: 0.92, face: [0.58, 0.2] },
    surprise: { ar: 0.343, face: [0.5, 0.3] },
    hang: { ar: 0.412, face: [0.5, 0.2] },
    flustered: { ar: 0.608, face: [0.5, 0.2] },
    lean: { ar: 0.55, face: [0.55, 0.08] },
    lie: { ar: 1.678, face: [0.12, 0.28] },
    bye: { ar: 0.505, face: [0.45, 0.08] },
    thumbs_up: { ar: 0.707, face: [0.45, 0.13] },
    peek_side: { ar: 0.791, face: [0.42, 0.45] },
    read: { ar: 0.788, face: [0.5, 0.15] },
  };
  const src = (p, blink) => `mascot/${p}${blink ? '_blink' : ''}.webp`;

  // How often: off · moments (welcome + after you play) · sometimes (+ tips and idle) · often.
  const level = () => settings().mascot || 'sometimes';
  const on = () => level() !== 'off';
  const GAP = { moments: Infinity, sometimes: [8, 14], often: [3, 6] }; // minutes between drop-ins on Home

  // Small memory kept in settings (spine-state.json): last session, milestones shown, last visit.
  const memo = () => settings().mascotMemo || {};
  function remember(patch) {
    const v = { ...memo(), ...patch };
    S.boot.settings = { ...settings(), mascotMemo: v };
    api.call('set_setting', { key: 'mascotMemo', value: v }).catch(() => {});
  }

  // ---- words ------------------------------------------------------------------
  function say(secs) {
    const m = Math.max(1, Math.round(secs / 60));
    if (m < 60) return `${m} minute${m === 1 ? '' : 's'}`;
    const h = Math.floor(m / 60), r = m % 60;
    const hs = `${h} hour${h === 1 ? '' : 's'}`;
    return r >= 5 && h < 10 ? `${hs} ${r} minutes` : hs;
  }
  function when(t) {
    const d = (Date.now() / 1000 - t) / 86400;
    if (d < 1) return new Date(t * 1000).getDate() === new Date().getDate() ? 'earlier today' : 'last night';
    if (d < 2) return 'yesterday';
    if (d < 7) return `${Math.floor(d)} days ago`;
    if (d < 14) return 'last week';
    if (d < 60) return `${Math.floor(d / 7)} weeks ago`;
    return `${Math.floor(d / 30)} months ago`;
  }
  const clockNow = () => new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const pick = (a) => a[Math.floor(Math.random() * a.length)];
  const isGame = (x) => x && !isMovies(x.s) && !/^(movies|tv|stream)\//.test(String(x.g.id));
  const games = () => allItems().filter(isGame);
  const lp = () => S.boot?.last_played || {};
  const pt = () => S.boot?.playtime || {};
  const lastGames = () => Object.entries(lp()).sort((a, b) => b[1] - a[1]).map(([id]) => findGame(id)).filter(isGame);

  // ---- layer, placement ---------------------------------------------------------
  let root = null;
  let cur = null; // what's on screen: { pose, at, lines, idle, el, bub, born, ... }
  function layer() {
    if (root) return root;
    root = document.createElement('div');
    root.id = 'mascot';
    root.setAttribute('aria-live', 'polite');
    document.body.appendChild(root);
    return root;
  }
  // Positions in the layer's own CSS pixels. With Interface size above 100%,
  // newer WebView2 reports rects zoomed: divide by the measured factor (as jumpletter does).
  function rectOf(el) {
    const r0 = layer().getBoundingClientRect(), k = root.offsetWidth ? r0.width / root.offsetWidth : 1;
    const r = el.getBoundingClientRect();
    return { l: (r.left - r0.left) / k, t: (r.top - r0.top) / k, r: (r.right - r0.left) / k, b: (r.bottom - r0.top) / k, w: r.width / k, h: r.height / k };
  }
  const view = () => ({ w: layer().offsetWidth, h: root.offsetHeight });
  const visible = (el) => {
    if (!el) return null;
    const r = rectOf(el), v = view(), top = barLine('top'), bot = barLine('bottom');
    return r.w > 0 && r.l >= -2 && r.r <= v.w + 2 && r.t >= top - 2 && r.b <= bot + 2 ? r : null;
  };
  function barLine(which) {
    const v = view();
    if (which === 'top') { const t = document.querySelector('#app .topbar, #app .tv-top'); return t ? rectOf(t).b : 0; }
    const b = [...document.querySelectorAll('#app .hints, #app .tv-hints')].pop();
    return b ? rectOf(b).t : v.h;
  }

  // Where (and how big) a pose goes. Returns null when that spot isn't on screen;
  // the caller then falls back to the hint bar, which every screen has.
  function place(at, pose) {
    const P = POSE[pose], v = view();
    const box = (x, y, h, extra = {}) => ({ x, y, h, w: h * P.ar, ...extra });
    if (at === 'tile-sit' || at === 'tile-lean') {
      const t = visible(document.querySelector('#app .console.sel'));
      if (!t) return null;
      if (at === 'tile-sit') { const h = t.h * 1.38; return box(t.l + t.w / 2 - (h * P.ar) / 2, t.t - h * P.seat, h, { enter: 'drop' }); }
      const h = t.h * 1.3, w = h * P.ar;
      if (t.r + w - 6 <= v.w) return box(t.r - 6, t.b - h, h, { enter: 'pop' });
      return box(t.l - w + 6, t.b - h, h, { enter: 'pop', mirror: true });
    }
    if (at === 'card-lie' || at === 'card-point') {
      const c = visible(document.querySelector('#app .recent .item'));
      if (!c) return null;
      if (at === 'card-lie') { const w = Math.min(c.w * 0.7, 310), h = w / P.ar; return box(c.l + c.w * 0.08, c.t - h * 0.94, h, { enter: 'drop' }); }
      const h = 250; // stands just right of the card, pointing back at it
      return box(c.r + 2, c.b - h, h, { enter: 'pop', mirror: true });
    }
    if (at === 'hang') {
      const top = barLine('top'), h = 240;
      return box(v.w * 0.64, top - h * 0.03, h, { enter: 'drop', swing: true });
    }
    if (at === 'side') {
      const h = Math.min(270, v.h * 0.42);
      return box(v.w - h * P.ar, v.h * 0.32, h, { enter: 'side' });
    }
    // The hint bar: stand, sit or lie on it, or rise from behind it.
    const bar = barLine('bottom');
    const right = (w) => v.w - w - 48;
    const H = { cheer: 240, think: 240, surprise: 250, bye: 240, point: 240, lean: 240, doze: 160, read: 180, peek: 200, flustered: 210, thumbs_up: 215, sit_wave: 250, lie: 140, hang: 220, peek_side: 220 };
    const h = H[pose] || 230, w = h * P.ar;
    if (pose === 'peek' || pose === 'flustered' || pose === 'thumbs_up') return box(right(w), bar - h * 0.97, h, { enter: 'rise', clip: bar });
    if (pose === 'sit_wave') return box(right(w), bar - h * P.seat, h, { enter: 'drop' });
    if (pose === 'lie') return box(right(w), bar - h * 0.95, h, { enter: 'drop' });
    return box(right(w), bar - h + 4, h, { enter: 'pop', mirror: pose === 'point' });
  }

  // ---- show / leave -------------------------------------------------------------
  let blinkT = 0, lineT = 0, stayT = 0;
  function show(o) {
    if (o.force && cur && canShow(o)) leave(true);
    if (!on() || cur || !canShow(o)) return false;
    let pose = o.pose, spot = place(o.at || 'bar', pose);
    if (!spot) spot = place('bar', pose);
    if (!spot) return false;
    layer();
    const P = POSE[pose];
    const fig = document.createElement('div');
    fig.className = `msc-fig in-${spot.enter}${spot.mirror ? ' mirror' : ''}${spot.swing ? ' swing' : ''}${pose === 'doze' ? ' breathe' : ''}`;
    fig.innerHTML = `<div class="msc-bob"><img alt="" draggable="false" src="${src(pose)}"></div>${pose === 'doze' ? '<span class="msc-z"><i>z</i><i>z</i><i>Z</i></span>' : ''}`;
    let wrap = fig;
    if (spot.clip != null) { // rising from behind the bar: hide what's below its edge
      wrap = document.createElement('div');
      wrap.className = 'msc-clip';
      wrap.style.height = `${spot.clip}px`;
      wrap.appendChild(fig);
    }
    const bub = document.createElement('div');
    bub.className = 'msc-bub';
    bub.onclick = () => leave();
    root.append(wrap, bub);
    cur = { ...o, pose, spot, fig, wrap, bub, born: performance.now(), line: 0 };
    position();
    if (P.blink) { const pre = new Image(); pre.src = src(pose, true); blinkLoop(); }
    nextLine();
    requestAnimationFrame(() => requestAnimationFrame(() => { fig.classList.add('here'); }));
    if (!o.idle) stayT = setTimeout(leave, o.stay || 3000 + (o.lines.length - 1) * 3600 + o.lines[o.lines.length - 1].length * 60);
    S_last = Date.now();
    return true;
  }
  function canShow(o) {
    if (S.playing || S.panel || S.shot != null || document.querySelector('#app .search-panel')) return false;
    if (o.homeOnly && S.screen !== 'home') return false;
    return ['home', 'games', 'detail'].includes(S.screen);
  }
  function position() {
    if (!cur) return;
    const { spot, fig, bub } = cur;
    Object.assign(fig.style, { left: `${spot.x}px`, top: `${spot.y}px`, width: `${spot.w}px`, height: `${spot.h}px` });
    placeBubble();
  }
  function placeBubble() {
    const { spot, bub, pose } = cur, P = POSE[pose], v = view();
    const fx = spot.x + spot.w * (spot.mirror ? 1 - P.face[0] : P.face[0]), fy = spot.y + spot.h * P.face[1];
    const bw = Math.min(300, v.w * 0.3);
    bub.style.width = `${bw}px`;
    const bh = bub.offsetHeight || 60;
    let left = fx - spot.w * 0.22 - bw, side = 'r';
    if (left < 16) { left = fx + spot.w * 0.22; side = 'l'; }
    left = Math.max(16, Math.min(v.w - bw - 16, left));
    const top = Math.max(barLine('top') + 10, Math.min(v.h - bh - 16, fy - bh * 0.7));
    bub.className = `msc-bub tail-${side}${bub.classList.contains('here') ? ' here' : ''}`;
    Object.assign(bub.style, { left: `${left}px`, top: `${top}px` });
  }
  function nextLine() {
    if (!cur) return;
    const text = cur.lines[cur.line];
    cur.bub.classList.remove('here');
    setTimeout(() => {
      if (!cur) return;
      cur.bub.textContent = text;
      placeBubble();
      cur.bub.classList.add('here');
    }, cur.line ? 180 : 420);
    if (cur.line < cur.lines.length - 1) lineT = setTimeout(() => { if (cur) { cur.line++; nextLine(); } }, 3600);
  }
  function blinkLoop() {
    clearTimeout(blinkT);
    blinkT = setTimeout(() => {
      const img = cur?.fig.querySelector('img');
      if (!img || !POSE[cur.pose].blink) return;
      img.src = src(cur.pose, true);
      setTimeout(() => { if (img.isConnected) img.src = src(cur?.pose || 'sit_wave'); blinkLoop(); }, 130);
    }, 2200 + Math.random() * 3800);
  }
  function swapPose(pose, lines) { // in place (reading → dozing off)
    if (!cur) return;
    leave(true);
    show({ ...cur0, pose, lines });
  }
  let cur0 = null;
  function leave(quick) {
    if (!cur) return;
    clearTimeout(blinkT); clearTimeout(lineT); clearTimeout(stayT);
    const { wrap, fig, bub } = cur;
    cur0 = cur;
    cur = null;
    fig.classList.remove('here'); fig.classList.add('away');
    bub.classList.remove('here');
    setTimeout(() => { wrap.remove(); bub.remove(); }, quick ? 0 : 450);
    if (quick) { wrap.remove(); bub.remove(); }
    lastLeft = Date.now();
  }
  let S_last = 0, lastLeft = 0;

  // ---- moments ------------------------------------------------------------------
  function welcome() {
    const m = memo(), now = Date.now() / 1000, hour = new Date().getHours();
    const away = m.seen ? (now - m.seen) / 86400 : 0;
    remember({ seen: now });
    const last = m.last && findGame(m.last.id);
    const recap = last && isGame(last) ? `Last time you played ${last.g.title} for ${say(m.last.sec)}.`
      : (() => { const g = lastGames()[0]; return g && pt()[g.g.id] ? `You've put ${say(pt()[g.g.id])} into ${g.g.title} so far.` : null; })();
    if (!m.seen) return show({ pose: 'sit_wave', at: 'tile-sit', lines: ['Hi there! Nice to meet you.', 'I\'ll drop by now and then. Have fun!'], homeOnly: true });
    if (hour < 5) return show({ pose: 'doze', at: 'bar', lines: [`Zzz… oh! Welcome back.`, `It's ${clockNow()}. One more level?`] });
    const hi = away > 3 ? `Long time no see! It's been ${Math.floor(away)} days.` : hour < 12 ? 'Good morning! Welcome back.' : hour < 18 ? 'Good afternoon! Welcome back.' : 'Good evening! Welcome back.';
    return show({ pose: 'sit_wave', at: 'tile-sit', lines: recap ? [hi, recap] : [hi, 'Pick a console and let\'s play something.'], homeOnly: true });
  }

  // After a game closes. prev = total seconds before this session.
  const MILESTONES = [1, 5, 10, 25, 50, 100, 250, 500];
  function played(p, prev) {
    const found = findGame(p.id);
    if (!found || !isGame(found) || !(p.seconds > 0)) return;
    remember({ last: { id: p.id, sec: p.seconds, at: Date.now() / 1000 } });
    if (!on() || p.seconds < 90) return;
    const title = found.g.title, total = prev + p.seconds;
    const crossed = MILESTONES.filter((h) => prev < h * 3600 && total >= h * 3600).pop();
    const shown = memo().ms || {};
    let o = null;
    if (crossed && (shown[p.id] || 0) < crossed && crossed >= 5) {
      remember({ ms: { ...shown, [p.id]: crossed } });
      o = crossed >= 25 ? { pose: 'flustered', lines: [`${title} just passed ${crossed} hours…`, crossed >= 100 ? 'That\'s real dedication. I\'m impressed!' : 'That\'s a lot of hours! You must really like it.'] }
        : { pose: 'cheer', lines: [`${crossed} hours in ${title}!`, 'Nice milestone!'] };
    } else if (!prev) {
      o = { pose: 'cheer', lines: [`First go at ${title}: ${say(p.seconds)}.`, 'What did you think?'] };
    } else if (p.seconds >= 2 * 3600) {
      o = { pose: 'think', lines: [`That was a ${Math.round(p.seconds / 3600)}-hour session!`, 'Maybe stretch your legs a little?'] };
    } else if (level() !== 'moments' || Math.random() < 0.5) {
      o = { pose: 'thumbs_up', lines: [pick([`Nice! ${say(p.seconds)} of ${title}.`, `${say(p.seconds)} of ${title}. Good session!`, `Back already? ${say(p.seconds)} of ${title}.`])] };
    }
    if (o) setTimeout(() => show({ ...o, at: 'bar', force: true }), 1400);
  }

  function spun(id) {
    const f = findGame(id);
    if (!f || !on() || level() === 'moments' || Math.random() < 0.4) return;
    setTimeout(() => show({ pose: 'surprise', at: 'bar', force: true, lines: [pick([`Ta-da! How about ${f.g.title}?`, `My pick: ${f.g.title}!`, `${f.g.title}. Trust me on this one.`])] }), 700);
  }

  // Drop-ins on Home: one at a time, a few minutes apart.
  function ambient() {
    const opts = [];
    const G = games(), L = lp(), T = pt();
    const played = G.filter((x) => T[x.g.id]);
    const totalH = Math.round(played.reduce((n, x) => n + T[x.g.id], 0) / 3600);
    const tile = (() => { const t = tiles()[S.sel]; return t && t.kind === 'sys' && !isMovies(t.sys) ? t : null; })();
    if (totalH >= 2) opts.push({ pose: 'lean', at: 'tile-lean', lines: [`You've played ${totalH} hours across ${played.length} games in Spine.`] });
    const top = played.sort((a, b) => T[b.g.id] - T[a.g.id])[0];
    if (top && T[top.g.id] > 3600) opts.push({ pose: 'lie', at: 'card-lie', lines: [`${top.g.title} is your most played game.`, `${say(T[top.g.id])} and counting!`] });
    const month = Date.now() / 1000 - 30 * 86400;
    const fresh = G.filter((x) => x.g.added > month && !L[x.g.id]).sort((a, b) => b.g.added - a.g.added)[0];
    if (fresh) opts.push({ pose: 'think', at: 'bar', lines: [`You added ${fresh.g.title} ${when(fresh.g.added)}.`, 'Want to give it a try?'] });
    const fav = favItems().filter(isGame).filter((x) => L[x.g.id] && L[x.g.id] < Date.now() / 1000 - 45 * 86400)[0];
    if (fav) opts.push({ pose: 'peek_side', at: 'side', lines: [`Psst… ${fav.g.title} is one of your favorites.`, `You last played it ${when(L[fav.g.id])}.`] });
    const quiet = systems().filter((s) => !isMovies(s) && s.games.length >= 5).map((s) => ({ s, t: Math.max(0, ...s.games.map((g) => L[g.id] || 0)) })).filter((x) => x.t && x.t < Date.now() / 1000 - 30 * 86400).sort((a, b) => a.t - b.t)[0];
    if (quiet) opts.push({ pose: 'peek', at: 'bar', lines: [`Haven't seen you play ${quiet.s.short || quiet.s.name} since ${when(quiet.t)}.`, `${quiet.s.games.length} games waiting there!`] });
    if (tile) opts.push({ pose: 'lean', at: 'tile-lean', lines: [`${tile.count} ${tile.sys.short} games. Can't choose?`, 'Press R3 and I\'ll pick one for you.'] });
    const rec = lastGames()[0];
    if (rec) opts.push({ pose: 'point', at: 'card-point', lines: [`You played ${rec.g.title} ${when(L[rec.g.id])}.`, T[rec.g.id] ? `${say(T[rec.g.id])} so far. Pick up where you left off?` : 'Pick up where you left off?'] });
    opts.push({ pose: 'hang', at: 'hang', lines: [pick(['Just hanging around.', 'Don\'t mind me!', 'Nice library you have here.'])] });
    if (new Date().getHours() < 5) opts.push({ pose: 'doze', at: 'bar', lines: [`It's ${clockNow()}…`, 'Remember to sleep sometime, okay?'] });
    // Don't repeat the same pose twice in a row.
    const fresh2 = opts.filter((o) => o.pose !== lastPose);
    const o = pick(fresh2.length ? fresh2 : opts);
    if (o && show({ ...o, homeOnly: true })) lastPose = o.pose;
  }
  let lastPose = null;

  // ---- timing: input, idle, drop-ins --------------------------------------------
  let lastInput = Date.now(), nextDrop = 0, idleStage = 0;
  function planDrop() {
    const g = GAP[level()];
    nextDrop = g === Infinity || !g ? Infinity : Date.now() + (g[0] + Math.random() * (g[1] - g[0])) * 60000;
  }
  function input() {
    lastInput = Date.now();
    if (idleStage) { idleStage = 0; if (cur?.idle) leave(); return; }
    // A press right as she arrives (e.g. still holding a button) doesn't count.
    if (cur && performance.now() - cur.born > 700) leave();
  }
  function tick() {
    if (!on() || !S.boot) return;
    const idle = Date.now() - lastInput;
    const homeFree = S.screen === 'home' && !S.panel && !S.playing && document.visibilityState === 'visible';
    if (level() !== 'moments' && homeFree) {
      if (idleStage === 0 && idle > 150000 && !cur) {
        if (show({ pose: 'read', at: 'bar', lines: [pick(['I\'ll read while you decide.', 'Take your time. I brought a book.'])], idle: true, homeOnly: true })) idleStage = 1;
      } else if (idleStage === 1 && idle > 480000 && cur?.idle) {
        idleStage = 2; swapPose('doze', ['Zzz…']);
      }
    }
    if (homeFree && !cur && !idleStage && Date.now() >= nextDrop && idle < 120000 && Date.now() - lastLeft > 60000) { ambient(); planDrop(); }
  }

  let booted = false;
  function boot() { // first Home after start
    if (booted || !on()) return;
    booted = true;
    planDrop();
    setTimeout(() => { if (S.screen === 'home') welcome(); }, 1400);
  }

  // After every render(): follow the anchor, or leave when it's gone.
  function sync() {
    if (!booted && S.boot && S.screen === 'home') boot();
    if (!cur) return;
    if (!canShow(cur)) return leave(true);
    const spot = place(cur.at || 'bar', cur.pose) || place('bar', cur.pose);
    if (!spot) return leave(true);
    cur.spot = { ...spot, enter: cur.spot.enter };
    position();
  }
  let raf = 0;
  const resync = () => { cancelAnimationFrame(raf); raf = requestAnimationFrame(sync); };
  window.addEventListener('resize', resync);
  document.addEventListener('scroll', resync, true);
  document.addEventListener('pointerdown', (e) => { if (!e.target.closest?.('.msc-bub')) input(); }, true);
  document.addEventListener('wheel', input, { passive: true, capture: true });
  setInterval(tick, 5000);

  // Settings → Look → "Say hi": one of her drop-ins right now.
  function demo() {
    if (cur) leave(true);
    lastInput = Date.now();
    setTimeout(() => { if (Math.random() < 0.3) welcome(); else ambient(); }, 350);
  }

  return { _show: show, boot, sync, input, played, spun, demo, leave, setLevel: () => { if (!on()) leave(true); planDrop(); }, POSE };
})();
