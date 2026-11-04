// TV mode: Spine as the living-room console (Settings → Look → TV mode).
// More than a Home style: two sections, Play and Watch (LT/RT flips between them),
// big type for the couch, safe edges, one soft accent. Plan: claude/TV-MODE.md.
// Release 1 (0.28): Play = focused game's banner, a Consoles row (Recent first) and the
// console's games as a carousel. Watch = big tiles that open the existing Movies / TV /
// Streaming screens. Game pages are the normal ones for now.
// Loaded before app.js, so app.js helpers are only used inside functions.

const tvOn = () => settings().tvMode === true;
const TV_ACCENT = '#F0B45A'; // amber, used until the person picks an accent of their own
// Carousel steps (card width + gap) are read from the page, so tv.css alone decides sizes.
// Measured right after a render (tvMount), when no card is in the middle of growing or shrinking.
const TVSTEP = { game: 0, con: 0 };
const tvStepOf = (track, k) => { const c = track.children[k]; return c ? c.offsetWidth + (parseFloat(getComputedStyle(track).columnGap) || 0) : 0; };

// Text color that reads on the accent (dark on light accents, white on dark ones).
function tvInk(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex || '');
  if (!m) return '#16120a';
  const n = parseInt(m[1], 16), r = n >> 16, g = (n >> 8) & 255, b = n & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? '#16120a' : '#ffffff';
}

// Play's Consoles row: Recent first, then the Home tiles that hold games (Favorites,
// your lists, consoles), in the person's tile order. Movies/TV/Streaming live in Watch.
function tvCons() {
  const lp = S.boot.last_played || {};
  const notMedia = (x) => !isMovies(x.s);
  const out = [];
  const recent = Object.entries(lp).sort((a, b) => b[1] - a[1]).map(([id]) => findGame(id)).filter((x) => x && notMedia(x)).slice(0, 24);
  if (recent.length) out.push({ kind: 'recent', id: 'recent', name: 'Recently played', short: 'Recent', items: recent });
  tiles().forEach((t, ti) => {
    let items = null;
    if (t.kind === 'sys' && !isMovies(t.sys)) items = t.sys.games.map((g) => ({ s: t.sys, g }));
    else if (t.kind === 'fav') items = favItems().filter(notMedia);
    else if (t.kind === 'coll') items = collItems(t.coll).filter(notMedia);
    if (items && items.length) out.push({ ...t, ti, items });
  });
  return out;
}

function tvState() {
  S.tv = S.tv || { sec: 'play', con: null, at: {} };
  const cons = tvCons();
  let i = cons.findIndex((c) => c.id === S.tv.con);
  if (i < 0) i = 0;
  const con = cons[i] || null;
  if (con) {
    S.tv.con = con.id;
    S.tv.at[con.id] = Math.max(0, Math.min(S.tv.at[con.id] || 0, con.items.length - 1));
  }
  return { cons, i, con, at: con ? S.tv.at[con.id] : 0 };
}
const tvGameKey = (x) => `tvg-${x.g.id}`;

// Watch: what there is to watch (release 2 adds the two Continue rows).
function tvWatchTiles() {
  const T = tiles();
  const lib = [], addons = [];
  T.forEach((t, ti) => {
    if (t.kind === 'sys' && isMovies(t.sys)) lib.push({ ...t, ti });
    else if (t.kind === 'stream') addons.push({ ...t, ti });
  });
  return { lib, addons };
}

function tvFocusKey() {
  const st = tvState();
  if (S.tv.sec === 'watch') {
    const { lib, addons } = tvWatchTiles();
    const t = lib[0] || addons[0];
    return t ? `con-${t.id}` : 'tvs-watch';
  }
  if (!st.con) return 'tvs-play';
  return tvGameKey(st.con.items[st.at]);
}

