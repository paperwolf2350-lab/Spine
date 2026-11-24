// Spine — pop-up keyboard for search (controller-friendly) and title suggestions.
// Loaded before app.js; uses its globals (S, ACT, systems, esc, cover, glyph, focusEl, SFX...) at call time.
//
// The keyboard shows when search is opened with a controller (or with the Keyboard
// button). Keys move on a fixed grid (no spatial move()), so it behaves the same in
// every WebView. A types, B deletes (closes when the box is empty), Y space, START done.

// Where the last input came from: 'pad' | 'key' | 'mouse'. Set by app.js.
let INPUT_SRC = 'mouse';

// --- Matching ---------------------------------------------------------------
// Lowercase, no accents (Pokémon -> pokemon), punctuation as spaces.
function srchNorm(s) {
  return String(s || '').normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
}
// 0 = title starts with it, 1 = every typed word starts a word in the title,
// 2 = found inside the title, -1 = no match.
function srchScore(title, q) {
  const t = srchNorm(title), n = srchNorm(q);
  if (!n) return -1;
  if (t.startsWith(n)) return 0;
  const tw = t.split(' '), qw = n.split(' ');
  if (qw.every((w) => tw.some((x) => x.startsWith(w)))) return 1;
  if (t.includes(n)) return 2;
  // Squashed spaces ("mario64", "megaman") only for longer input: short bits match too much.
  if (n.length >= 4 && t.replace(/ /g, '').includes(n.replace(/ /g, ''))) return 2;
  return -1;
}
// Library hits for the results grid, best matches first.
function srchHits(q, max = 60) {
  const out = [];
  for (const s of systems()) for (const g of s.games) {
    const sc = srchScore(g.title, q);
    if (sc >= 0) out.push({ s, g, sc });
  }
  out.sort((a, b) => a.sc - b.sc || a.g.title.localeCompare(b.g.title));
  return out.slice(0, max);
}
// Title suggestions: one per title (the same game on two consoles shows once).
function srchSuggest(q, max = 4) {
  if (!srchNorm(q)) return [];
  const seen = new Map();
  for (const { s, g, sc } of srchHits(q, 400)) {
    if (sc > 1) continue; // suggestions finish words you're typing; loose matches stay in the results
    const k = srchNorm(g.title);
    const e = seen.get(k);
    if (e) { if (!e.sys.includes(s.short)) e.sys.push(s.short); continue; }
    seen.set(k, { title: g.title, sys: [s.short], sc });
  }
  // A suggestion that's exactly what's typed adds nothing.
  return [...seen.values()].filter((x) => srchNorm(x.title) !== srchNorm(q)).slice(0, max);
}
// Bold the part of each word that matches what's typed.
function srchMark(title, q) {
  const qw = srchNorm(q).split(' ').filter(Boolean);
  return String(title).split(/([\p{L}\p{N}]+)/u).map((part) => {
    const n = srchNorm(part);
    const w = n && qw.filter((x) => n.startsWith(x)).sort((a, b) => b.length - a.length)[0];
    if (!w) return esc(part);
    // Find how many original characters cover the matched letters (accents etc.).
    let i = 0, got = 0;
    while (i < part.length && got < w.length) { got += srchNorm(part[i]).length; i++; }
    return `<b>${esc(part.slice(0, i))}</b>${esc(part.slice(i))}`;
  }).join('');
}

// --- Keyboard layout ----------------------------------------------------------
const OSK_LAYOUTS = {
  abc: { cols: 6, rows: ['abcdef', 'ghijkl', 'mnopqr', 'stuvwx', 'yz1234', '567890'] },
  qwerty: { cols: 10, rows: ['1234567890', 'qwertyuiop', 'asdfghjkl', 'zxcvbnm'] },
};
const oskLayout = () => (settings().oskLayout === 'qwerty' ? 'qwerty' : 'abc');
const OSK_DEL = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 5H9l-6 7 6 7h12z"/><path d="M17 9l-6 6M11 9l6 6"/></svg>';
const OSK_SPACE = '<svg width="26" height="14" viewBox="0 0 26 14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M2 5v6h22V5"/></svg>';

