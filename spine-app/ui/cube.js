// Cube: a Home style built around a hollow cube of glowing data lines. The
// chosen console floats inside it as a holographic panel; left/right turn the
// cube to the previous/next console (wrapping round), up shows Recent and down
// Favorites. The previous and next consoles wait as small panels on the floor.
// Below, the chosen console's games sit on a curved shelf.
// The frame is drawn on two canvases (edges behind the panel, edges in front of
// it) with canvas glow, never CSS filters or preserve-3d: Linux WebKitGTK
// without a GPU flattens 3D and makes live blur slow. Settings → Home style → Cube.
// No music; Cube has its own interface sounds (SFX in app.js). Loaded before app.js.

const cubeOn = () => settings().home_style === 'cube';
const CB = { gap: 168, window: 8, u: 1 }; // shelf spacing (CSS px), games drawn each side, last frame scale (frame size / 560)

// The ring of consoles (everything except Recent and Favorites, which are up and down).
function cubeBelt() { return waveCats().filter((c) => c.kind !== 'recent' && c.kind !== 'fav'); }
function cubeTop() { const c = waveCats()[0]; return c && c.kind === 'recent' ? c : null; }
function cubeBottom() { const c = waveCats().find((x) => x.kind === 'fav'); return c || null; }

function cubeState() {
  if (!S.cube) {
    const belt = cubeBelt();
    const i = Math.max(0, belt.findIndex((c) => c.ti === S.sel));
    S.cube = { i, rot: 0, tilt: 0, zone: 'cube', at: {}, intro: true, dir: '' };
  }
  const belt = cubeBelt();
  S.cube.i = Math.min(S.cube.i, belt.length - 1);
  const cat = S.cube.tilt > 0 ? cubeTop() : S.cube.tilt < 0 ? cubeBottom() : belt[S.cube.i];
  return { belt, cat: cat || belt[S.cube.i] };
}
const mod = (a, n) => ((a % n) + n) % n;
const cubeStill = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