// The big top part: the focused game's banner, its console, title, one line and Play / Details.
function tvHero(x) {
  if (!x) return '';
  const { s, g } = x, inf = g.info || {};
  const pt = (S.boot.playtime || {})[g.id];
  const meta = g.offline ? 'Its drive is not connected'
    : s.blocked ? s.blocked
      : [yearOf(g), inf.genre, pt ? `${fmtTime(pt)} played` : 'Not played yet'].filter(Boolean).join(' · ');
  const art = g.hero ? `<img class="tv-art" src="${mediaUrl(g.hero)}" alt="">`
    : g.cover ? `<img class="tv-art blur" src="${mediaUrl(g.cover)}" alt="">` : '';
  return `<div class="tv-bg" aria-hidden="true">${art}<div class="tv-scrim"></div></div>
    <div class="tv-info">
      <span class="tv-sys">${esc(s.name)}</span>
      ${g.logo ? `<img class="tv-logo" src="${mediaUrl(g.logo)}" alt="${esc(g.title)}">` : `<h1 class="tv-title">${esc(g.title)}</h1>`}
      <span class="tv-meta${s.blocked || g.offline ? ' warn' : ''}">${esc(meta)}</span>
      <div class="tv-acts">
        <button class="tv-btn primary" data-nav data-key="tvplay" data-act="play" data-id="${esc(g.id)}">${glyph('A')}Play</button>
        <button class="tv-btn" data-nav data-key="tvdetails" data-act="open" data-id="${esc(g.id)}" data-from="home">Details</button>
      </div>
    </div>`;
}

function tvCard(x, k, at) {
  const { s, g } = x;
  const pic = g.cover ? `<img data-src="${mediaUrl(g.cover)}" alt="" decoding="async">`
    : `<span class="ph" style="background:${PAL[hash(g.title) % PAL.length]}"><span>${esc(g.title)}</span></span>`;
  return `<button class="tv-game${k === at ? ' sel' : ''}${g.offline ? ' off' : ''}" data-nav data-key="${esc(tvGameKey(x))}" data-act="tvgame" data-k="${k}" data-id="${esc(g.id)}" aria-label="${esc(g.title)}, ${esc(s.short)}">
      ${pic}${isFav(g.id) ? `<span class="tv-fav">${ICON.heart}</span>` : ''}</button>`;
}

function tvTop(sec) {
  const time = new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const pill = (v, l) => `<button class="${sec === v ? 'on' : ''}" data-nav data-key="tvs-${v}" data-act="tvsec" data-s="${v}">${l}</button>`;
  return `<header class="tv-top">
      <div class="tv-left"><span class="tv-brand">${LOGO(30, 'var(--ta)')}Spine</span><div class="tv-switch" role="tablist">${pill('play', 'Play')}${pill('watch', 'Watch')}</div></div>
      <div class="tv-right"><span class="tv-clock" id="clock">${time}</span>
        <button class="tv-gear" data-nav data-key="gear" data-act="settings" aria-label="Settings">${ICON.gear}</button></div>
    </header>`;
}

function tvHints(list) {
  return `<footer class="tv-hints">${list.map(([k, l]) => `<span>${k.length === 1 ? `<b class="r">${k}</b>` : `<b>${k}</b>`}${esc(l)}</span>`).join('')}</footer>`;
}

function homeTv() {
  const { cons, i, con, at } = tvState();
  const accent = settings().accent || TV_ACCENT;
  const style = `--ta:${accent};--tink:${tvInk(accent)}`;
  if (S.tv.sec === 'watch') return tvWatch(style);
  if (!con) {
    return `<main class="tvm" id="tvm" style="${style}">${tvTop('play')}
      <div class="tv-empty"><h1>No games yet</h1><p>Add a ROM folder in Settings → Library.</p></div>
      ${tvHints([['LT RT', 'Play / Watch'], ['☰', 'Settings']])}</main>`;
  }
  const conHtml = cons.map((c, k) => `<button class="tv-con${k === i ? ' sel' : ''}" data-nav data-key="con-${esc(c.id)}" data-act="tvcon" data-id="${esc(c.id)}" aria-label="${esc(c.name)}, ${c.items.length}">
      ${sysIcon(c.kind === 'recent' ? 'recent' : tileIcon(c), 52)}<span>${esc(c.short)}</span></button>`).join('');
  const n = con.items.length;
  const label = con.kind === 'recent' ? 'Recently played' : `${con.name} · ${n.toLocaleString()} ${n === 1 ? 'game' : 'games'}`;
  return `<main class="tvm play" id="tvm" style="${style}">
      ${tvTop('play')}
      <section class="tv-hero" id="tvhero">${tvHero(con.items[at])}</section>
      <div class="tv-lower">
        <section class="tv-row"><h2>Consoles</h2><div class="tv-cons" id="tvcons"><div class="tv-track" id="tvcontrack">${conHtml}</div></div></section>
        <section class="tv-row"><h2 id="tvlabel">${esc(label)}</h2>
          <div class="tv-games" id="tvgames"><div class="tv-track" id="tvtrack">${con.items.map((x, k) => tvCard(x, k, at)).join('')}</div></div></section>
      </div>
      ${tvHints([['A', 'Play'], ['X', 'Details'], ['Y', 'Favorite'], ['LB RB', 'Console'], ['LT RT', 'Play / Watch']])}
    </main>`;
}

