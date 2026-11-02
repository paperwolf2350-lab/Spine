// Wave: a Home style with consoles across and games down, over a slowly moving
// wave (Settings → Home style). Game, movie and show pages are shared with the
// Classic home; only the Home screen differs. Loaded before app.js, so it only
// uses app.js helpers inside functions (they exist by the time these run).

const waveOn = () => settings().home_style === 'wave';

// Spacing in CSS pixels (before Interface size zoom). Keep in sync with wave.css.
const WV = { col: 168, row: 84, gap: 64, catRow: 150 };

// Categories across the top: Recently played (when there is any), then the
// same tiles as Classic home (Favorites, lists, consoles, Movies, TV, Surprise me).
function waveCats() {
  const lp = S.boot.last_played || {};
  const recent = Object.entries(lp).sort((a, b) => b[1] - a[1]).map(([id]) => findGame(id)).filter(Boolean).slice(0, 12);
  const T = tiles().map((t, i) => ({ ...t, ti: i }));
  return recent.length ? [{ kind: 'recent', id: 'recent', name: 'Recently played', short: 'Recent', color: '#4F5D7A', recent }, ...T] : T;
}

function waveState() {
  S.wave = S.wave || { cat: null, at: {} };
  const cats = waveCats();
  let i = cats.findIndex((c) => c.id === S.wave.cat);
  if (i < 0) i = Math.max(0, cats.findIndex((c) => c.ti === S.sel && c.kind !== 'recent'));
  S.wave.cat = cats[i].id;
  return { cats, i, cat: cats[i] };
}

function waveThumb(g, s) {
  if (g.cover) return `<span class="wv-thumb"><img src="${mediaUrl(g.cover)}" alt="" loading="lazy" decoding="async"></span>`;
  return `<span class="wv-thumb ph" style="background:${PAL[hash(g.title) % PAL.length]}"><span>${esc(g.title.slice(0, 2))}</span></span>`;
}

// One line under a selected game: what you'd want to know before opening it.
function waveSub(s, g, withSystem) {
  const lp = (S.boot.last_played || {})[g.id];
  const parts = [];
  if (withSystem) parts.push(s.short);
  if (isMovies(s)) {
    if (g.episodes?.length) parts.push(`${g.episodes.length} episode${g.episodes.length === 1 ? '' : 's'}`);
    else { const p = progOf(g, null); parts.push(p?.watched ? 'Watched' : canResume(p) ? `Resume at ${clock(p.pos)}` : 'Movie'); }
  } else parts.push(fmtTime((S.boot.playtime || {})[g.id]));
  if (lp) parts.push(ago(lp));
  return parts.join(' · ');
}

// The list under a category. Each entry: { key, attrs, thumb, title, sub }.
function waveItems(c) {
  const game = ({ s, g }, withSystem) => ({
    key: `wv-${c.id}-${g.id}`, attrs: `data-act="open" data-id="${esc(g.id)}" data-from="home"`,
    thumb: waveThumb(g, s), title: g.title, sub: waveSub(s, g, withSystem), off: g.offline, fav: isFav(g.id),
  });
  const opener = (title, sub) => ({
    key: `wv-${c.id}-open`, attrs: `data-act="tile" data-i="${c.ti}"`,
    thumb: `<span class="wv-thumb icon" style="background:${c.color}">${sysIcon(tileIcon(c), 44)}</span>`, title, sub,
  });
  if (c.kind === 'recent') return c.recent.map((x) => game(x, true));
  if (c.kind === 'random') {
    const n = systems().filter((x) => !isMovies(x)).reduce((k, x) => k + x.games.length, 0);
    return [{ key: 'wv-random-go', attrs: 'data-act="random" data-scope="all"', thumb: `<span class="wv-thumb icon" style="background:${c.color}">${sysIcon('random', 44)}</span>`,
      title: 'Surprise me', sub: `Spine picks one of your ${n.toLocaleString()} games` }];
  }
  if (c.kind === 'fav') return [opener('All favorites', `${c.count} in the A–Z library view`), ...favItems().map((x) => game(x, true))];
  if (c.kind === 'coll') return [opener('Open list', `${c.count} in the library view`), ...collItems(c.coll).map((x) => game(x, true))];
  const s = c.sys;
  const how = s.blocked ? s.blocked : s.emulator === 'system' ? 'Opens with their store or your PC' : `${isMovies(s) ? 'Plays in' : 'Launches with'} ${s.emulator}`;
  return [opener(`All ${c.count.toLocaleString()} ${things(s, c.count)}`, how), ...s.games.map((g) => game({ s, g }, false))];
}

