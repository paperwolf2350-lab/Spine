// Cube: a Home style with one big cube in the middle. The four faces around
// its middle show consoles in order (left/right rolls to the previous/next one,
// wrapping round at the ends); the top face is Recent and the bottom face is
// Favorites. Below it, the chosen console's games sit on a curved shelf.
// Only six faces exist: faces out of view are repainted with the next console
// in line before they can come round again. Settings → Home style → Cube.
// No music; Cube has its own interface sounds (SFX in app.js). Loaded before app.js.

const cubeOn = () => settings().home_style === 'cube';
const CB = { gap: 168, window: 8 }; // shelf spacing (CSS px) and how many games each side are drawn

// The ring of consoles (everything except Favorites, which lives on the bottom face).
function cubeBelt() { return waveCats().filter((c) => c.kind !== 'recent' && c.kind !== 'fav'); }
function cubeTop() { const c = waveCats()[0]; return c && c.kind === 'recent' ? c : null; }
function cubeBottom() { const c = waveCats().find((x) => x.kind === 'fav'); return c || null; }

function cubeState() {
  if (!S.cube) {
    const belt = cubeBelt();
    const i = Math.max(0, belt.findIndex((c) => c.ti === S.sel));
    S.cube = { i, rot: 0, tilt: 0, zone: 'cube', at: {}, intro: true };
  }
  const belt = cubeBelt();
  S.cube.i = Math.min(S.cube.i, belt.length - 1);
  const cat = S.cube.tilt > 0 ? cubeTop() : S.cube.tilt < 0 ? cubeBottom() : belt[S.cube.i];
  return { belt, cat: cat || belt[S.cube.i] };
}
const mod = (a, n) => ((a % n) + n) % n;
// Which console a side slot (0–3) shows: the front slot shows the current one,
// the slot to its right the next, to its left the previous, behind it two along.
function cubeSlotCat(belt, s) {
  const front = mod(S.cube.rot, 4);
  const d = [0, 1, 2, -1][mod(s - front, 4)];
  return { c: belt[mod(S.cube.i + d, belt.length)], k: mod(S.cube.i + d, belt.length) };
}