// ---- colors ---------------------------------------------------------------
const cubeRgbCtx = document.createElement('canvas').getContext('2d');
function cubeRgb(str) { // any CSS color -> [r, g, b]
  cubeRgbCtx.fillStyle = '#000'; cubeRgbCtx.fillStyle = str || '#000';
  const v = cubeRgbCtx.fillStyle;
  if (v[0] === '#') { const n = parseInt(v.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; }
  const m = v.match(/[\d.]+/g) || [0, 0, 0]; return [+m[0], +m[1], +m[2]];
}
const cubeMix = (a, b, t) => a.map((x, k) => Math.round(x + (b[k] - x) * t));
const cubeCss = (c, a = 1) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;
const cubeHex = (c) => '#' + c.map((x) => x.toString(16).padStart(2, '0')).join('');
// The console color and its light partner (lines, glow); on light themes the partner goes darker instead.
function cubeColors(color) {
  const bg = cubeRgb(getComputedStyle(document.documentElement).getPropertyValue('--bg'));
  const light = (bg[0] * 0.299 + bg[1] * 0.587 + bg[2] * 0.114) > 140;
  const c1 = cubeRgb(color);
  return { light, c1, c2: light ? cubeMix(c1, [0, 0, 0], 0.2) : cubeMix(c1, [255, 255, 255], 0.5) };
}

// ---- what each console shows ----------------------------------------------
function cubeItems(c) {
  if (!c || c.kind === 'stream') return [];
  return c.kind === 'recent' ? c.recent : c.kind === 'fav' ? favItems() : c.kind === 'coll' ? collItems(c.coll)
    : c.kind === 'sys' ? c.sys.games.map((g) => ({ s: c.sys, g })) : systems().filter((x) => !isMovies(x)).flatMap((x) => x.games.map((g) => ({ s: x, g })));
}
function cubeCount(c) {
  return c.kind === 'recent' ? `${c.recent.length} recently played` : c.count != null ? `${c.count.toLocaleString()} ${c.kind === 'sys' ? things(c.sys, c.count) : c.count === 1 ? 'game' : 'games'}` : c.label || 'Random pick';
}
// The floating panel: two faint ghost layers, a cover behind and one in front, the panel itself.
function cubeHoloHtml(c, dir = '') {
  if (!c) return `<div class="cb-hin"><div class="cb-panel empty"><span class="nm">Nothing here yet</span></div></div>`;
  const covers = cubeItems(c).filter((x) => x.g.cover);
  const step = Math.max(1, Math.floor(covers.length / 2));
  const pick = covers.filter((_, k) => k % step === 0).slice(0, 2);
  const cov = (x, k) => x ? `<img class="cb-cov cv${k}" src="${mediaUrl(x.g.cover)}" alt="" decoding="async">` : '';
  return `<div class="cb-hin ${dir ? 'in-' + dir : ''}">
      <span class="cb-ghost g1"></span><span class="cb-ghost g2"></span>${cov(pick[0], 0)}
      <div class="cb-panel"><span class="cb-scan"></span><i class="cb-br tl"></i><i class="cb-br tr"></i><i class="cb-br bl"></i><i class="cb-br br"></i>
        <span class="ic">${sysIcon(tileIcon(c), Math.round(64 * CB.u))}</span>
        <span class="nm">${esc(c.name)}</span><span class="ct">${esc(cubeCount(c))}</span></div>${cov(pick[1], 1)}
    </div>`;
}
// Shrink the console name until it fits the panel (at most two lines, no word cut).
function cubeFitName() {
  const el = document.querySelector('#cbholo .cb-panel .nm');
  if (!el) return;
  let size = 34 * CB.u;
  el.style.fontSize = size + 'px';
  const tooBig = () => el.scrollWidth > el.clientWidth + 1 || el.offsetHeight > size * 1.1 * 2 + 2;
  for (let k = 0; k < 14 && tooBig() && size > 14; k++) { size *= 0.92; el.style.fontSize = size + 'px'; }
}
// The two consoles waiting on the floor, and the Recent / Favorites tags on the frame.
function cubeSideHtml(belt) {
  const t = S.cube.tilt, n = belt.length;
  const side = (d) => {
    if (t || n < 2) return '';
    const k = mod(S.cube.i + d, n), c = belt[k];
    return `<button class="cb-floor ${d < 0 ? 'prev' : 'next'}" style="--fc:${c.color}" data-act="cubepick" data-i="${k}" tabindex="-1">
      <span class="fn">${esc(c.short || c.name)}</span><span class="fd">${d < 0 ? '◀ previous' : 'next ▶'}</span></button>`;
  };
  const top = cubeTop(), bot = cubeBottom(), here = belt[S.cube.i];
  const up = t < 0 ? here : t === 0 ? top : null, down = t > 0 ? here : t === 0 ? bot : null;
  const tag = (c, cls, ch, to) => c ? `<button class="cb-tag ${cls}" data-act="cubetilt" data-t="${to}" tabindex="-1">${ch} ${esc(c.short || c.name)}</button>` : '';
  return side(-1) + side(1) + tag(up, 'up', '▲', t < 0 ? 0 : 1) + tag(down, 'down', '▼', t > 0 ? 0 : -1);
}

function cubeShelfHtml(cat) {
  const items = waveItems(cat);
  const at = Math.min(S.cube.at[cat.id] || 0, items.length - 1);
  return items.map((it, k) => {
    const art = it.g ? (it.g.cover ? `<img data-src="${mediaUrl(it.g.cover)}" alt="">` : `<span class="ph" style="background:${PAL[hash(it.g.title) % PAL.length]}">${esc(it.g.title)}</span>`)
      : `<span class="ph icon" style="background:${cat.color}">${sysIcon(it.random ? 'random' : tileIcon(cat), 64)}</span>`;
    return `<button class="cb-game" data-nav data-key="${esc(it.key)}" ${it.attrs} data-k="${k}" style="${cubeGameStyle(k - at)}">
      <span class="art">${art}</span><span class="t">${esc(it.title)}</span></button>`;
  }).join('');
}
// Position on the curved shelf for a game `off` places from the middle.
function cubeGameStyle(off) {
  const a = Math.abs(off), s = Math.sign(off);
  if (a > CB.window) return `transform:translateX(${s * (CB.window + 1) * CB.gap}px) translateZ(-400px);opacity:0;visibility:hidden`;
  const x = off === 0 ? 0 : s * (60 + a * 118);
  return `transform:translateX(${x}px) translateZ(${-Math.min(a, 6) * 46}px) rotateY(${-s * Math.min(a * 22, 48)}deg) scale(${off === 0 ? 1.18 : 1});opacity:${Math.max(0.25, 1 - a * 0.1)};z-index:${20 - a}`;
}

function cubePanelAct(belt) { return S.cube.tilt ? `data-act="cubetilt" data-t="${S.cube.tilt}"` : `data-act="cubepick" data-i="${S.cube.i}"`; }
function cubeVars(cat) {
  const { c2 } = cubeColors(cat.color);
  return `--cc:${cat.color};--c2:${cubeHex(c2)}`;
}

function homeCube() {
  const { belt, cat } = cubeState();
  const c = S.cube;
  const intro = c.intro; c.intro = false;
  S.cube.shown = cat.id;
  return topbar() + `<main class="main cubehome ${cubeColors(cat.color).light ? 'light' : ''} ${c.zone === 'shelf' ? 'onshelf' : ''} ${intro ? 'intro' : ''}" id="cubehome" style="${cubeVars(cat)}">
      <canvas class="cb-art" id="cbart0" aria-hidden="true"></canvas><canvas class="cb-art" id="cbart1" aria-hidden="true"></canvas>
      <div class="cb-tint" aria-hidden="true"></div>
      <canvas class="cb-lines" id="cbback" aria-hidden="true"></canvas>
      <div class="cb-stage" id="cbstage">
        <span class="cb-beam" aria-hidden="true"></span><span class="cb-disc" aria-hidden="true"></span>
        <div class="cb-side" id="cbside">${cubeSideHtml(belt)}</div>
        <div class="cb-holo" id="cbholo" ${cubePanelAct(belt)}>${cubeHoloHtml(cat)}</div>
        <button class="cb-hit" data-nav data-key="cb-cube" aria-label="${esc(cat.name)}"></button>
      </div>
      <canvas class="cb-lines front" id="cbfront" aria-hidden="true"></canvas>
      <div class="cb-shelf ${c.zone === 'shelf' ? 'live' : ''}" id="cbshelf">${cubeShelfHtml(cat)}</div>
    </main>` + hints(c.zone === 'shelf' ? '↑ or B: back to the cube' : '← → turn · ↑ Recent · ↓ Favorites', c.zone === 'shelf' ? [['A', 'Open'], ['Y', 'Favorite'], ['X', 'Search'], ['☰', 'Settings']] : [['A', 'Open'], ['X', 'Search'], ['☰', 'Settings']]);
}

// ---- the frame ------------------------------------------------------------
// A cube of half-size 1 seen from straight in front, a little from above
// (numbers match the approved mockup: front square 560 px, back square 348 px).
// The frame turns -90° round the vertical per console (rot) and ±90° round the
// horizontal for Recent / Favorites (tilt). A turned cube looks the same, so
// only the in-between frames show the turn.
const CUBE_D = 4.284, CUBE_EYE = -0.186;
const CUBE_V = [[-1, -1, -1], [1, -1, -1], [1, 1, -1], [-1, 1, -1], [-1, -1, 1], [1, -1, 1], [1, 1, 1], [-1, 1, 1]];
const CUBE_E = [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]];
const CG = { y: 0, x: 0, fromY: 0, fromX: 0, t0: 0, raf: 0, geo: null, col: null, dots: null };