function tvWatch(style) {
  const { lib, addons } = tvWatchTiles();
  const tile = (t, sub) => `<button class="tv-wtile" style="--tc:${t.color}" data-nav data-key="con-${esc(t.id)}" data-act="tile" data-i="${t.ti}">
      <span class="ic">${sysIcon(tileIcon(t), 64)}</span><span class="t">${esc(t.name)}</span><span class="s">${esc(sub)}</span></button>`;
  const libHtml = lib.map((t) => tile(t, `${t.count.toLocaleString()} ${things(t.sys, t.count)}`)).join('');
  const addHtml = addons.map((t) => tile(t, `Catalogs and search · ${t.label}`)).join('');
  const body = lib.length || addons.length ? `<div class="tv-watch">
      ${lib.length ? `<section class="tv-row"><h2>Your library</h2><div class="tv-wrow">${libHtml}</div></section>` : ''}
      ${lib.length && addons.length ? '<hr class="tv-div">' : ''}
      ${addons.length ? `<section class="tv-row"><h2>From your addons</h2><div class="tv-wrow">${addHtml}</div></section>` : ''}
    </div>` : `<div class="tv-empty"><h1>Nothing to watch yet</h1><p>Add movie or TV folders in Settings → Library, or a streaming addon in Settings → Streaming.</p></div>`;
  return `<main class="tvm watch" id="tvm" style="${style}">${tvTop('watch')}${body}
    ${tvHints([['A', 'Open'], ['B', 'Play'], ['LT RT', 'Play / Watch'], ['☰', 'Settings']])}</main>`;
}

// Slide the carousels and refresh the banner without rebuilding the page.
function tvSync() {
  const { cons, i, con, at } = tvState();
  const track = document.getElementById('tvtrack');
  if (!con || !track) return;
  const step = TVSTEP.game;
  track.style.transform = `translateX(${-Math.max(0, at - 1) * step}px)`;
  const cards = track.children;
  for (let k = 0; k < cards.length; k++) {
    const el = cards[k];
    el.classList.toggle('sel', k === at);
    if (Math.abs(k - at) <= 14) { const img = el.querySelector('img[data-src]'); if (img) { img.src = img.dataset.src; img.removeAttribute('data-src'); } }
  }
  const hero = document.getElementById('tvhero');
  const x = con.items[at];
  if (hero && x && hero.dataset.id !== x.g.id) { hero.dataset.id = x.g.id; hero.innerHTML = tvHero(x); }
  // Consoles row: keep the selected one in view (offsetWidth is unzoomed, like the step).
  const ctrack = document.getElementById('tvcontrack'), cbox = document.getElementById('tvcons');
  if (ctrack && cbox) {
    const cstep = TVSTEP.con;
    const fit = cstep ? Math.max(1, Math.floor(cbox.offsetWidth / cstep)) : 1;
    const first = Math.max(0, Math.min(i - 2, cons.length - fit));
    ctrack.style.transform = `translateX(${-first * cstep}px)`;
  }
}

function tvStep(d) {
  const { con, at } = tvState();
  if (!con) return;
  const n = at + d;
  if (n < 0 || n >= con.items.length) return SFX.play('edge');
  S.tv.at[con.id] = n;
  tvSync();
  const el = document.querySelectorAll('#tvtrack .tv-game')[n];
  if (el) focusEl(el, false, false);
  SFX.play('move');
}

function tvConGo(d, zone) {
  const { cons, i } = tvState();
  const j = i + d;
  if (j < 0 || j >= cons.length) return SFX.play('edge');
  S.tv.con = cons[j].id;
  const st = tvState();
  S.focusKey = zone === 'cons' ? `con-${st.con.id}` : tvGameKey(st.con.items[st.at]);
  render();
  SFX.play('move');
}