// Rows of keys: {k: data-key, act, label, c: first column, w: width in columns, extra attrs}.
function oskRows() {
  const L = OSK_LAYOUTS[oskLayout()], C = L.cols;
  const rows = L.rows.map((r, ri) => {
    const off = Math.floor((C - r.length) / 2); // centre short QWERTY rows
    return [...r].map((ch, ci) => ({ k: `k-${ch}`, act: 'osktype', ch, label: ch, c: off + ci, w: 1 }));
  });
  // Bottom row: Space · Delete · ABC/QWERTY · Done (widths add up to the column count).
  const w = C === 6 ? [2, 1, 1, 2] : [4, 2, 2, 2];
  let c = 0;
  const bottom = [
    { k: 'k-space', act: 'oskspace', label: `${OSK_SPACE}<span>Space</span>`, aria: 'Space' },
    { k: 'k-del', act: 'oskdel', label: OSK_DEL, aria: 'Delete' },
    { k: 'k-layout', act: 'osklayout', label: oskLayout() === 'abc' ? 'QWE' : 'ABC', aria: oskLayout() === 'abc' ? 'Switch to QWERTY' : 'Switch to ABC order' },
    { k: 'k-done', act: 'oskdone', label: 'Done', aria: 'Done', cls: 'done' },
  ].map((b, i) => { const o = { ...b, c, w: w[i] }; c += w[i]; return o; });
  rows.push(bottom);
  return rows;
}

function oskHTML() {
  const rows = oskRows(), C = OSK_LAYOUTS[oskLayout()].cols;
  return `<div class="osk" style="--cols:${C}" role="group" aria-label="On-screen keyboard">${rows.map((row, r) => row.map((b) =>
    `<button class="key${b.cls ? ' ' + b.cls : ''}${b.ch ? '' : ' fn'}" data-nav data-key="${b.k}" data-act="${b.act}"${b.ch ? ` data-ch="${b.ch}"` : ''} data-r="${r}" data-c="${b.c}" data-w="${b.w}" style="grid-row:${r + 1};grid-column:${b.c + 1} / span ${b.w}"${b.aria ? ` aria-label="${esc(b.aria)}"` : ''}>${b.label}</button>`).join('')).join('')}</div>`;
}

// Suggestions: a list under the keyboard, or a chip row when the keyboard is hidden.
function oskSugHTML() {
  const list = srchSuggest(S.query);
  if (!list.length) return `<div class="osk-sug" aria-live="polite">${S.osk && !srchNorm(S.query) ? '<span class="osk-tip">Start typing — titles show up here.</span>' : ''}</div>`;
  return `<div class="osk-sug" aria-live="polite"><span class="sr">Suggestions</span>${list.map((x, i) =>
    `<button class="sug" data-nav data-key="sug-${i}" data-act="osksug" data-v="${esc(x.title)}">${ICON.search}<span class="st">${srchMark(x.title, S.query)}</span><span class="ss">${esc(x.sys.slice(0, 2).join(' · '))}${x.sys.length > 2 ? ' +' + (x.sys.length - 2) : ''}</span></button>`).join('')}</div>`;
}

function oskHints() {
  if (!S.osk) return '';
  const empty = !S.query;
  return `<footer class="hints osk-hints"><span class="note">${empty ? 'Type with the keyboard or your controller' : ''}</span>
    <span class="h">${glyph('A')}Type</span><span class="h">${glyph('B')}${empty ? 'Close' : 'Delete'}</span><span class="h">${glyph('Y')}Space</span><span class="h">${glyph('START')}Done</span></footer>`;
}