function waveItemsHtml(c, at) {
  return waveItems(c).map((it, k) => `<button class="wv-item ${k < at ? 'above' : k === at ? 'sel' : 'below'}${it.off ? ' off' : ''}" style="top:${k * WV.row}px" data-nav data-key="${esc(it.key)}" ${it.attrs}>
      ${it.thumb}<span class="wv-text"><span class="t">${esc(it.title)}${it.fav ? ` <span class="wv-fav">${ICON.heart}</span>` : ''}</span><span class="s">${esc(it.sub || '')}</span></span></button>`).join('');
}

function homeWave() {
  const { cats, i, cat } = waveState();
  const at = Math.min(S.wave.at[cat.id] || 0, waveItems(cat).length - 1);
  S.wave.at[cat.id] = at;
  const catsHtml = cats.map((c, k) => `<button class="wv-cat ${k === i ? 'sel' : ''}" style="left:${k * WV.col}px;--chip:${c.color}" data-act="wavecat" data-i="${k}" tabindex="-1" aria-label="${esc(c.name)}">
      <span class="wv-icon">${sysIcon(tileIcon(c), 64)}</span>
      <span class="wv-label">${esc(c.short)}</span></button>`).join('');
  const offline = (S.boot.roots || []).filter((r) => !r.online).length;
  return topbar() + `<main class="main wave" id="wave" style="--wc:${cat.color}">
      <canvas class="wave-bg" id="wavebg" aria-hidden="true"></canvas>
      <div class="wave-stage">
        <div class="wave-cats" id="wavecats" style="transform:translateX(${-i * WV.col}px)">${catsHtml}</div>
        <div class="wave-items" id="waveitems" style="transform:translateY(${-at * WV.row}px)">${waveItemsHtml(cat, at)}</div>
      </div>
      ${offline ? `<div class="wave-note">${offline === 1 ? 'A drive is' : `${offline} drives are`} not connected; its games are greyed out.</div>` : ''}
    </main>` + hints('← → consoles · ↑ ↓ games', [['A', 'Open'], ['Y', 'Favorite'], ['X', 'Search'], ['☰', 'Settings']]);
}

// Move without rebuilding the page, so the slide animations play.
function waveSync(catChanged) {
  const { cats, i, cat } = waveState();
  const box = document.getElementById('waveitems'), row = document.getElementById('wavecats'), wave = document.getElementById('wave');
  if (!box || !row) return render();
  const at = S.wave.at[cat.id] || 0;
  row.style.transform = `translateX(${-i * WV.col}px)`;
  row.querySelectorAll('.wv-cat').forEach((el, k) => el.classList.toggle('sel', k === i));
  wave.style.setProperty('--wc', cat.color);
  if (catChanged) {
    box.classList.add('swap');
    box.style.transition = 'none';
    box.style.transform = `translateY(${-at * WV.row}px)`;
    box.innerHTML = waveItemsHtml(cat, at);
    void box.offsetHeight;
    box.style.transition = '';
    requestAnimationFrame(() => box.classList.remove('swap'));
  } else {
    box.style.transform = `translateY(${-at * WV.row}px)`;
    box.querySelectorAll('.wv-item').forEach((el, k) => { el.classList.toggle('above', k < at); el.classList.toggle('sel', k === at); el.classList.toggle('below', k > at); });
  }
  const el = box.children[at];
  if (el) focusEl(el, false, false);
  if (cat.ti != null) S.sel = cat.ti;
}

const waveFocusKey = () => { const { cat } = waveState(); return waveItems(cat)[S.wave.at[cat.id] || 0]?.key; };