function cubeProject(p, g) {
  const k = g.fs * 1.642, s = k / (CUBE_D - p[2]);
  return [g.cx + p[0] * s, g.cy - g.fs * 0.093 + (p[1] - CUBE_EYE) * s];
}
function cubeTurn(v, ay, ax) {
  const a = -ay * Math.PI / 2, b = -ax * Math.PI / 2;
  const x1 = v[0] * Math.cos(a) + v[2] * Math.sin(a), z1 = -v[0] * Math.sin(a) + v[2] * Math.cos(a);
  return [x1, v[1] * Math.cos(b) - z1 * Math.sin(b), v[1] * Math.sin(b) + z1 * Math.cos(b)];
}
function cubeCanvas(id, g) {
  const cv = document.getElementById(id);
  if (!cv) return null;
  const w = Math.round(g.w * g.scale), h = Math.round(g.h * g.scale);
  if (cv.width !== w || cv.height !== h) { cv.width = w; cv.height = h; }
  const ctx = cv.getContext('2d');
  ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, w, h);
  ctx.setTransform(g.scale, 0, 0, g.scale, 0, 0);
  return ctx;
}
function cubeDraw() {
  const g = CG.geo, col = CG.col;
  if (!g || !col) return;
  const back = cubeCanvas('cbback', g), front = cubeCanvas('cbfront', g);
  if (!back || !front) return;
  const { c1, c2, light } = col, dim = S.cube && S.cube.zone === 'shelf' ? 0.6 : 1;
  // Data points scattered over the background.
  if (!CG.dots) { let seed = 7; const r = () => (seed = (seed * 9301 + 49297) % 233280) / 233280; CG.dots = Array.from({ length: 70 }, () => [r(), r(), r()]); }
  for (const [x, y, t] of CG.dots) {
    back.fillStyle = t > 0.5 ? cubeCss(c2, (0.15 + t * 0.4) * (light ? 0.8 : 1)) : light ? cubeCss(c1, 0.12 + t * 0.2) : `rgba(255,255,255,${0.15 + t * 0.4})`;
    back.beginPath(); back.arc(x * g.w, 8 + y * (g.h - 16), t > 0.8 ? 1.5 : 1, 0, Math.PI * 2); back.fill();
  }
  // The floor: a grid that stays put while the frame turns above it.
  const fl = (x, z) => cubeProject([x, 1, z], g);
  back.fillStyle = cubeCss(c1, 0.12);
  back.beginPath(); [[-1, -1], [1, -1], [1, 1], [-1, 1]].forEach(([x, z], k) => { const p = fl(x, z); k ? back.lineTo(p[0], p[1]) : back.moveTo(p[0], p[1]); }); back.fill();
  back.strokeStyle = cubeCss(c2, 0.22); back.lineWidth = 1;
  const seg = (a, b) => { back.beginPath(); back.moveTo(a[0], a[1]); back.lineTo(b[0], b[1]); back.stroke(); };
  [-0.5, 0, 0.5].forEach((x) => seg(fl(x, -1), fl(x, 1)));
  [-1 / 3, 1 / 3].forEach((z) => seg(fl(-1, z), fl(1, z)));
  // The frame. Edges nearer than the middle go on the front canvas (over the panel).
  const pts = CUBE_V.map((v) => cubeTurn(v, CG.y, CG.x));
  const u = g.fs / 560, glow = (light ? 6 : 10) * u * g.scale;
  const edges = CUBE_E.map(([a, b]) => ({ a: pts[a], b: pts[b], z: (pts[a][2] + pts[b][2]) / 2 })).sort((p, q) => p.z - q.z);
  for (const e of edges) {
    const ctx = e.z > 0.05 ? front : back, near = (e.z + 1) / 2; // 0 at the back .. 1 at the front
    const A = cubeProject(e.a, g), B = cubeProject(e.b, g);
    ctx.lineCap = 'round';
    ctx.shadowBlur = 0; ctx.strokeStyle = cubeCss(c1, (light ? 0.28 : 0.35) * dim); ctx.lineWidth = (2 + near * 2) * u;
    ctx.beginPath(); ctx.moveTo(A[0], A[1]); ctx.lineTo(B[0], B[1]); ctx.stroke();
    ctx.shadowBlur = glow; ctx.shadowColor = cubeCss(light ? c1 : c2, light ? 0.6 : 1);
    ctx.strokeStyle = cubeCss(c2, (0.7 + near * 0.3) * dim); ctx.lineWidth = (1.2 + near * 0.8) * u;
    ctx.beginPath(); ctx.moveTo(A[0], A[1]); ctx.lineTo(B[0], B[1]); ctx.stroke();
  }
  // Small bright nodes at the corners.
  pts.forEach((p) => {
    const ctx = p[2] > 0.05 ? front : back, q = cubeProject(p, g);
    ctx.shadowBlur = glow; ctx.shadowColor = cubeCss(light ? c1 : c2, light ? 0.6 : 1);
    ctx.fillStyle = light ? cubeCss(c2, dim) : `rgba(255,255,255,${0.9 * dim})`;
    ctx.beginPath(); ctx.arc(q[0], q[1], (1.6 + (p[2] + 1)) * u, 0, Math.PI * 2); ctx.fill();
  });
  front.shadowBlur = 0; back.shadowBlur = 0;
}
// Ease towards the cube's current rot/tilt. A new turn mid-way starts from where the frame is.
const cubeEase = (t) => { const c = 1.35; return 1 + (c + 1) * Math.pow(t - 1, 3) + c * Math.pow(t - 1, 2); };
function cubeAnimate(fromY, fromX) {
  const ty = S.cube.rot, tx = S.cube.tilt;
  cancelAnimationFrame(CG.raf);
  if (cubeStill() || (CG.y === ty && CG.x === tx && fromY == null)) { CG.y = ty; CG.x = tx; return cubeDraw(); }
  CG.fromY = fromY ?? CG.y; CG.fromX = fromX ?? CG.x; CG.t0 = performance.now();
  const step = (now) => {
    if (!document.getElementById('cbback')) return;
    const t = Math.min(1, (now - CG.t0) / 520), e = cubeEase(t);
    CG.y = CG.fromY + (S.cube.rot - CG.fromY) * e; CG.x = CG.fromX + (S.cube.tilt - CG.fromX) * e;
    if (t >= 1) { CG.y = S.cube.rot; CG.x = S.cube.tilt; }
    cubeDraw();
    if (t < 1) CG.raf = requestAnimationFrame(step);
  };
  CG.raf = requestAnimationFrame(step);
}