// --- Updating without a full render -------------------------------------------
// Swaps the results, suggestions and hints, keeps focus on the key you're on.
function oskRefresh() {
  const panel = document.querySelector('.search-panel');
  if (!panel) return render();
  const tmp = document.createElement('div');
  tmp.innerHTML = searchPanel();
  for (const sel of ['.res', '.osk-sug', '.osk-hints']) {
    const a = panel.querySelector(sel), b = tmp.querySelector(sel);
    if (a && b) a.replaceWith(b);
  }
  const q = document.getElementById('q');
  if (q && q !== document.activeElement) q.value = S.query;
  const el = S.focusKey && panel.querySelector(`[data-key="${CSS.escape(S.focusKey)}"]`);
  if (el) focusEl(el, false); else oskFocusKey();
}
function oskSet(q) { S.query = q; oskRefresh(); }

function oskFocusKey(key) {
  const panel = document.querySelector('.search-panel .osk');
  if (!panel) return false;
  const el = panel.querySelector(`[data-key="${CSS.escape(key || S.oskKey || 'k-a')}"]`) || panel.querySelector('.key');
  if (el) focusEl(el, false);
  return !!el;
}
function oskFirstResult() {
  return document.querySelector('.search-panel .res .game[data-nav]') || document.querySelector('.search-panel .res [data-nav]');
}

// Smaller keys when the window is short (1080p at 125% scaling and the like).
function oskFit() {
  const p = document.querySelector('.search-panel');
  if (p) p.classList.toggle('short', p.offsetHeight < 800);
}

// --- Controller / keyboard keys ---------------------------------------------------
// Returns true when it handled the press (press() in app.js asks first while search is open).
function oskPad(btn) {
  if (S.panel !== 'search' || !S.osk) return false;
  const cur = document.activeElement;
  const inPanel = cur && cur.closest && cur.closest('.search-panel');
  const zone = !inPanel ? 'none' : cur.closest('.osk') ? 'keys' : cur.closest('.osk-sug') ? 'sug' : cur.closest('.res') ? 'res' : 'top';
  if (zone === 'none' || zone === 'top') {
    if (['up', 'down', 'left', 'right'].includes(btn)) { oskFocusKey(); return true; }
    if (btn === 'B' && cur?.tagName !== 'INPUT') { SFX.play('back'); ACT.closepanel(); return true; }
    return false;
  }
  if (btn === 'START') { SFX.play('select'); if (zone === 'keys' || zone === 'sug') ACT.oskdone(); else oskFocusKey(); return true; }
  if (btn === 'B') {
    if (zone !== 'keys') { SFX.play('back'); oskFocusKey(); return true; }
    if (S.query) { SFX.play('tick'); ACT.oskdel(); return true; }
    SFX.play('back'); ACT.closepanel(); return true; // empty box: B closes, like everywhere else
  }
  if (btn === 'Y' && zone === 'keys') { SFX.play('tick'); ACT.oskspace(); return true; }
  if (btn === 'X' && zone === 'keys') { SFX.play('tick'); ACT.oskdel(); return true; }
  if (!['up', 'down', 'left', 'right'].includes(btn)) return false;

  if (zone === 'keys') return oskMoveKeys(cur, btn), true;
  if (zone === 'sug') {
    const all = [...document.querySelectorAll('.search-panel .osk-sug [data-nav]')];
    const i = all.indexOf(cur);
    if (btn === 'up') { if (i > 0) { focusEl(all[i - 1]); SFX.play('move'); } else { oskToRow(oskRows().length - 1); } return true; }
    if (btn === 'down') { if (i < all.length - 1) { focusEl(all[i + 1]); SFX.play('move'); } else SFX.play('edge'); return true; }
    if (btn === 'right') { const r = oskFirstResult(); if (r) { focusEl(r); SFX.play('move'); } else SFX.play('edge'); return true; }
    SFX.play('edge'); return true;
  }
  // Results: normal movement; left past the first column goes back to the keyboard.
  const before = document.activeElement;
  move(btn);
  if (btn === 'left' && document.activeElement === before) oskFocusKey();
  return true;
}