// Controller / keyboard on the Wave home. Returns true when it handled the button.
function waveKey(btn) {
  const { cats, i, cat } = waveState();
  const n = waveItems(cat).length;
  const at = S.wave.at[cat.id] || 0;
  if (btn === 'left' || btn === 'right' || btn === 'LB' || btn === 'RB') {
    const d = btn === 'left' || btn === 'LB' ? -1 : 1;
    const j = i + d;
    if (j < 0 || j >= cats.length) { if (btn === 'left' || btn === 'right') SFX.play('edge'); return true; }
    S.wave.cat = cats[j].id;
    if (btn === 'left' || btn === 'right') SFX.play('move');
    waveSync(true);
    return true;
  }
  if (btn === 'up' || btn === 'down') {
    const j = at + (btn === 'down' ? 1 : -1);
    if (j < 0 || j >= n) { SFX.play('edge'); return true; }
    S.wave.at[cat.id] = j;
    SFX.play('move');
    waveSync(false);
    return true;
  }
  if (btn === 'LT' || btn === 'RT') { // jump a page of games
    const j = Math.max(0, Math.min(n - 1, at + (btn === 'RT' ? 10 : -10)));
    if (j !== at) { S.wave.at[cat.id] = j; waveSync(false); }
    return true;
  }
  return false;
}

// A click on a game that isn't highlighted highlights it; a second click opens it.
function waveClick(el) {
  if (!el.classList.contains('wv-item') || el.classList.contains('sel')) return false;
  const { cat } = waveState();
  S.wave.at[cat.id] = [...el.parentNode.children].indexOf(el);
  SFX.play('move');
  waveSync(false);
  return true;
}

// Mouse wheel over the list scrolls the games; over the console row it changes console.
function waveWheel(e) {
  e.preventDefault();
  const now = performance.now();
  if (now - (waveWheel.last || 0) < 90) return;
  waveWheel.last = now;
  const sideways = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.target.closest('.wave-cats');
  const d = (sideways ? (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY) : e.deltaY) > 0;
  waveKey(sideways ? (d ? 'right' : 'left') : (d ? 'down' : 'up'));
}

// The moving wave behind everything: a few bands of thin sine lines, tinted by
// the theme's text color. One draw only when the system asks for reduced motion.
let waveRaf = 0;
function waveMount() {
  const wave = document.getElementById('wave');
  if (!wave) return;
  wave.onwheel = waveWheel;
  cancelAnimationFrame(waveRaf);
  const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let lastDraw = 0;
  const t0 = performance.now() - ((Date.now() / 1000) % 600) * 1000; // keeps the wave's phase across re-renders
  const frame = (now) => {
    const cv = document.getElementById('wavebg');
    if (!cv) return; // left Home
    waveRaf = still ? 0 : requestAnimationFrame(frame);
    if (document.hidden || S.playing || now - lastDraw < 33) return; // ~30 fps is plenty
    lastDraw = now;
    const z = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--z')) || 1;
    const dpr = Math.min(2, (window.devicePixelRatio || 1) * z);
    const w = cv.clientWidth, h = cv.clientHeight;
    if (!w || !h) return;
    if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) { cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr); }
    const g = cv.getContext('2d');
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    const ink = getComputedStyle(wave).color;
    const t = (now - t0) / 1000;
    const bands = [
      { y: 0.40, amp: 0.07, k: 1.6, sp: 0.11, lines: 14, spread: 0.035, a: 0.10 },
      { y: 0.47, amp: 0.05, k: 2.3, sp: -0.08, lines: 10, spread: 0.028, a: 0.08 },
      { y: 0.35, amp: 0.04, k: 3.1, sp: 0.05, lines: 6, spread: 0.02, a: 0.06 },
    ];
    g.strokeStyle = ink;
    g.lineWidth = 1;
    for (const b of bands) {
      for (let l = 0; l < b.lines; l++) {
        const off = (l - b.lines / 2) * b.spread;
        g.globalAlpha = b.a * (1 - Math.abs(l - b.lines / 2) / b.lines);
        g.beginPath();
        for (let x = 0; x <= w; x += 8) {
          const u = x / w;
          const y = h * (b.y + off * Math.sin(u * 2.4 + t * 0.3 + l * 0.2)
            + b.amp * Math.sin(u * Math.PI * b.k + t * b.sp * 6 + l * 0.07)
            + b.amp * 0.4 * Math.sin(u * Math.PI * b.k * 2.1 - t * b.sp * 4));
          x ? g.lineTo(x, y) : g.moveTo(x, y);
        }
        g.stroke();
      }
    }
    g.globalAlpha = 1;
  };
  waveRaf = requestAnimationFrame(frame);
}