// Size everything from the room the stage gets (CSS px, so it works at any Interface size).
function cubeLayout() {
  const home = document.getElementById('cubehome'), stage = document.getElementById('cbstage');
  if (!home || !stage) return;
  const z = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--z')) || 1;
  const fs = Math.max(240, Math.min(stage.offsetHeight * 0.9, stage.offsetWidth * 0.55, 620));
  const u = fs / 560, changed = Math.abs(u - CB.u) > 0.08;
  CB.u = u;
  home.style.setProperty('--u', u.toFixed(4));
  CG.geo = { w: home.offsetWidth, h: home.offsetHeight, fs, scale: Math.min(3, z * (window.devicePixelRatio || 1)),
    cx: stage.offsetLeft + stage.offsetWidth / 2, cy: stage.offsetTop + stage.offsetHeight / 2 };
  if (changed) { const { cat } = cubeState(); const h = document.getElementById('cbholo'); if (h) h.innerHTML = cubeHoloHtml(cat); }
  cubeFitName();
  cubeDraw();
}

// ---- the blurred background art ---------------------------------------------
// The picture is drawn into a tiny canvas and stretched by the browser, which
// blurs it for free; two canvases crossfade when the console changes.
let cubeArtN = 0, cubeArtFor = '';
function cubeArt(cat) {
  if (cubeArtFor === cat.id) return;
  cubeArtFor = cat.id;
  const items = cubeItems(cat).slice(0, 40);
  const g = (items.find((x) => x.g.hero) || items.find((x) => x.g.cover) || {}).g;
  const src = g ? mediaUrl(g.hero || g.cover) : null;
  const show = (cv) => { document.querySelectorAll('#cubehome .cb-art').forEach((el) => el.classList.toggle('on', el === cv)); };
  if (!src) return show(null);
  const img = new Image();
  img.decoding = 'async';
  img.onload = () => {
    if (cubeArtFor !== cat.id) return;
    const home = document.getElementById('cubehome');
    const cv = document.getElementById(`cbart${cubeArtN = 1 - cubeArtN}`);
    if (!home || !cv) return;
    // Shrink to 48 px wide, stretch to 384 px, then soften by averaging shifted copies
    // (a box blur, run twice each way). Linux WebKitGTK scales canvases without
    // smoothing, so the blur can't come from scaling; shifting works everywhere.
    const asp = home.offsetHeight / Math.max(1, home.offsetWidth);
    const w0 = 48, h0 = Math.max(12, Math.round(48 * asp)), W = w0 * 8, H = h0 * 8;
    const small = document.createElement('canvas'); small.width = w0; small.height = h0;
    const r = Math.max(w0 / img.naturalWidth, h0 / img.naturalHeight), dw = img.naturalWidth * r, dh = img.naturalHeight * r;
    small.getContext('2d').drawImage(img, (w0 - dw) / 2, (h0 - dh) / 2, dw, dh);
    const a = document.createElement('canvas'), b = document.createElement('canvas');
    a.width = b.width = W; a.height = b.height = H;
    const actx = a.getContext('2d'), bctx = b.getContext('2d');
    actx.imageSmoothingEnabled = true; actx.drawImage(small, -8, -8, W + 16, H + 16);
    const pass = (from, to, ctx, dx, dy) => {
      ctx.clearRect(0, 0, W, H);
      for (let k = 0; k < 9; k++) { const o = (k - 4) * 2; ctx.globalAlpha = 1 / (k + 1); ctx.drawImage(from, o * dx, o * dy); }
      ctx.globalAlpha = 1;
    };
    for (let n = 0; n < 2; n++) { pass(a, b, bctx, 1, 0); pass(b, a, actx, 0, 1); }
    cv.width = W; cv.height = H;
    cv.getContext('2d').drawImage(a, 0, 0);
    show(cv);
  };
  img.src = src;
}