function tvSec(sec) {
  tvState();
  if (S.tv.sec === sec) return;
  S.tv.sec = sec;
  S.focusKey = null;
  render();
}

// Controller / keyboard on the TV home. Returns true when it handled the press.
function tvKey(btn) {
  tvState();
  if (btn === 'LT' || btn === 'RT') { tvSec(S.tv.sec === 'play' ? 'watch' : 'play'); return true; }
  if (S.tv.sec === 'watch') {
    if (btn === 'B') { tvSec('play'); return true; }
    return btn === 'LB' || btn === 'RB';
  }
  const cur = document.activeElement;
  const zone = cur?.closest?.('#tvgames') ? 'games' : cur?.closest?.('#tvcons') ? 'cons' : 'other';
  if (btn === 'LB' || btn === 'RB') { tvConGo(btn === 'RB' ? 1 : -1, zone === 'cons' ? 'cons' : 'games'); return true; }
  const focusKey = (k) => { const el = navEls().find((e) => e.dataset.key === k); if (el) { focusEl(el, false, false); SFX.play('move'); } };
  const { con, at } = tvState();
  if (zone === 'games') {
    if (btn === 'left' || btn === 'right') { tvStep(btn === 'right' ? 1 : -1); return true; }
    if (btn === 'up') { focusKey(`con-${con.id}`); return true; }
    if (btn === 'down') { SFX.play('edge'); return true; }
    if (btn === 'X') { ACT.open({ id: con.items[at].g.id, from: 'home' }); return true; }
    return false;
  }
  if (zone === 'cons') {
    if (btn === 'left' || btn === 'right') { tvConGo(btn === 'right' ? 1 : -1, 'cons'); return true; }
    if (btn === 'down') { focusKey(tvGameKey(con.items[at])); return true; }
    if (btn === 'up') { focusKey('tvplay'); return true; }
    return false;
  }
  // Banner buttons: down goes to the games (the row you were on), not the nearest tile.
  if (btn === 'down' && cur?.closest?.('#tvhero')) { focusKey(`con-${con.id}`); return true; }
  return false;
}

// Mouse wheel over the games row moves through it; the cursor hides while a controller is used (tv.css).
function tvMount() {
  const box = document.getElementById('tvgames');
  if (!box) return;
  const track = document.getElementById('tvtrack'), ctrack = document.getElementById('tvcontrack');
  const at = [...track.children].findIndex((c) => c.classList.contains('sel'));
  TVSTEP.game = tvStepOf(track, at === 0 ? 1 : 0);
  TVSTEP.con = tvStepOf(ctrack, 0);
  tvSync();
  let acc = 0;
  box.onwheel = (e) => {
    e.preventDefault();
    acc += Math.abs(e.deltaY) > Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    if (Math.abs(acc) < 60) return;
    const d = acc > 0 ? 1 : -1; acc = 0;
    const { con, at } = tvState();
    const n = at + d;
    if (n < 0 || n >= con.items.length) return;
    S.tv.at[con.id] = n; tvSync();
  };
}

const TV_ACT = {
  tvsec(d) { tvSec(d.s); },
  tvmode(d) { S.tv = null; S.focusKey = `tvm-${d.v}`; setSetting('tvMode', d.v === 'on'); },
  // Click on a cover: the first click picks it, a click on the picked one plays it (A plays).
  tvgame(d) {
    const { con, at } = tvState();
    const k = +d.k;
    if (k !== at) { S.tv.at[con.id] = k; tvSync(); const el = document.querySelectorAll('#tvtrack .tv-game')[k]; if (el) focusEl(el, false, false); return; }
    ACT.play({ id: con.items[k].g.id });
  },
  // Console tile: picks that console; on the picked one, A opens its A–Z library (Recent: the games row).
  tvcon(d) {
    const { con } = tvState();
    if (d.id !== con.id) { S.tv.con = d.id; S.focusKey = `con-${d.id}`; return render(); }
    if (con.ti == null) { const el = navEls().find((e) => e.dataset.key === tvGameKey(con.items[S.tv.at[con.id]])); if (el) focusEl(el, false, false); return; }
    ACT.tile({ i: con.ti });
  },
};