function cubeFaceHtml(c, extra = '') {
  if (!c) return `<span class="cb-in empty"><span class="nm">Nothing here yet</span></span>`;
  const items = c.kind === 'stream' ? [] : c.kind === 'recent' ? c.recent : c.kind === 'fav' ? favItems() : c.kind === 'coll' ? collItems(c.coll)
    : c.kind === 'sys' ? c.sys.games.map((g) => ({ s: c.sys, g })) : systems().filter((x) => !isMovies(x)).flatMap((x) => x.games.map((g) => ({ s: x, g })));
  const covers = items.filter((x) => x.g.cover);
  const step = Math.max(1, Math.floor(covers.length / 3));
  const stickers = covers.filter((_, k) => k % step === 0).slice(0, 3)
    .map((x, k) => `<img class="st st${k}" src="${mediaUrl(x.g.cover)}" alt="" decoding="async">`).join('');
  const count = c.kind === 'recent' ? `${c.recent.length} recently played` : c.count != null ? `${c.count.toLocaleString()} ${c.kind === 'sys' ? things(c.sys, c.count) : c.count === 1 ? 'game' : 'games'}` : c.label || 'Random pick';
  return `<span class="cb-in" style="--fc:${c.color}">${stickers}<span class="ic">${sysIcon(tileIcon(c), 92)}</span>
    <span class="nm">${esc(c.name)}</span><span class="ct">${esc(count)}</span>${extra}</span>`;
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

function homeCube() {
  const { belt, cat } = cubeState();
  const c = S.cube;
  const slots = [0, 1, 2, 3].map((s) => { const { c: sc, k } = cubeSlotCat(belt, s); return `<span class="cb-face s${s}" data-f="s${s}" style="transform:${cubeFaceTransform(`s${s}`)}" data-act="cubepick" data-i="${k}">${cubeFaceHtml(sc)}</span>`; }).join('');
  const intro = c.intro; c.intro = false;
  const dots = belt.map((b, k) => `<button class="cb-dot ${k === c.i && !c.tilt ? 'on' : ''}" style="--dc:${b.color}" data-act="cubepick" data-i="${k}" tabindex="-1" aria-label="${esc(b.name)}">${sysIcon(tileIcon(b), 26)}</button>`).join('');
  const drift = Array.from({ length: 12 }, (_, k) => `<i style="left:${(k * 37) % 100}%;top:${(k * 53) % 90}%;animation-delay:${-k * 3.1}s;animation-duration:${22 + (k % 5) * 6}s;--s:${18 + (k % 4) * 14}px"></i>`).join('');
  return topbar() + `<main class="main cubehome" id="cubehome" style="--cc:${cat.color}">
      <div class="cb-drift" aria-hidden="true">${drift}</div>
      <div class="cb-stage">
        <button class="cb-hit" data-nav data-key="cb-cube" aria-label="${esc(cat.name)}"></button>
        <div class="cb-cube ${intro ? 'intro' : ''}" id="cbcube">
          ${slots}
          <span class="cb-face top" data-f="top" style="transform:${cubeFaceTransform('top')}" data-act="cubetilt" data-t="1">${cubeFaceHtml(cubeTop())}</span>
          <span class="cb-face bot" data-f="bot" style="transform:${cubeFaceTransform('bot')}" data-act="cubetilt" data-t="-1">${cubeFaceHtml(cubeBottom())}</span>
        </div>
      </div>
      <div class="cb-dots" id="cbdots">${dots}</div>
      <div class="cb-shelf ${c.zone === 'shelf' ? 'live' : ''}" id="cbshelf">${cubeShelfHtml(cat)}</div>
    </main>` + hints(c.zone === 'shelf' ? '↑ or B: back to the cube' : '← → roll · ↑ Recent · ↓ Favorites', c.zone === 'shelf' ? [['A', 'Open'], ['Y', 'Favorite'], ['X', 'Search'], ['☰', 'Settings']] : [['A', 'Choose'], ['X', 'Search'], ['☰', 'Settings']]);
}

// The cube's rotation: rolls are whole quarter turns that keep adding up (so
// the cube always turns the short way), tilts show the top or bottom face.
// The top and bottom faces counter-turn so their pictures stay upright.
// At rest the cube is turned a little so you see its top (Recent) and the next console on its right.
function cubeTransform() {
  return S.cube.tilt ? `rotateX(${-S.cube.tilt * 90}deg) rotateY(${-S.cube.rot * 90}deg)` : `rotateX(-14deg) rotateY(${-22 - S.cube.rot * 90}deg)`;
}
// Each face gets the whole cube's turn plus its own place, so the cube doesn't
// depend on the engine keeping 3D children together (some Linux setups can't).
const CUBE_FACE = { s0: 'rotateY(0deg) rotateX(0deg)', s1: 'rotateY(90deg) rotateX(0deg)', s2: 'rotateY(180deg) rotateX(0deg)', s3: 'rotateY(270deg) rotateX(0deg)', top: 'rotateY(0deg) rotateX(90deg)', bot: 'rotateY(0deg) rotateX(-90deg)' };
const cubeFaceTransform = (f) => `${cubeTransform()} ${CUBE_FACE[f]} translateZ(150px)`;
function cubePlaceFaces() {
  document.querySelectorAll('#cbcube .cb-face').forEach((el) => { el.style.transform = cubeFaceTransform(el.dataset.f); });
}

// Which faces can be seen from where the cube is now. Only those are drawn:
// some Linux setups can't tell which side of a 3D face points at you and would
// paint the back of the cube over the front.
function cubeVisible() {
  const f = mod(S.cube.rot, 4);
  if (S.cube.tilt > 0) return ['top'];
  if (S.cube.tilt < 0) return ['bot'];
  return [`s${f}`, `s${mod(f + 1, 4)}`, 'top'];
}
let cubeShowT = 0;
function cubeShow(before) {
  const cube = document.getElementById('cbcube');
  if (!cube) return;
  const now = cubeVisible(), during = new Set([...(before || []), ...now]);
  // While turning, faces on both sides of the turn stay; once it settles, only what's in view.
  const apply = (set) => cube.querySelectorAll('.cb-face').forEach((el) => { el.style.visibility = set.has(el.dataset.f) ? '' : 'hidden'; });
  apply(during);
  clearTimeout(cubeShowT);
  cubeShowT = setTimeout(() => apply(new Set(now)), before ? 560 : 0);
}

// Update the cube, dots and shelf in place so the turns animate.
function cubeSync(catChanged, before) {
  const { belt, cat } = cubeState();
  const cube = document.getElementById('cbcube'), shelf = document.getElementById('cbshelf'), home = document.getElementById('cubehome');
  if (!cube || !shelf) return render();
  cubePlaceFaces();
  cubeShow(before);
  home.style.setProperty('--cc', cat.color);
  const spin = mod(S.cube.rot, 4) * 90;
  cube.querySelectorAll('.cb-face.top .cb-in, .cb-face.bot .cb-in').forEach((el) => { el.style.transform = `rotate(${el.closest('.top') ? -spin : spin}deg)`; });
  // Repaint side faces whose console changed; they're out of view when that happens.
  for (let s = 0; s < 4; s++) {
    const f = cube.querySelector(`.cb-face.s${s}`), { c, k } = cubeSlotCat(belt, s);
    if (+f.dataset.i !== k) { f.dataset.i = k; f.innerHTML = cubeFaceHtml(c); }
  }
  document.querySelectorAll('#cbdots .cb-dot').forEach((d, k) => d.classList.toggle('on', k === S.cube.i && !S.cube.tilt));
  const hit = document.querySelector('.cb-hit'); if (hit) hit.setAttribute('aria-label', cat.name);
  if (catChanged) { shelf.innerHTML = cubeShelfHtml(cat); }
  cubeShelfPlace(cat);
  shelf.classList.toggle('live', S.cube.zone === 'shelf');
  if (cat.ti != null) S.sel = cat.ti;
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
  const before = cubeVisible();
  if (S.cube.tilt) { S.cube.tilt = 0; SFX.play('cubeL'); return cubeSync(true, before); } // level out first
  S.cube.rot += d; S.cube.i = mod(S.cube.i + d, belt.length);
  SFX.play(d > 0 ? 'cubeR' : 'cubeL');
  cubeSync(true, before);
}
function cubeTilt(t) {
  const want = Math.max(-1, Math.min(1, S.cube.tilt + t));
  if (want === S.cube.tilt || (want > 0 && !cubeTop()) || (want < 0 && !cubeBottom())) return false;
  const before = cubeVisible();
  S.cube.tilt = want;
  SFX.play('cubeR');
  cubeSync(true, before);
  return true;
}

// Controller / keyboard on the Cube home. Returns true when it handled the button.
function cubeKey(btn) {
  const cur = document.activeElement;
  const inCube = cur?.classList?.contains('cb-hit'), inShelf = cur?.classList?.contains('cb-game');
  if (!inCube && !inShelf) { // focus is up in the top bar
    if (btn === 'down') { S.cube.zone = 'cube'; const hit = document.querySelector('.cb-hit'); if (hit) { focusEl(hit, false, false); SFX.play('move'); return true; } }
    return false;
  }
  const { cat } = cubeState();
  if (S.cube.zone === 'cube') {
    if (btn === 'left' || btn === 'LB') return cubeRoll(-1), true;
    if (btn === 'right' || btn === 'RB') return cubeRoll(1), true;
    if (btn === 'up') return cubeTilt(1) || (S.cube.tilt > 0 ? false : (SFX.play('edge'), true)); // past the top face: up to the top bar
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
  return true;
}
function cubeMount() {
  const home = document.getElementById('cubehome');
  if (!home) return;
  const spin = mod(S.cube.rot, 4) * 90;
  home.querySelectorAll('.cb-face.top .cb-in, .cb-face.bot .cb-in').forEach((el) => { el.style.transform = `rotate(${el.closest('.top') ? -spin : spin}deg)`; });
  cubeShow();
  cubeShelfPlace(cubeState().cat);
  home.onwheel = (e) => {
    e.preventDefault();
    const now = performance.now(); if (now - (cubeMount.last || 0) < 120) return; cubeMount.last = now;
    const d = (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY) > 0 ? 1 : -1;
    if (e.target.closest('.cb-shelf')) { const { cat } = cubeState(); const n = waveItems(cat).length; S.cube.zone = 'shelf'; S.cube.at[cat.id] = Math.max(0, Math.min(n - 1, (S.cube.at[cat.id] || 0) + d)); SFX.play('move'); cubeShelfPlace(cat); }
    else cubeRoll(d);
  };
}