// Update everything in place so turns animate. `dir` (S.cube.dir) says where the new panel comes from.
function cubeSync(catChanged) {
  const { belt, cat } = cubeState();
  const home = document.getElementById('cubehome'), shelf = document.getElementById('cbshelf'), holo = document.getElementById('cbholo');
  if (!home || !shelf || !holo) return render();
  const fromY = CG.y, fromX = CG.x;
  if (S.cube.shown !== cat.id) {
    S.cube.shown = cat.id;
    holo.innerHTML = cubeHoloHtml(cat, cubeStill() ? '' : S.cube.dir);
    cubeFitName();
    home.setAttribute('style', cubeVars(cat) + `;--u:${CB.u.toFixed(4)}`);
    CG.col = cubeColors(cat.color);
    cubeArt(cat);
  }
  S.cube.dir = '';
  holo.removeAttribute('data-i'); holo.removeAttribute('data-t');
  if (S.cube.tilt) { holo.dataset.act = 'cubetilt'; holo.dataset.t = S.cube.tilt; } else { holo.dataset.act = 'cubepick'; holo.dataset.i = S.cube.i; }
  document.getElementById('cbside').innerHTML = cubeSideHtml(belt);
  const hit = document.querySelector('.cb-hit'); if (hit) hit.setAttribute('aria-label', cat.name);
  home.classList.toggle('onshelf', S.cube.zone === 'shelf');
  home.classList.toggle('light', CG.col.light);
  if (catChanged) shelf.innerHTML = cubeShelfHtml(cat);
  cubeShelfPlace(cat);
  shelf.classList.toggle('live', S.cube.zone === 'shelf');
  if (cat.ti != null) S.sel = cat.ti;
  cubeAnimate(fromY, fromX);
}
function cubeShelfPlace(cat) {
  const shelf = document.getElementById('cbshelf');
  if (!shelf) return;
  const at = S.cube.at[cat.id] || 0;
  shelf.querySelectorAll('.cb-game').forEach((el) => {
    const off = +el.dataset.k - at;
    el.style.cssText = cubeGameStyle(off);
    el.classList.toggle('sel', off === 0);
    if (Math.abs(off) <= CB.window + 1) { const img = el.querySelector('img[data-src]'); if (img) { img.src = img.dataset.src; img.removeAttribute('data-src'); } }
  });
  if (S.cube.zone === 'shelf') focusEl(shelf.children[at], false, false);
  else { const hit = document.querySelector('.cb-hit'); if (hit) focusEl(hit, false, false); }
}
const cubeFocusKey = () => { const { cat } = cubeState(); return S.cube.zone === 'shelf' ? waveItems(cat)[S.cube.at[cat.id] || 0]?.key : 'cb-cube'; };