function oskKeysOf(r) {
  return [...document.querySelectorAll(`.search-panel .osk .key[data-r="${r}"]`)].sort((a, b) => a.dataset.c - b.dataset.c);
}
// Up/down keep the column you were in (remembered across short rows).
function oskToRow(r, col) {
  const row = oskKeysOf(r);
  if (!row.length) return;
  const c = col ?? (S.oskCol ?? 0);
  const el = row.find((k) => c >= +k.dataset.c && c < +k.dataset.c + +k.dataset.w)
    || row.reduce((best, k) => (Math.abs(+k.dataset.c - c) < Math.abs(+best.dataset.c - c) ? k : best), row[0]);
  focusEl(el, false); S.oskKey = el.dataset.key; SFX.play('move');
}
function oskMoveKeys(cur, dir) {
  const r = +cur.dataset.r, c = +cur.dataset.c, w = +cur.dataset.w, last = oskRows().length - 1;
  if (dir === 'left' || dir === 'right') {
    const row = oskKeysOf(r), i = row.indexOf(cur);
    const next = row[i + (dir === 'right' ? 1 : -1)];
    if (next) { focusEl(next, false); S.oskKey = next.dataset.key; S.oskCol = +next.dataset.c; SFX.play('move'); return; }
    if (dir === 'right') { const res = oskFirstResult(); if (res) { focusEl(res); SFX.play('move'); return; } }
    SFX.play('edge'); return;
  }
  // Keep the column we started from when passing over a wide key.
  if (S.oskCol == null || S.oskCol < c || S.oskCol >= c + w) S.oskCol = c;
  if (dir === 'up') { if (r > 0) oskToRow(r - 1); else SFX.play('edge'); return; }
  if (r < last) return oskToRow(r + 1);
  const sug = document.querySelector('.search-panel .osk-sug [data-nav]');
  if (sug) { focusEl(sug); SFX.play('move'); } else SFX.play('edge');
}

// Physical keyboard while a key has focus: typing still types.
// Returns true when it used the key (keydown in app.js asks first).
function oskKeydown(e) {
  if (S.panel !== 'search' || !S.osk) return false;
  const cur = document.activeElement;
  if (cur?.tagName === 'INPUT') return false; // the box itself works as usual
  if (e.key === 'Escape') { e.preventDefault(); ACT.closepanel(); return true; }
  if (!cur?.closest?.('.osk')) return false;
  if (e.ctrlKey || e.metaKey || e.altKey) return false;
  if (e.key === 'Enter') { e.preventDefault(); ACT.oskdone(); return true; }
  if (e.key === ' ') { e.preventDefault(); ACT.oskspace(); return true; }
  if (e.key.length === 1 && /[\p{L}\p{N}'&:\-.!?]/u.test(e.key)) { e.preventDefault(); oskSet(S.query + e.key); return true; }
  return false;
}

// --- Actions (merged into ACT by app.js) ------------------------------------------
const KEYS_ACT = {
  osktype(d) { S.oskKey = `k-${d.ch}`; S.focusKey = S.oskKey; oskSet(S.query + d.ch); },
  oskspace() { S.oskKey = S.focusKey = 'k-space'; if (S.query && !S.query.endsWith(' ')) oskSet(S.query + ' '); },
  oskdel() {
    if (document.activeElement?.closest?.('.osk')) S.oskKey = S.focusKey = document.activeElement.dataset.key;
    if (!S.query) return SFX.play('edge');
    oskSet(S.query.slice(0, -1));
  },
  async osklayout() {
    S.oskKey = S.focusKey = 'k-layout';
    await setSetting('oskLayout', oskLayout() === 'abc' ? 'qwerty' : 'abc');
  },
  oskdone() {
    const r = oskFirstResult();
    if (r) focusEl(r); else SFX.play('edge');
  },
  osksug(d) {
    S.focusKey = null;
    oskSet(d.v);
    const r = oskFirstResult();
    if (r) focusEl(r); else oskFocusKey();
  },
  osktoggle() {
    S.osk = !S.osk;
    S.focusKey = S.osk ? (S.oskKey || 'k-a') : 'q';
    render();
  },
};