function cubeRoll(d) {
  const { belt } = cubeState();
  if (belt.length < 2 && !S.cube.tilt) { SFX.play('edge'); return; }
  if (S.cube.tilt) { S.cube.dir = S.cube.tilt > 0 ? 'd' : 'u'; S.cube.tilt = 0; SFX.play('cubeL'); return cubeSync(true); } // level out first
  S.cube.rot += d; S.cube.i = mod(S.cube.i + d, belt.length);
  S.cube.dir = d > 0 ? 'r' : 'l';
  SFX.play(d > 0 ? 'cubeR' : 'cubeL');
  cubeSync(true);
}
function cubeTilt(t) {
  const want = Math.max(-1, Math.min(1, S.cube.tilt + t));
  if (want === S.cube.tilt || (want > 0 && !cubeTop()) || (want < 0 && !cubeBottom())) return false;
  S.cube.dir = want > S.cube.tilt ? 'u' : 'd';
  S.cube.tilt = want;
  SFX.play('cubeR');
  cubeSync(true);
  return true;
}

// Controller / keyboard on the Cube home. Returns true when it handled the button.
function cubeKey(btn) {
  const cur = document.activeElement;
  const inCube = cur?.classList?.contains('cb-hit'), inShelf = cur?.classList?.contains('cb-game');
  if (!inCube && !inShelf) { // focus is up in the top bar
    if (btn === 'down') { S.cube.zone = 'cube'; const hit = document.querySelector('.cb-hit'); if (hit) { focusEl(hit, false, false); document.getElementById('cubehome')?.classList.remove('onshelf'); cubeDraw(); SFX.play('move'); return true; } }
    return false;
  }
  const { cat } = cubeState();
  if (S.cube.zone === 'cube') {
    if (btn === 'left' || btn === 'LB') return cubeRoll(-1), true;
    if (btn === 'right' || btn === 'RB') return cubeRoll(1), true;
    if (btn === 'up') return cubeTilt(1) || (S.cube.tilt > 0 ? false : (SFX.play('edge'), true)); // past the top: up to the top bar
    if (btn === 'down') { if (!cubeTilt(-1)) { S.cube.zone = 'shelf'; SFX.play('move'); cubeSync(false); } return true; }
    if (btn === 'A') { S.cube.zone = 'shelf'; SFX.play('select'); cubeSync(false); return true; }
    return false;
  }
  // On the shelf.
  const n = waveItems(cat).length, at = S.cube.at[cat.id] || 0;
  if (btn === 'left' || btn === 'right') {
    const j = at + (btn === 'right' ? 1 : -1);
    if (j < 0 || j >= n) { SFX.play('edge'); return true; }
    S.cube.at[cat.id] = j; SFX.play('move'); cubeShelfPlace(cat); return true;
  }
  if (btn === 'LT' || btn === 'RT') { S.cube.at[cat.id] = Math.max(0, Math.min(n - 1, at + (btn === 'RT' ? 10 : -10))); cubeShelfPlace(cat); return true; }
  if (btn === 'up' || btn === 'B') { S.cube.zone = 'cube'; SFX.play('back'); cubeSync(false); return true; }
  if (btn === 'LB' || btn === 'RB') { S.cube.zone = 'cube'; cubeRoll(btn === 'RB' ? 1 : -1); return true; }
  return false;
}

// Mouse: a click on a side game slides it to the middle; a click on the middle one opens it.
function cubeClick(el) {
  if (!el.classList.contains('cb-game') || el.classList.contains('sel')) return false;
  const { cat } = cubeState();
  S.cube.zone = 'shelf'; S.cube.at[cat.id] = +el.dataset.k; SFX.play('move'); cubeShelfPlace(cat);
  document.getElementById('cubehome')?.classList.add('onshelf'); cubeDraw();
  return true;
}
let cubeResizeOn = false;
function cubeMount() {
  const home = document.getElementById('cubehome');
  if (!home) { cancelAnimationFrame(CG.raf); return; }
  const { cat } = cubeState();
  CG.col = cubeColors(cat.color);
  cubeArtFor = '';
  cubeLayout();
  document.fonts?.ready.then(cubeFitName); // the display font may arrive after the first fit
  cubeArt(cat);
  // The intro: the frame swings in from a little off to the side.
  if (home.classList.contains('intro') && !cubeStill()) cubeAnimate(S.cube.rot - 0.6, S.cube.tilt + 0.25);
  else { CG.y = S.cube.rot; CG.x = S.cube.tilt; cubeDraw(); }
  cubeShelfPlace(cat);
  if (!cubeResizeOn) { cubeResizeOn = true; window.addEventListener('resize', () => cubeLayout()); }
  home.onwheel = (e) => {
    e.preventDefault();
    const now = performance.now(); if (now - (cubeMount.last || 0) < 120) return; cubeMount.last = now;
    const d = (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY) > 0 ? 1 : -1;
    if (e.target.closest('.cb-shelf')) { const { cat: c } = cubeState(); const n = waveItems(c).length; S.cube.zone = 'shelf'; S.cube.at[c.id] = Math.max(0, Math.min(n - 1, (S.cube.at[c.id] || 0) + d)); SFX.play('move'); cubeShelfPlace(c); home.classList.add('onshelf'); cubeDraw(); }
    else cubeRoll(d);
  };
}
