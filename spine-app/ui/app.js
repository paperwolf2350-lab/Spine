// Spine — interface. Plain JavaScript, no build step.
// Talks to the Rust side through Tauri commands; works with mouse, keyboard
// and any standard controller (Xbox / PlayStation / Switch Pro).
'use strict';

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------
const T = window.__TAURI__;
const api = T
  ? { call: (cmd, args) => T.core.invoke(cmd, args), on: (ev, cb) => T.event.listen(ev, (e) => cb(e.payload)) }
  : window.__SPINE_MOCK__; // only used when previewing the UI outside the app
const MEDIA = navigator.userAgent.includes('Windows') ? 'http://spine.localhost/' : 'spine://localhost/';
const OS_NAME = navigator.userAgent.includes('Windows') ? 'Windows' : navigator.userAgent.includes('Mac') ? 'macOS' : 'Linux';
// What opens a PC game: its store's launcher, or the system itself.
function opener(g) {
  if (g.format === 'steam') return 'Steam';
  if ((g.file || '').startsWith('Installed with Heroic')) return 'Heroic';
  if (g.format === 'epic') return 'Epic Games';
  return OS_NAME;
}
const mediaUrl = (rel) => {
  const [path, query] = rel.split('?');
  return (window.__SPINE_MEDIA__ || MEDIA) + path.split('/').map(encodeURIComponent).join('/') + (query ? '?' + query : '');
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
const S = {
  boot: null,          // everything from the backend
  screen: 'loading',   // loading | welcome | scanning | home | games | detail
  sel: 0,              // selected console on home
  con: 0,              // console shown on games screen
  game: null,          // game id on detail screen
  from: 'games',       // where detail returns to
  filter: 'all',
  panel: null,         // null | settings | search | scrapers | emulator
  emu: null,           // {id, current, options} for the emulator chooser
  accounts: null,      // scraper account status for Settings > Scrapers
  spun: null,          // game id picked by "Surprise me" (shows "Spin again")
  query: '',
  playing: null,       // {title, emulator, error}
  fetch: null,         // {stage, done, total}
  focusKey: null,
};
const settings = () => S.boot?.settings || {};
const videosOn = () => settings().videos !== false;
const storesOn = () => settings().storeGames !== false;
const exitComboOn = () => settings().exitCombo !== false;
const soundOn = () => settings().video_sound === true;
const systems = () => S.boot?.systems || [];
const isFav = (id) => (S.boot?.favorites || []).includes(id);

const PAL = ['#7A2E2E', '#2E4A7A', '#2E6B4F', '#6B3F7A', '#7A5A2E', '#2E6470', '#4A4A6B', '#7A2E5A', '#3F5A2E', '#8A3B1E'];
const hash = (s) => { let h = 0; for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0; return h; };
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmtTime = (secs) => {
  if (!secs) return 'Never played';
  const h = Math.floor(secs / 3600), m = Math.round((secs % 3600) / 60);
  return h ? `${h}h ${String(m).padStart(2, '0')}m` : `${Math.max(m, 1)}m`;
};
const ago = (t) => {
  if (!t) return '';
  const d = (Date.now() / 1000 - t) / 86400;
  return d < 1 ? 'today' : d < 2 ? 'yesterday' : d < 7 ? `${Math.floor(d)} days ago` : new Date(t * 1000).toLocaleDateString();
};

const FAV_COLOR = '#B23A6E';
const CAT_COLOR = '#2F6F6A';
const RANDOM_COLOR = '#3E4A5C';
// Home carousel: Favorites (when there are any), every console, then Surprise me.
function tiles() {
  const t = [];
  const favs = favItems();
  if (favs.length) t.push({ kind: 'fav', id: 'favorites', name: 'Favorites', short: 'Favorites', color: FAV_COLOR, count: favs.length });
  for (const name of Object.keys(S.boot?.collections || {})) {
    const items = collItems(name);
    if (items.length) t.push({ kind: 'coll', id: 'coll:' + name, coll: name, name, short: name, color: PAL[hash(name) % PAL.length], count: items.length });
  }
  systems().forEach((s, i) => t.push({ kind: 'sys', id: s.id, name: s.name, short: s.short, color: s.color, count: s.games.length, sys: s, i }));
  t.push({ kind: 'random', id: 'random', name: 'Surprise me', short: 'Surprise me', color: RANDOM_COLOR, count: null });
  return t;
}
// Tiles you can open as a game list (everything except Surprise me).
const browsable = () => tiles().filter((t) => t.kind !== 'random');
function favItems() {
  const out = [];
  for (const s of systems()) for (const g of s.games) if (isFav(g.id)) out.push({ s, g });
  return out.sort((a, b) => sortTitle(a.g.title).localeCompare(sortTitle(b.g.title)));
}
// One of the user's lists, in the order games were added.
function collItems(name) {
  const ids = (S.boot?.collections || {})[name] || [];
  return ids.map((id) => findGame(id)).filter(Boolean);
}
const tileIcon = (t) => (t.kind === 'fav' ? 'favorites' : t.kind === 'random' ? 'random' : t.kind === 'cat' ? 'category' : t.kind === 'coll' ? 'collection' : t.id);
const sortTitle = (t) => t.replace(/^the /i, '').toLowerCase();
// What the games screen is showing.
function curList() {
  if (S.cat) {
    const label = S.cat.type === 'series' ? `${S.cat.value} series` : S.cat.type === 'dev' ? `By ${S.cat.value}` : S.cat.value;
    return { tile: { kind: 'cat', id: 'category', name: label, short: label, color: CAT_COLOR }, items: catItems(S.cat), emulator: null, blocked: null };
  }
  const b = browsable();
  const t = b[Math.min(S.con, b.length - 1)];
  if (!t) return null;
  if (t.kind === 'fav') return { tile: t, items: favItems(), emulator: null, blocked: null };
  if (t.kind === 'coll') return { tile: t, items: collItems(t.coll), emulator: null, blocked: null };
  return { tile: t, items: t.sys.games.map((g) => ({ s: t.sys, g })), emulator: t.sys.emulator, blocked: t.sys.blocked };
}

// ---- Categories: genres + community tags, plus series and developer.
// Compare names loosely: no accents, no leading "The", no punctuation ("Pokémon" = "Pokemon").
const catKey = (c) => String(c).normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/^the\s+/, '').replace(/['\u2019-]/g, '').replace(/[^a-z0-9]+/g, ' ').trim();
// A category also covers its more specific tags: "Roguelike" includes "Action Roguelike"
// and "Roguelike Deckbuilder"; short broad words ("Action", "RPG") match exactly.
const catMatches = (tagKey, k) => tagKey === k || (k.length >= 8 && (` ${tagKey} `).includes(` ${k} `));
function catsOf(g) {
  const out = [], seen = new Set();
  const add = (c) => { const k = catKey(c); if (c && !seen.has(k)) { seen.add(k); out.push(c.trim()); } };
  (g.meta?.genres || []).forEach(add);
  (g.info?.genre || '').split(/[,/]/).forEach((x) => x.trim() && add(x.trim()));
  (g.meta?.tags || []).forEach(add);
  return out;
}
// Series: libretro's franchise, else the name before ":" (minus a trailing
// number), else a known series name the title starts with
// ("The Legend of Zelda Breath of the Wild" -> The Legend of Zelda).
function seriesHead(g) {
  const t = g.title;
  if (!/:| - /.test(t)) return null;
  const head = t.split(/:| - /)[0].replace(/\s+(\d+|II|III|IV|V|VI|VII|VIII|IX|X|HD|DX|3D|Advance|64|DS)$/i, '').trim();
  return head.split(' ').length >= 2 || head.length >= 6 ? head : null;
}
let knownSeriesFor = null, knownSeries = [];
function seriesList() {
  if (knownSeriesFor === S.boot) return knownSeries;
  const m = new Map();
  for (const s of systems()) for (const g of s.games) {
    const n = g.meta?.franchise || seriesHead(g);
    if (n && catKey(n).split(' ').length >= 2) m.set(catKey(n), n);
  }
  knownSeries = [...m.entries()].sort((a, b) => b[0].length - a[0].length);
  knownSeriesFor = S.boot;
  return knownSeries;
}
function seriesOf(g) {
  if (g.meta?.franchise) return g.meta.franchise;
  const h = seriesHead(g);
  if (h) return h;
  const k = catKey(g.title);
  const hit = seriesList().find(([key]) => k.startsWith(key + ' '));
  return hit ? hit[1] : null;
}
const devOf = (g) => g.info?.developer || g.meta?.developer || null;
// Movies use "movie / watched" words where games say "game / played".
const isMovies = (s) => s && (s.id === 'movies' || s.id === 'tv'); // anything watched rather than played
const isTv = (s) => s && s.id === 'tv';
const things = (s, n) => (isTv(s) ? (n === 1 ? 'show' : 'shows') : isMovies(s) ? (n === 1 ? 'movie' : 'movies') : (n === 1 ? 'game' : 'games'));
// "S1 · E3" for seasons, "E3" in an arc, nothing for specials.
// A show's tabs in viewing order: seasons, then arcs (by the season they were
// matched to, else A–Z), then movies & specials.
const isArc = (g, group) => (g.episodes || []).some((e) => e.group === group && e.season == null && e.number != null);
function showGroups(g) {
  const eps = g.episodes || [];
  const all = [...new Set(eps.map((e) => e.group))];
  const rank = (gr) => {
    if (gr === 'Movies & specials') return [3, 0, 0, ''];
    const e = eps.find((x) => x.group === gr);
    if (e.season != null) return [0, e.season, 0, ''];
    const m = g.arcs?.[gr];
    return m ? [0, m.season, m.start, ''] : [1, 0, 0, gr.toLowerCase()];
  };
  return all.sort((a, b) => { const x = rank(a), y = rank(b); for (let i = 0; i < 4; i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1; return 0; });
}
// "Season 2 · Entertainment District Arc (episodes 8–18)"
function arcLabel(g, m, count) {
  const t = (g.tmdb_seasons || []).find((x) => x.number === m.season);
  const name = t?.name && !/^season \d+$/i.test(t.name) ? ` · ${t.name}` : '';
  const part = t && count < t.episodes ? ` (episodes ${m.start}–${m.start + count - 1})` : '';
  return `Season ${m.season}${name}${part}`;
}
const epCode = (e) => (e.number == null ? '' : e.season != null ? `S${e.season} · E${e.number}` : `E${e.number}`);
// Where you left off: movies by id, episodes by "<show id>|<file name>".
const progKey = (g, i) => (i == null ? g.id : `${g.id}|${g.episodes[i].file}`);
const progOf = (g, i) => (S.boot.progress || {})[progKey(g, i)] || null;
const finished = (p) => !!p.len && (p.pos * 10 >= p.len * 9 || p.pos + 60 >= p.len);
const canResume = (p) => !!p && p.pos >= 30 && !finished(p);
const clock = (t) => { const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), x = t % 60; return `${h ? `${h}:${String(m).padStart(2, '0')}` : m}:${String(x).padStart(2, '0')}`; };
// A show's main button: back into the episode you stopped in, else the next
// unwatched one after the last you watched, else the first.
function showNext(g) {
  const eps = g.episodes || [];
  const order = showGroups(g).flatMap((gr) => eps.map((e, i) => ({ e, i })).filter((x) => x.e.group === gr).map((x) => x.i));
  let last = -1, lastAt = 0;
  order.forEach((i, k) => { const p = progOf(g, i); if (p && p.at > lastAt) { lastAt = p.at; last = k; } });
  if (last < 0) return { i: order[0], kind: 'play' };
  const p = progOf(g, order[last]);
  if (canResume(p)) return { i: order[last], kind: 'resume', pos: p.pos };
  if (!p.watched) return { i: order[last], kind: 'play' };
  const after = order.slice(last + 1).concat(order.slice(0, last)).find((i) => !progOf(g, i)?.watched);
  return after != null ? { i: after, kind: 'next' } : { i: order[0], kind: 'play' };
}
const fmtRuntime = (m) => (m >= 60 ? `${Math.floor(m / 60)} h ${m % 60 ? (m % 60) + ' min' : ''}`.trim() : `${m} min`);
const yearOf = (g) => g.info?.year || g.meta?.year || null;
function allItems() { return systems().flatMap((s) => s.games.map((g) => ({ s, g }))); }
function catItems(cat) {
  const k = catKey(cat.value);
  const items = allItems().filter(({ g }) =>
    cat.type === 'series' ? catKey(seriesOf(g) || '') === k
    : cat.type === 'dev' ? catKey(devOf(g) || '') === k
    : catsOf(g).some((c) => catMatches(catKey(c), k)));
  return items.sort((a, b) => sortTitle(a.g.title).localeCompare(sortTitle(b.g.title)));
}
// Most common categories in the library (for the home row and search).
function topCats(min = 3) {
  const n = new Map();
  for (const { g } of allItems()) for (const c of catsOf(g)) { const k = catKey(c); const e = n.get(k) || { name: c, count: 0 }; e.count++; n.set(k, e); }
  return [...n.values()].filter((e) => e.count >= min).sort((a, b) => b.count - a.count);
}
// Stable shuffle so rows don't jump around between visits.
const seeded = (arr, seed) => arr.map((x) => [hash(seed + x.g.id), x]).sort((a, b) => a[0] - b[0]).map((x) => x[1]);

function findGame(id) {
  for (const s of systems()) { const g = s.games.find((x) => x.id === id); if (g) return { s, g }; }
  return null;
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------
const ICON = {
  download: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v11M7 10l5 5 5-5M5 20h14"/></svg>',
  sort: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 4v16M3 16l4 4 4-4M14 6h7M14 12h5M14 18h3"/></svg>',
  list: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4" y="7" width="13" height="13" rx="2"/><path d="M8 3h10a2 2 0 0 1 2 2v10"/></svg>',
  pencil: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/></svg>',
  plus: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>',
  search: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>',
  gear: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  back: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 18l-6-6 6-6"/></svg>',
  heart: '<svg width="14" height="14" viewBox="0 0 24 24" fill="#fff" aria-hidden="true"><path d="M12 21s-7-4.5-9.5-9A5.5 5.5 0 0 1 12 6a5.5 5.5 0 0 1 9.5 6c-2.5 4.5-9.5 9-9.5 9z"/></svg>',
  heartA: '<svg width="16" height="16" viewBox="0 0 24 24" fill="var(--accent)" role="img" aria-label="Favorite"><path d="M12 21s-7-4.5-9.5-9A5.5 5.5 0 0 1 12 6a5.5 5.5 0 0 1 9.5 6c-2.5 4.5-9.5 9-9.5 9z"/></svg>',
  dice: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true" style="vertical-align:-3px"><rect x="3" y="3" width="18" height="18" rx="4"/><circle cx="8" cy="8" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="16" cy="16" r="1.3" fill="currentColor"/></svg>',
  image: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="M21 15l-5-5L5 21"/></svg>',
  film: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M10 9l5 3-5 3z"/></svg>',
  soundOn: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M11 5L6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13"/></svg>',
  soundOff: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M11 5L6 9H3v6h3l5 4z"/><path d="M16 9l5 6M21 9l-5 6"/></svg>',
  check: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12l5 5L20 7"/></svg>',
  tiles: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>',
  list: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M9 6h12M9 12h12M9 18h12"/><circle cx="4" cy="6" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="18" r="1"/></svg>',
  cards: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="13" width="18" height="7" rx="1.5"/></svg>',
  shelf: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" aria-hidden="true"><rect x="4" y="5" width="4" height="15" rx="1"/><rect x="10" y="3" width="4" height="17" rx="1"/><path d="M16.5 7.5l3.5-1 3 13-3.5 1z"/><path d="M2 21h20"/></svg>',
};
const LOGO = (size, accent) => `<svg width="${size}" height="${size}" viewBox="0 0 84 84" aria-hidden="true"><rect x="8" y="14" width="18" height="62" rx="5" fill="${accent}"/><rect x="33" y="6" width="18" height="70" rx="5" fill="var(--text)"/><rect x="58" y="22" width="18" height="54" rx="5" fill="var(--muted)"/></svg>`;
// Console icons: simple original line drawings of each machine's shape (no logos).
const SYS_ICON = {
  // Cartridge consoles
  nes: '<rect x="8" y="16" width="48" height="22" rx="3"/><rect x="18" y="16" width="28" height="8" rx="1"/><path d="M14 32h6M44 32h6"/>',
  snes: '<path d="M10 22c0-4 3-6 7-6h30c4 0 7 2 7 6v12c0 3-2 5-5 5H15c-3 0-5-2-5-5z"/><rect x="20" y="12" width="24" height="7" rx="1.5"/><circle cx="20" cy="31" r="2"/><circle cx="44" cy="31" r="2"/>',
  n64: '<path d="M5 30l7-11c1-1 2-2 4-2h32c2 0 3 1 4 2l7 11v7c0 2-1 3-3 3H8c-2 0-3-1-3-3z"/><path d="M5 30h54"/><rect x="22" y="9" width="20" height="13" rx="1.5"/><path d="M25 13h14"/><path d="M15 23h5M44 23h5"/><rect x="14" y="33" width="5" height="4" rx="1"/><rect x="23" y="33" width="5" height="4" rx="1"/><rect x="36" y="33" width="5" height="4" rx="1"/><rect x="45" y="33" width="5" height="4" rx="1"/>',
  genesis: '<path d="M4 25c0-3 2-6 6-6h44c4 0 6 3 6 6v8c0 3-2 5-5 5H9c-3 0-5-2-5-5z"/><ellipse cx="27" cy="23" rx="15" ry="7"/><rect x="18" y="20.5" width="18" height="4" rx="1"/><path d="M47 24h8"/><path d="M45 31h11M45 34h11"/><path d="M9 33h8"/>',
  mastersystem: '<path d="M8 20l6-5h36l6 5v14c0 2-2 4-4 4H12c-2 0-4-2-4-4z"/><path d="M22 15v6h20v-6"/><path d="M14 32h10"/>',
  // Handhelds
  gb: '<rect x="18" y="4" width="28" height="40" rx="3"/><rect x="23" y="9" width="18" height="14" rx="1"/><path d="M26 31v6M23 34h6"/><circle cx="37" cy="35" r="1.8"/><circle cx="41" cy="31" r="1.8"/>',
  gbc: '<path d="M19 4h26c2 0 3 1 3 3v31c0 3-3 6-6 6H22c-3 0-6-3-6-6V7c0-2 1-3 3-3z"/><rect x="22" y="9" width="20" height="15" rx="1"/><path d="M25 32v6M22 35h6"/><circle cx="37" cy="36" r="1.8"/><circle cx="41" cy="32" r="1.8"/>',
  gba: '<path d="M8 16c0-4 4-6 8-6h32c4 0 8 2 8 6v14c0 5-5 8-10 8H18c-5 0-10-3-10-8z"/><rect x="21" y="14" width="22" height="16" rx="1"/><path d="M14 20v6M11 23h6"/><circle cx="49" cy="25" r="1.8"/><circle cx="53" cy="21" r="1.8"/>',
  nds: '<rect x="14" y="4" width="36" height="18" rx="2"/><rect x="20" y="7" width="24" height="12" rx="1"/><rect x="14" y="25" width="36" height="19" rx="2"/><rect x="22" y="28" width="20" height="13" rx="1"/><path d="M17 33h3M45 33h2"/>',
  n3ds: '<rect x="13" y="4" width="38" height="19" rx="3"/><rect x="18" y="7" width="28" height="13" rx="1"/><rect x="13" y="25" width="38" height="19" rx="3"/><rect x="23" y="28" width="18" height="13" rx="1"/><circle cx="18" cy="30" r="2.2"/><path d="M18 36v4M16 38h4"/><circle cx="47" cy="34" r="1.6"/>',
  psp: '<path d="M10 14h44c5 0 8 4 8 10s-3 10-8 10H10c-5 0-8-4-8-10s3-10 8-10z"/><rect x="18" y="17" width="28" height="14" rx="1"/><path d="M9 22v5M6.5 24.5h5"/><circle cx="55" cy="24.5" r="2"/>',
  psvita: '<path d="M10 13h44c5 0 8 5 8 11s-3 11-8 11H10c-5 0-8-5-8-11s3-11 8-11z"/><rect x="17" y="16" width="30" height="16" rx="2"/><path d="M8 20v5M5.5 22.5h5"/><circle cx="56" cy="22" r="1.8"/><circle cx="10" cy="30" r="2"/><circle cx="54" cy="30" r="2"/>',
  // Disc consoles
  gc: '<path d="M12 18l20-9 20 9-20 9z"/><path d="M12 18v16l20 9V27"/><path d="M52 18v16l-20 9"/><path d="M19 13c0-7 26-7 26 0"/><ellipse cx="32" cy="18" rx="9" ry="4"/><path d="M16 29v4M21 31v4M26 33v4"/>',
  wii: '<rect x="24" y="4" width="16" height="40" rx="3"/><path d="M27 10h10"/><circle cx="32" cy="38" r="1.5"/>',
  wiiu: '<path d="M8 14c0-3 2-5 5-5h38c3 0 5 2 5 5v20c0 3-2 5-5 5H13c-3 0-5-2-5-5z"/><rect x="18" y="13" width="28" height="22" rx="1"/><circle cx="13" cy="18" r="2"/><circle cx="51" cy="18" r="2"/><path d="M13 27v4M11 29h4"/><circle cx="51" cy="29" r="1.6"/>',
  switch: '<rect x="16" y="11" width="32" height="26" rx="2"/><rect x="19" y="14" width="26" height="20" rx="1"/><path d="M16 11h-4c-4 0-6 3-6 6v14c0 3 2 6 6 6h4z"/><path d="M48 11h4c4 0 6 3 6 6v14c0 3-2 6-6 6h-4z"/><circle cx="11" cy="18" r="2"/><circle cx="53" cy="28" r="2"/>',
  segacd: '<rect x="8" y="14" width="48" height="24" rx="3"/><circle cx="32" cy="26" r="8"/><circle cx="32" cy="26" r="2"/>',
  saturn: '<path d="M8 20c0-4 3-6 7-6h34c4 0 7 2 7 6v12c0 4-3 6-7 6H15c-4 0-7-2-7-6z"/><circle cx="32" cy="24" r="7"/><path d="M14 33h8M42 33h8"/>',
  dreamcast: '<path d="M10 16h44l2 6v12c0 2-2 4-4 4H12c-2 0-4-2-4-4V22z"/><circle cx="32" cy="27" r="7"/><path d="M32 22a5 5 0 1 0 4 2"/>',
  psx: '<rect x="8" y="12" width="48" height="26" rx="3"/><circle cx="24" cy="25" r="9"/><circle cx="24" cy="25" r="2"/><rect x="40" y="18" width="10" height="3" rx="1.5"/><circle cx="45" cy="31" r="2"/>',
  ps2: '<path d="M4 15h56v9H4z"/><path d="M8 24h52v11H8z"/><path d="M4 15l4 9v11"/><path d="M15 19.5h34"/><circle cx="54" cy="19.5" r="1.4"/><path d="M12 27v6M15 27v6M18 27v6"/><rect x="24" y="28" width="6" height="3" rx="1"/><rect x="33" y="28" width="6" height="3" rx="1"/><rect x="48" y="28" width="7" height="3" rx="1"/>',
  ps3: '<path d="M10 16c0-3 3-6 6-6h32c3 0 6 3 6 6v20c0 1-1 2-2 2H12c-1 0-2-1-2-2z"/><path d="M16 26h32"/>',
  xbox: '<rect x="8" y="12" width="48" height="26" rx="4"/><circle cx="32" cy="25" r="6"/><path d="M14 33h8"/>',
  pcengine: '<rect x="14" y="12" width="36" height="26" rx="2"/><rect x="20" y="8" width="24" height="8" rx="1"/><path d="M20 30h10"/>',
  neogeo: '<rect x="8" y="14" width="48" height="24" rx="2"/><rect x="18" y="10" width="28" height="8" rx="1"/><circle cx="16" cy="30" r="2"/>',
  arcade: '<path d="M18 4h28v40H18z"/><rect x="22" y="9" width="20" height="14" rx="1"/><path d="M18 28h28"/><path d="M27 28v-4"/><circle cx="27" cy="23" r="2"/><circle cx="37" cy="27" r="1.6"/>',
  atari2600: '<path d="M8 22l6-8h36l6 8v12c0 2-2 4-4 4H12c-2 0-4-2-4-4z"/><path d="M8 28h48M14 24h6M44 24h6"/>',
  tv: '<rect x="6" y="12" width="52" height="32" rx="3"/><path d="M24 4l8 8 8-8"/><path d="M14 20h28v16H14z"/><circle cx="50" cy="22" r="2"/><circle cx="50" cy="30" r="2"/>',
  movies: '<rect x="8" y="18" width="48" height="26" rx="2"/><path d="M8 18l4-10 44 0-4 10"/><path d="M20 8l-4 10M32 8l-4 10M44 8l-4 10"/><path d="M28 25l10 6-10 6z"/>',
  pc: '<rect x="8" y="6" width="48" height="30" rx="2"/><path d="M26 36l-2 6h16l-2-6M20 42h24"/>',
  dos: '<rect x="8" y="6" width="48" height="30" rx="2"/><path d="M16 16l5 4-5 4M24 26h8"/><path d="M26 36l-2 6h16l-2-6"/>',
  category: '<path d="M8 10h22l26 26-16 16L14 26z"/><circle cx="18" cy="18" r="3"/>',
  collection: '<rect x="12" y="20" width="34" height="34" rx="5"/><path d="M20 12h29a3 3 0 0 1 3 3v29"/><path d="M22 34h14M22 42h10"/>',
  favorites: '<path d="M32 42s-17-10-22-20a10 10 0 0 1 22-9 10 10 0 0 1 22 9c-5 10-22 20-22 20z"/>',
  random: '<rect x="8" y="12" width="24" height="24" rx="4"/><circle cx="14" cy="18" r="1.6" fill="currentColor"/><circle cx="20" cy="24" r="1.6" fill="currentColor"/><circle cx="26" cy="30" r="1.6" fill="currentColor"/><path d="M36 20l12-6 8 18-12 6z"/><circle cx="45" cy="22" r="1.5" fill="currentColor"/><circle cx="48" cy="29" r="1.5" fill="currentColor"/>',
  _: '<path d="M14 18h36c5 0 8 4 8 9l-1 7c-1 4-6 5-9 2l-5-5H21l-5 5c-3 3-8 2-9-2l-1-7c0-5 3-9 8-9z"/><path d="M20 23v6M17 26h6"/><circle cx="43" cy="24" r="1.6"/><circle cx="47" cy="28" r="1.6"/>',
};
const sysIcon = (id, size = 56) => `<svg class="sysicon" width="${size}" height="${Math.round(size * 0.75)}" viewBox="0 0 64 48" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${SYS_ICON[id] || SYS_ICON._}</svg>`;
const glyph = (k) => ('ABXY'.includes(k) && k.length === 1 ? `<span class="glyph ${k.toLowerCase()}">${k}</span>` : `<span class="glyph wide">${k}</span>`);

function cover(g, s, extra = '') {
  const fav = isFav(g.id) && !extra.includes('nofav') ? `<span class="favdot">${ICON.heart}</span>` : '';
  const off = g.offline ? ' off' : '';
  if (g.cover) {
    return `<div class="cover${off}"><img src="${mediaUrl(g.cover)}" alt="" loading="lazy" decoding="async">${fav}</div>`;
  }
  return `<div class="cover ph${off}" style="background:${PAL[hash(g.title) % PAL.length]}"><span class="pt">${esc(g.title)}</span><span class="ps">${esc(s ? s.short : '')}</span>${fav}</div>`;
}

function topbar() {
  const total = systems().reduce((n, s) => n + s.games.length, 0);
  const media = systems().some((s) => isMovies(s) && s.games.length);
  const time = new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  return `<header class="topbar">
    <button class="brand" data-nav data-key="brand" data-act="home" aria-label="Spine home">${LOGO(30, 'var(--accent)')}<span>Spine</span></button>
    <div class="spacer"></div>
    <button class="searchbox" data-nav data-key="searchbtn" data-act="search">${ICON.search}<span>Search ${total.toLocaleString()} ${media ? 'titles' : 'games'}</span><span style="margin-left:auto">${glyph('X')}</span></button>
    <span class="clock" id="clock">${time}</span>
    <button class="iconbtn" data-nav data-key="gear" data-act="settings" aria-label="Settings">${ICON.gear}</button>
  </header>`;
}

function hints(note, list) {
  const items = list.map(([k, label]) => `<span class="h">${glyph(k)}${label}</span>`).join('');
  return `<footer class="hints"><span class="note">${esc(note)}</span>${items}</footer>`;
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------
function welcome() {
  return `<div class="welcome">
    ${LOGO(84, 'var(--accent)')}
    <h1>Spine</h1>
    <p>Point it at your games. That's it.</p>
    <button class="btn primary" style="margin-top:14px;font-size:20px;padding:18px 30px" data-nav data-key="pick" data-act="pick">${glyph('A')}Choose your ROM folder</button>
    <p class="small">Works with RetroArch, PCSX2, Dolphin, Cemu and more · No config files, ever</p>
  </div>`;
}

function scanning() {
  return `<div class="welcome"><div class="spinner"></div><h2 style="font-size:40px">Reading your library…</h2><p>${esc(S.boot?.rom_root || '')}</p></div>`;
}

function home() {
  const sys = systems();
  if (!sys.length) {
    return topbar() + `<main class="main"><div class="welcome"><h2 style="font-size:36px">No games found here</h2><p>${esc(S.boot.rom_root)}</p>
      <button class="btn primary" data-nav data-key="pick" data-act="pick">${glyph('A')}Choose a different folder</button></div></main>` + hints('', [['B', 'Back']]);
  }
  const T = tiles();
  const sel = Math.min(S.sel, T.length - 1);
  const t = T[sel];
  // Hero: a fan of covers from the selected tile.
  const pool = t.kind === 'sys' ? t.sys.games.map((g) => ({ s: t.sys, g })) : t.kind === 'fav' ? favItems() : t.kind === 'coll' ? collItems(t.coll) : sys.flatMap((x) => x.games.map((g) => ({ s: x, g })));
  const withCovers = pool.filter((x) => x.g.cover);
  const src = withCovers.length >= 5 ? withCovers : pool;
  const step = t.kind === 'random' ? 37 : Math.max(1, Math.floor(src.length / 5));
  const fan = src.filter((_, i) => i % step === 0).slice(0, 5);
  let status, cta;
  if (t.kind === 'sys') {
    status = `${t.count.toLocaleString()} ${things(t.sys, t.count)} · ` + (t.sys.blocked ? `<span class="warn">${esc(t.sys.blocked)}</span>` : (t.sys.emulator === 'system' ? 'opens with their store or your PC' : `${isMovies(t.sys) ? 'plays in' : 'launches with'} ${esc(t.sys.emulator)}`))
      + (t.sys.bios && !t.sys.blocked ? `<br><span class="warn">${esc(t.sys.bios.message)}</span>` : '');
    cta = 'Open library';
  } else if (t.kind === 'coll') {
    status = `Your list · ${t.count} game${t.count === 1 ? '' : 's'} from ${new Set(collItems(t.coll).map((x) => x.s.id)).size} console${new Set(collItems(t.coll).map((x) => x.s.id)).size === 1 ? '' : 's'}`;
    cta = 'Open list';
  } else if (t.kind === 'fav') {
    status = `${t.count} favorite${t.count === 1 ? '' : 's'} from ${new Set(favItems().map((x) => x.s.id)).size} consoles`;
    cta = 'Open favorites';
  } else {
    status = `Can't decide? Spine picks something from all ${sys.reduce((n, x) => n + x.games.length, 0).toLocaleString()} games.`;
    cta = 'Surprise me';
  }
  const cards = T.map((c, i) => `<button class="console ${i === sel ? 'sel' : ''} ${c.kind}" style="background:${c.color};--chip:${c.color}" data-nav data-key="con-${c.id}" data-act="tile" data-i="${i}" data-focus="selcon" aria-label="${esc(c.name)}${c.count != null ? ', ' + c.count + ' ' + things(c.sys, c.count) : ''}">
      ${sysIcon(tileIcon(c), i === sel ? 72 : 60)}<span><span class="short">${esc(c.short)}</span>${c.kind === 'sys' && c.name !== c.short ? `<span class="name">${esc(c.name)}</span>` : ''}<span class="count">${c.count != null ? c.count + ' ' + things(c.sys, c.count) : 'Random pick'}</span></span></button>`).join('');

  const lp = S.boot.last_played || {};
  const byLast = Object.entries(lp).sort((a, b) => b[1] - a[1]).map(([id]) => findGame(id)).filter(Boolean);
  const recent = byLast.filter((x) => !isMovies(x.s)).slice(0, 6);
  // Continue watching: shows with an episode to resume or a next one waiting, and
  // movies stopped partway. Finished movies and shows drop off.
  const watching = byLast.filter((x) => isMovies(x.s)).map((x) => {
    if ((x.g.episodes || []).length) {
      const n = showNext(x.g);
      const any = (x.g.episodes || []).some((_, i) => progOf(x.g, i));
      const allSeen = any && (x.g.episodes || []).every((_, i) => progOf(x.g, i)?.watched);
      if (allSeen) return null;
      const code = epCode(x.g.episodes[n.i]) || x.g.episodes[n.i].title || 'episode';
      return { ...x, line: n.kind === 'resume' ? `Resume ${code} · ${clock(n.pos)}` : n.kind === 'next' ? `Next: ${code}` : `Play ${code}` };
    }
    const p = progOf(x.g, null);
    if (p?.watched && !canResume(p)) return null;
    return { ...x, line: canResume(p) ? `Resume · ${clock(p.pos)}${p.len ? ` of ${clock(p.len)}` : ''}` : 'Movie' };
  }).filter(Boolean).slice(0, 6);
  const watchHtml = watching.length ? `<section class="row"><h3>Continue watching</h3><div class="recent">${watching.map(({ s: rs, g, line }) => `<button class="item" data-nav data-key="watch-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="home">
        ${cover(g, rs, 'nofav')}<span style="min-width:0;display:flex;flex-direction:column;gap:3px"><span class="t">${esc(g.title)}</span>
        <span class="s">${esc(line)} · ${ago(lp[g.id])}</span></span></button>`).join('')}</div></section>` : '';
  const recentHtml = recent.length
    ? `<div class="recent">${recent.map(({ s: rs, g }) => `<button class="item" data-nav data-key="recent-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="home">
        ${cover(g, rs, 'nofav')}<span style="min-width:0;display:flex;flex-direction:column;gap:3px"><span class="t">${esc(g.title)}</span>
        <span class="s">${esc(rs.short)} · ${fmtTime((S.boot.playtime || {})[g.id])} · ${ago(lp[g.id])}</span></span></button>`).join('')}</div>`
    : `<div class="empty-row">Games you play show up here.</div>`;

  const monthAgo = Date.now() / 1000 - 30 * 86400;
  const newest = allItems().filter((x) => x.g.added > monthAgo).sort((a, b) => b.g.added - a.g.added).slice(0, 8);
  const freshHtml = newest.length ? `<section class="row"><h3>Recently added</h3><div class="tiles" style="grid-template-columns:repeat(8,minmax(0,1fr))">${newest.map(({ s: fs, g }) =>
      `<button class="game" data-nav data-key="new-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="home">${cover(g, fs, 'nofav')}</button>`).join('')}</div></section>` : '';
  const favs = favItems().slice(0, 8);
  const favHtml = favs.length ? `<section class="row"><h3>Favorites</h3><div class="tiles" style="grid-template-columns:repeat(8,minmax(0,1fr))">${favs.map(({ s: fs, g }) =>
      `<button class="game" data-nav data-key="fav-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="home">${cover(g, fs, 'nofav')}</button>`).join('')}</div></section>` : '';

  // Ambient wall: the highlighted console's covers drifting slowly behind the header.
  const wallSrc = withCovers.length ? withCovers : [];
  const wstep = Math.max(1, Math.floor(wallSrc.length / 14));
  const wall = wallSrc.filter((_, i) => i % wstep === 0).slice(0, 14);
  const fresh = S.heroKey !== t.id;
  S.heroKey = t.id;
  const drift = -((Date.now() / 1000) % 90);
  const wallHtml = wall.length >= 3 ? `<div class="herowall ${fresh ? 'fresh' : ''}" aria-hidden="true">
      <div class="glow" style="background-image:url('${mediaUrl(wall[Math.floor(wall.length / 2)].g.cover)}')"></div>
      <div class="strip" style="animation-delay:${drift}s">${[...wall, ...wall].map((x) => `<img src="${mediaUrl(x.g.cover)}" alt="" decoding="async">`).join('')}</div></div>` : '';

  return topbar() + `<main class="main scroll" id="homescroll">
    <section class="hero" style="background:${t.color}1F">${wallHtml}
      <div class="hero-top">
        <div class="hero-text"><h2 style="display:flex;align-items:center;gap:16px"><span class="heroicon" style="background:${t.color}">${sysIcon(tileIcon(t), 52)}</span>${esc(t.name)}</h2><p>${status}</p>
          <div style="margin-top:10px;display:flex;gap:10px;flex-wrap:wrap"><button class="btn primary" data-nav data-key="openlib" data-act="tile" data-i="${sel}">${glyph('A')}${cta}</button>
          ${t.kind === 'sys' && t.sys.blocked && t.sys.download ? `<button class="btn" data-nav data-key="getemu" data-act="openurl" data-u="${esc(t.sys.download)}">${ICON.download} Get ${esc(t.sys.blocked.replace(/^Install |\.$/g, ''))}</button>` : ''}
          ${t.kind === 'sys' && t.sys.bios && !t.sys.blocked ? `<button class="btn" data-nav data-key="biosdir" data-act="biosdir" data-s="${esc(t.id)}" title="${esc(t.sys.bios.message)}">Open BIOS folder</button>` : ''}</div></div>
        <div class="fan" aria-hidden="true">${fan.map((x) => cover(x.g, x.s, 'nofav')).join('')}</div>
      </div>
      <div class="carousel" id="carousel">${cards}</div>
      <div class="cbar" id="cbar" hidden><div class="cthumb" id="cthumb"></div></div>
    </section>
    ${(S.boot.roots || []).filter((r) => !r.online).map((r) => `<div class="offline-banner" style="margin-top:22px">${esc(r.path)} isn't connected. Its games stay listed (greyed out) and come back when you plug the drive in.</div>`).join('')}
    <section class="row"><h3>Continue playing</h3>${recentHtml}</section>
    ${watchHtml}
    ${freshHtml}
    ${favHtml}
    ${(() => { const tc = topCats(3).slice(0, 18); return tc.length ? `<section class="row"><h3>Browse by category</h3><div class="cats big">${tc.map((c) =>
      `<button class="cat" data-nav data-key="hcat-${esc(c.name)}" data-act="category" data-type="tag" data-v="${esc(c.name)}">${esc(c.name)} <span class="n">${c.count}</span></button>`).join('')}</div></section>` : ''; })()}
    <div style="height:24px"></div>
  </main>` + hints('Press A on a console to open it', [['A', 'Select'], ['X', 'Search'], ['R3', 'Surprise me'], ['☰', 'Settings']]);
}

const RAIL = '#ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('');

// Highlight the letter currently at the top of the game list.
function markLetter(c) {
  document.querySelectorAll('.rail button.cur').forEach((b) => b.classList.remove('cur'));
  document.querySelector(`.rail button[data-l="${c}"]`)?.classList.add('cur');
}
function currentLetter() {
  const box = document.getElementById('gamesscroll');
  if (!box) return null;
  const top = box.getBoundingClientRect().top + 24;
  let cur = null;
  for (const a of box.querySelectorAll('[id^="L-"]')) { if (a.getBoundingClientRect().top <= top) cur = a.id.slice(2); else break; }
  if (!cur) cur = box.querySelector('[id^="L-"]')?.id.slice(2);
  return cur === 'num' ? '#' : cur;
}
function stepLetter(dir) {
  const avail = RAIL.filter((c) => document.getElementById(`L-${c === '#' ? 'num' : c}`));
  if (!avail.length) return;
  const i = avail.indexOf(currentLetter());
  const next = avail[Math.max(0, Math.min(avail.length - 1, i + dir))];
  ACT.jump({ l: next });
}

function layoutFor(id) { return (settings().layouts || {})[id] || 'tiles'; }

// Sorting the games screen (remembered).
// Movies and shows say "watched" where games say "played".
const sortLabel = (s, k) => (isMovies(s) ? SORTS[k].replace('played', 'watched') : SORTS[k]);
const SORTS = { title: 'A–Z', played: 'Most played', recent: 'Last played', year: 'Newest first', added: 'Recently added', listed: 'List order' };
const sortMode = () => settings().sort || 'title';
const maxPlayers = (g) => { const m = String(g.info?.players || '').match(/\d+/g); return m ? Math.max(...m.map(Number)) : 0; };
function sortItems(items, keepOrder) {
  const m = sortMode();
  if (m === 'title') return keepOrder ? items : items;
  const pt = S.boot.playtime || {}, lp = S.boot.last_played || {};
  const key = { played: (g) => pt[g.id] || 0, recent: (g) => lp[g.id] || 0, year: (g) => +(yearOf(g) || 0), added: (g) => g.added || 0 }[m];
  return [...items].sort((a, b) => key(b.g) - key(a.g) || sortTitle(a.g.title).localeCompare(sortTitle(b.g.title)));
}

function games() {
  const L = curList();
  if (!L) { S.screen = 'home'; return home(); }
  const s = L.tile;
  const isColl = s.kind !== 'sys';
  const layout = layoutFor(s.id);
  const pt0 = S.boot.playtime || {};
  const lp0 = S.boot.last_played || {};
  const filt = { fav: (x) => isFav(x.g.id), new: (x) => !pt0[x.g.id] && !lp0[x.g.id], multi: (x) => maxPlayers(x.g) >= 2 }[S.filter];
  const items = sortItems(filt ? L.items.filter(filt) : L.items, s.kind === 'coll');
  const list = items.map((x) => x.g);
  const bySort = sortMode() !== 'title' || s.kind === 'coll';
  const sysOf = {};
  items.forEach((x) => { sysOf[x.g.id] = x.s; });
  const so = (g) => sysOf[g.id];
  const pt = S.boot.playtime || {};
  const letters = new Set(bySort ? [] : list.map((g) => g.letter));
  const firstOf = {};
  list.forEach((g) => { if (!(g.letter in firstOf)) firstOf[g.letter] = g.id; });
  const anchor = (g) => (!bySort && firstOf[g.letter] === g.id ? ` id="L-${g.letter === '#' ? 'num' : g.letter}"` : '');
  const sub = (g) => (isColl ? so(g).short + ' · ' : '') + ([g.info?.year, g.info?.genre].filter(Boolean).join(' · ') || g.format.toUpperCase());

  let body;
  if (!list.length) body = `<div class="nofav">${S.filter === 'fav' || s.kind === 'fav' ? `No favorites ${isColl ? '' : 'here '}yet. Open a game and press ${glyph('Y')} to add it.` : S.filter === 'new' ? "You've played everything here." : S.filter === 'multi' ? 'No multiplayer games found here (player counts come from ScreenScraper).' : 'Nothing here yet.'}</div>`;
  else if (layout === 'list') body = `<div class="list">${list.map((g) => `<button class="game letter-anchor"${anchor(g)} data-nav data-key="g-${g.id}" data-act="open" data-id="${esc(g.id)}">
      ${cover(g, so(g), 'nofav')}<span class="t">${esc(g.title)}</span><span class="c">${esc(sub(g))}</span><span class="c r">${esc(pt[g.id] ? fmtTime(pt[g.id]) : '')}</span>
      <span class="f">${isFav(g.id) ? ICON.heartA : ''}</span></button>`).join('')}</div>`;
  else if (layout === 'cards') body = `<div class="cards">${list.map((g) => `<button class="game letter-anchor"${anchor(g)} data-nav data-key="g-${g.id}" data-act="open" data-id="${esc(g.id)}">
      ${cover(g, so(g))}<span class="body"><span class="t">${esc(g.title)}</span><span class="s">${esc(sub(g))}</span>
      ${g.info?.description ? `<span class="d">${esc(g.info.description)}</span>` : `<span class="s">${esc(pt[g.id] ? fmtTime(pt[g.id]) + ' played' : (g.launcher || so(g).emulator))}</span>`}</span></button>`).join('')}</div>`;
  else if (layout === 'shelf') body = `<div class="shelf">${list.map((g) => {
      const h = hash(g.title), H = 180 + (h % 56), W = 46 + (h % 3) * 6;
      return `<div class="slot letter-anchor"${anchor(g)}><button class="game" style="width:${W}px;height:${H}px;background:${PAL[h % PAL.length]}" data-nav data-key="g-${g.id}" data-act="open" data-id="${esc(g.id)}" aria-label="${esc(g.title)}">
        ${g.cover ? `<span class="cap"><img src="${mediaUrl(g.cover)}" alt="" loading="lazy"></span>` : ''}<span class="vt">${esc(g.title)}</span></button></div>`;
    }).join('')}</div>`;
  else body = `<div class="tiles">${list.map((g) => `<button class="game letter-anchor"${anchor(g)} data-nav data-key="g-${g.id}" data-act="open" data-id="${esc(g.id)}">
      ${cover(g, so(g))}<span class="t">${esc(g.title)}</span>${isColl ? `<span style="font-size:13px;color:var(--muted)">${esc(so(g).short)}</span>` : ''}</button>`).join('')}</div>`;

  const lay = (k, label) => `<button class="${layout === k ? 'on' : ''}" data-nav data-key="lay-${k}" data-act="layout" data-l="${k}" aria-label="${label}" title="${label}">${ICON[k]}</button>`;
  const rail = RAIL.map((c) => `<button class="${letters.has(c) ? 'has' : ''}" data-act="jump" data-l="${c}" tabindex="-1" aria-label="Jump to ${c === '#' ? 'numbers' : c}">${c}</button>`).join('');

  return topbar() + `<main class="main">
    <div class="gamesbar">
      <button class="iconbtn" data-nav data-key="gback" data-act="back" aria-label="Back to consoles">${ICON.back}</button>
      <span class="hdricon" style="background:${s.color}">${sysIcon(tileIcon(s), 30)}</span><h2>${esc(s.name)}</h2>
      <span style="color:var(--muted)">${L.items.length} ${things(s, L.items.length)}</span>
      <div class="spacer"></div>
      <button class="chip" data-nav data-key="rnd" data-act="random" data-scope="list">${ICON.dice} Random</button>
      <button class="chip" data-nav data-key="sortby" data-act="sortby" title="Change the order">${ICON.sort} ${esc(sortLabel(s, s.kind === 'coll' && sortMode() === 'title' ? 'listed' : sortMode()))}</button>
      <button class="chip ${!S.filter || S.filter === 'all' ? 'on' : ''}" data-nav data-key="f-all" data-act="filter" data-f="all">All</button>
      ${s.kind === 'fav' ? '' : `<button class="chip ${S.filter === 'fav' ? 'on' : ''}" data-nav data-key="f-fav" data-act="filter" data-f="fav">Favorites</button>`}
      <button class="chip ${S.filter === 'new' ? 'on' : ''}" data-nav data-key="f-new" data-act="filter" data-f="new">${isMovies(s) ? 'Not watched' : 'Not played'}</button>
      ${L.items.some((x) => maxPlayers(x.g) >= 2) ? `<button class="chip ${S.filter === 'multi' ? 'on' : ''}" data-nav data-key="f-multi" data-act="filter" data-f="multi">2+ players</button>` : ''}
      ${s.kind === 'coll' ? `<button class="chip" data-nav data-key="dellist" data-act="dellist" data-n="${esc(s.coll)}">Delete list</button>` : ''}
      <div class="seg" role="group" aria-label="Layout">${lay('tiles', 'Tiles')}${lay('list', 'List')}${lay('cards', 'Cards')}${lay('shelf', 'Shelf')}</div>
    </div>
    <div class="games-wrap"><nav class="rail" aria-label="Jump to letter">${rail}</nav><div class="games scroll" id="gamesscroll">${body}</div></div>
  </main>` + hints(`${s.kind === 'cat' ? 'Across your whole library · LT / RT letter' : (isColl ? 'Every console' : L.emulator === 'system' ? 'Opens with their store or your PC' : 'Launches with ' + L.emulator) + ' · LB / RB console · LT / RT letter'}`, [['A', 'Open'], ['B', 'Back'], ['Y', 'Favorite'], ['R3', 'Random'], ['Select', 'Layout']]);
}

function moreRow(title, items, key) {
  if (!items.length) return '';
  return `<section class="morerow"><h3>${esc(title)}</h3><div class="strip">${items.map(({ s, g }) =>
    `<button class="game" data-nav data-key="${key}-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="detail-${esc(S.game)}">${cover(g, s, 'nofav')}<span class="t">${esc(g.title)}</span><span class="s">${esc(s.short)}</span></button>`).join('')}</div></section>`;
}

function detail() {
  const found = findGame(S.game);
  if (!found) { S.screen = 'home'; return home(); }
  const { s, g } = found;
  const i = g.info || {};
  const pt = (S.boot.playtime || {})[g.id];
  const lp = (S.boot.last_played || {})[g.id];
  const year = yearOf(g), dev = devOf(g), series = seriesOf(g);
  const cats = catsOf(g);
  const extras = [g.copies > 1 ? `${g.copies} copies found, best one plays` : null, g.updates ? `${g.updates} update${g.updates > 1 ? 's' : ''}` : null, g.dlc ? `${g.dlc} DLC` : null].filter(Boolean);
  const chip = (type, value, label) => `<button class="cat" data-nav data-key="cat-${type}-${esc(value)}" data-act="category" data-type="${type}" data-v="${esc(value)}">${label || esc(value)}</button>`;

  // "More like this": same series, same category, same console.
  const others = allItems().filter((x) => x.g.id !== g.id);
  const used = new Set([g.id]);
  const take = (list, n) => { const out = []; for (const x of list) { if (!used.has(x.g.id)) { used.add(x.g.id); out.push(x); if (out.length >= n) break; } } return out; };
  const seriesItems = series ? take(others.filter((x) => catKey(seriesOf(x.g) || '') === catKey(series)).sort((a, b) => (yearOf(a.g) || '9999').localeCompare(yearOf(b.g) || '9999')), 14) : [];
  const firstCat = cats[0];
  const catPool = firstCat ? others.filter((x) => catsOf(x.g).some((c) => catKey(c) === catKey(firstCat))) : [];
  const catRow = take(seeded(catPool.filter((x) => x.s.id === s.id), g.id).concat(seeded(catPool.filter((x) => x.s.id !== s.id), g.id)), 14);
  const conRow = take(seeded(s.games.map((x) => ({ s, g: x })), g.id), 14);
  const rows = [
    seriesItems.length ? moreRow(`More ${series}`, seriesItems, 'ms') : '',
    catRow.length ? moreRow(`More ${firstCat}`, catRow, 'mc') : '',
    moreRow(`More from ${s.name}`, conRow, 'mn'),
  ].join('');

  const movie = isMovies(s);
  // TV: the chosen season (or arc), and its episodes with their place in the full list.
  const tvEps = g.episodes || [];
  const groups = showGroups(g);
  S.epGroup = S.epGroup || {};
  const group = groups.includes(S.epGroup[g.id]) ? S.epGroup[g.id] : groups[0];
  const shown = tvEps.map((e, i) => ({ e, i })).filter((x) => x.e.group === group);
  const next = tvEps.length ? showNext(g) : null;
  const mp = tvEps.length ? null : progOf(g, null);
  const thumbFallback = g.hero ? `<img src="${mediaUrl(g.hero)}" alt="" style="opacity:.35">` : '';
  const episodesHtml = !tvEps.length ? '' : `<section class="eps" aria-label="Episodes">
    <h3 style="font-family:var(--body);font-size:18px;font-weight:700">${tvEps.filter((e) => e.number != null).length} episodes</h3>
    ${groups.length > 1 ? `<div class="seasons" role="tablist">${groups.map((gr) => `<button class="chip ${gr === group ? 'on' : ''}" role="tab" aria-selected="${gr === group}" data-nav data-key="grp-${esc(gr)}" data-act="epgroup" data-id="${esc(g.id)}" data-g="${esc(gr)}">${esc(gr)}</button>`).join('')}</div>` : ''}
    ${isArc(g, group) ? `<div style="display:flex;align-items:center;gap:12px;flex-wrap:wrap"><span class="note">${g.arcs?.[group] ? `Matched to ${esc(arcLabel(g, g.arcs[group], shown.length))} on TMDB.` : 'TMDB names its seasons differently, so this arc has no episode details yet.'}</span>
      <button class="btn small" data-nav data-key="arcpick" data-act="arcpick" data-id="${esc(g.id)}" data-g="${esc(group)}">${g.arcs?.[group] ? 'Change season' : 'Which season is this?'}</button></div>` : ''}
    <div class="eplist">${shown.map(({ e, i }) => { const p = progOf(g, i); return `<div class="eprow"><button class="ep" data-nav data-key="ep-${i}" data-act="play" data-id="${esc(g.id)}" data-ep="${i}" title="${esc(e.file)}${canResume(p) ? ` · resumes at ${clock(p.pos)}` : ''}">
      <span class="thumb">${e.still ? `<img src="${mediaUrl(e.still)}" alt="" loading="lazy">` : `${thumbFallback}<span style="position:relative">${e.number ?? ICON.film}</span>`}${p?.watched ? `<span class="seen" aria-label="Watched">${ICON.check}</span>` : ''}${canResume(p) ? `<span class="pbar" aria-label="Stopped at ${clock(p.pos)}"><i style="width:${Math.round((100 * p.pos) / p.len)}%"></i></span>` : ''}</span>
      <span><span class="et">${epCode(e) ? `<span class="n">${esc(epCode(e))}</span>` : ''}${esc(e.title || (e.number != null ? 'Episode ' + e.number : e.file))}</span>${e.note ? `<span class="eo" style="font-weight:600">${esc(e.note)}</span>` : ''}${e.overview ? `<span class="eo">${esc(e.overview)}</span>` : ''}</span></button>
      <button class="epcheck ${p?.watched ? 'on' : ''}" data-nav data-key="w-${i}" data-act="watched" data-k="${esc(progKey(g, i))}" data-on="${p?.watched ? 1 : 0}" title="${p?.watched ? 'Watched · click to unmark' : 'Mark as watched'}" aria-pressed="${!!p?.watched}" aria-label="Watched">${ICON.check}</button></div>`; }).join('')}</div>
    ${group === 'Movies & specials' ? `<div style="display:flex;align-items:center;gap:12px;flex-wrap:wrap"><span class="note">Wrong movie, or no details?</span><button class="btn small" data-nav data-key="spfix" data-act="spfix" data-id="${esc(g.id)}">Fix match</button></div>` : ''}</section>`;
  const metaLine = [
    year ? `<span>${esc(year)}</span>` : null,
    i.runtime ? `<span>${esc(fmtRuntime(i.runtime))}</span>` : null,
    i.players ? `<span>${esc(i.players)} player${i.players === '1' ? '' : 's'}</span>` : null,
    dev ? chip('dev', dev, `by ${esc(dev)}`) : null,
    series ? chip('series', series, `${esc(series)} series`) : null,
  ].filter(Boolean).join('<span class="dotsep">·</span>');

  const gc = g.cover ? coverColor(mediaUrl(g.cover), g.id) : null;
  return topbar() + `<main class="main"${gc ? ` style="--accent:${gc.bg};--accent-ink:${gc.ink}"` : ''}><div class="backdrop" aria-hidden="true">${g.cover ? `<img src="${mediaUrl(g.cover)}" alt="">` : `<div class="tint" style="background:${s.color}"></div>`}<canvas id="bdcanvas" width="48" height="27"></canvas></div>
  <div class="detail scroll ${g.hero ? 'has-hero' : ''}" id="detailscroll">
   ${g.hero ? `<div class="herobanner" aria-hidden="true"><img src="${mediaUrl(g.hero)}" alt=""></div>` : ''}
   <div class="detail-top">
    <div class="left"><div class="media">${cover(g, s, 'nofav')}${g.video && videosOn() ? `<video class="preview" id="preview" muted loop playsinline preload="none" data-src="${mediaUrl(g.video)}"></video>${g.video_audio ? `<audio id="previewAudio" loop preload="none" data-src="${mediaUrl(g.video_audio)}"></audio>` : ''}
        <button class="sound" id="soundbtn" data-act="sound" aria-label="${soundOn() ? 'Mute video' : 'Turn on video sound'}" hidden>${soundOn() ? ICON.soundOn : ICON.soundOff}</button>` : ''}</div>
      <div class="coverbtns">
        <button class="btn small" data-nav data-key="mycover" data-act="mycover" data-id="${esc(g.id)}">${ICON.image}${g.custom_cover ? 'Change cover' : 'My own cover'}</button>
        ${g.custom_cover ? `<button class="btn small" data-nav data-key="resetcover" data-act="resetcover" data-id="${esc(g.id)}">Reset</button>` : ''}
        ${movie ? '' : `<button class="btn small" data-nav data-key="myvideo" data-act="myvideo" data-id="${esc(g.id)}">${ICON.film}${g.custom_video ? 'Change video' : 'My own video'}</button>
        ${g.custom_video ? `<button class="btn small" data-nav data-key="resetvideo" data-act="resetvideo" data-id="${esc(g.id)}">Reset</button>` : ''}
        <button class="btn small" data-nav data-key="fixgame" data-act="fixgame" data-id="${esc(g.id)}">${ICON.pencil}Fix name or cover</button>`}
      </div></div>
    <div class="right">
      <button class="back" data-nav data-key="dback" data-act="back">${ICON.back}${esc(S.cat ? curList().tile.name : s.name)}</button>
      ${g.logo ? `<h2 class="haslogo"><img class="gamelogo" src="${mediaUrl(g.logo)}" alt="${esc(g.title)}"></h2>` : `<h2>${esc(g.title)}</h2>`}
      ${metaLine ? `<div class="metaline">${metaLine}</div>` : ''}
      ${cats.length ? `<div class="cats" role="list" aria-label="Categories">${cats.map((c) => chip('tag', c)).join('')}</div>` : ''}
      ${i.description ? `<p class="desc">${esc(i.description)}</p>` : ''}
      ${g.warnings.map((w) => `<div class="warning">${esc(w)}</div>`).join('')}
      <div class="actions">
        ${next ? `<button class="btn primary" data-nav data-key="play" data-act="play" data-id="${esc(g.id)}" data-ep="${next.i}">${glyph('A')}${{ resume: 'Resume', next: 'Next:', play: 'Play' }[next.kind]} ${esc(epCode(tvEps[next.i]) || tvEps[next.i].title || 'episode')}${next.kind === 'resume' ? ` · ${clock(next.pos)}` : ''}</button>`
          : canResume(mp) ? `<button class="btn primary" data-nav data-key="play" data-act="play" data-id="${esc(g.id)}">${glyph('A')}Resume · ${clock(mp.pos)}</button>
            <button class="btn" data-nav data-key="restart" data-act="play" data-id="${esc(g.id)}" data-from="start">Start over</button>`
          : `<button class="btn primary" data-nav data-key="play" data-act="play" data-id="${esc(g.id)}">${glyph('A')}Play</button>`}
        ${movie && !tvEps.length ? `<button class="btn ${mp?.watched ? 'on' : ''}" data-nav data-key="mwatched" data-act="watched" data-k="${esc(g.id)}" data-on="${mp?.watched ? 1 : 0}" aria-pressed="${!!mp?.watched}">${ICON.check} ${mp?.watched ? 'Watched' : 'Mark as watched'}</button>` : ''}
        <button class="btn" data-nav data-key="fav" data-act="fav" data-id="${esc(g.id)}">${glyph('Y')}${isFav(g.id) ? 'Remove favorite' : 'Favorite'}</button>
        <button class="btn" data-nav data-key="addlist" data-act="lists" data-id="${esc(g.id)}">${ICON.list} ${Object.values(S.boot.collections || {}).some((l) => l.includes(g.id)) ? 'In your lists' : 'Add to list'}</button>
        ${S.spun === g.id ? `<button class="btn" data-nav data-key="spin" data-act="random" data-scope="${esc(S.spinScope || 'all')}">${ICON.dice} Spin again</button>` : ''}
      </div>
      ${s.blocked && !g.launcher ? `<div class="warning"><b>Can't play yet:</b> ${esc(s.blocked)} ${s.download ? `<button class="link" data-nav data-key="getemu" data-act="openurl" data-u="${esc(s.download)}">${s.download.includes('flathub.org') ? 'Get it from Flathub' : 'Open the download page'}</button> <span class="note">${s.download.includes('flathub.org') ? 'Install it from your software center (Discover, GNOME Software) or Flathub, then reopen Spine.' : movie ? 'Install it the normal way, then reopen Spine.' : 'Put it in your Emulators folder, and Spine finds it next time it starts.'}</span>` : ''}</div>` : ''}
      ${g.offline ? `<div class="warning"><b>Drive not connected.</b> Plug in the drive this ${movie ? 'movie' : 'game'} is on to ${movie ? 'watch' : 'play'} it.</div>` : ''}
      ${s.bios && !s.blocked && !g.launcher ? `<div class="warning"><b>Missing file:</b> ${esc(s.bios.message)} <span class="path">${esc(s.bios.folder)}</span> <button class="link" data-nav data-key="biosdir" data-act="biosdir" data-s="${esc(s.id)}">Open that folder</button></div>` : ''}
      <div class="facts">
        <div class="fact"><span class="k">${s.id === 'pc' ? 'Opens with' : movie ? 'Plays in' : 'Emulator'}</span><span class="v">${esc(g.launcher || (s.emulator === 'system' ? opener(g) : s.emulator))}</span>
          ${s.id === 'pc' ? '<span class="n">Your PC</span>' : `<span class="n">${g.launcher ? `Chosen for this ${things(s, 1)}` : movie ? 'Default player' : 'Console default'}</span><button class="link" data-nav data-key="chemu" data-act="emu" data-id="${esc(g.id)}">Change for this ${things(s, 1)}</button>`}</div>
        <div class="fact"><span class="k">${movie ? 'Watched' : 'Playtime'}</span><span class="v">${movie && !pt ? 'Not yet' : fmtTime(pt)}</span><span class="n">${lp ? (movie ? 'Last watched ' : 'Last played ') + ago(lp) : (g.format === 'steam' || g.format === 'epic') ? `${g.format === 'steam' ? 'Steam' : 'Epic'} keeps this one` : 'Tracked automatically'}</span></div>
        ${tvEps.length ? `<div class="fact"><span class="k">Files</span><span class="v">${tvEps.length} video${tvEps.length === 1 ? '' : 's'}</span><span class="n">${groups.length} ${groups.length === 1 ? 'group' : 'groups'}: ${esc(groups.join(', '))}</span></div>` : `<div class="fact"><span class="k">File</span><span class="v">${esc(g.format.toUpperCase())}${g.region ? ' · ' + esc(g.region) : ''}</span><span class="n" title="${esc(g.file)}">${esc(g.file)}</span>${extras.length ? `<span class="n">${esc(extras.join(' · '))}</span>` : ''}${isFav(g.id) ? `<span class="n" style="color:var(--accent);font-weight:600">${ICON.check} Favorite</span>` : ''}</div>`}
      </div>
    </div>
   </div>
   ${episodesHtml}
   ${shotsHtml(g)}
   ${movie ? '' : achHtml(g, s)}
   ${rows}
  </div></main>` + hints(`Pick a category to see every ${things(s, 1)} like it`, [['A', 'Select'], ['B', 'Back'], ['Y', 'Favorite']]);
}

function playingOverlay() {
  const p = S.playing;
  if (!p) return '';
  return `<div class="overlay" role="dialog" aria-label="Playing"><div class="box">
    ${p.error ? '' : '<div class="spinner"></div>'}
    <p style="font-size:15px;font-weight:600;letter-spacing:.08em;text-transform:uppercase">${p.error ? "Couldn't start" : p.started ? (p.movie ? 'Watching' : 'Playing') : (p.movie ? 'Opening' : 'Launching')}</p>
    <h2>${esc(p.title)}</h2>
    ${p.error ? `<p class="err">${esc(p.error)}</p>` : `<p>${esc(p.emulator === 'system' ? `Opening with ${p.opener || OS_NAME}` : p.emulator)}</p><p style="font-size:15px">${p.closing ? (p.movie ? 'Closing the player…' : 'Closing the game…') : exitComboOn() ? `Hold ${glyph('SELECT')} + ${glyph('START')} on your controller, or ${p.movie ? 'close the player' : 'quit the game'}, to come back.` : p.movie ? 'Close the player to come back here.' : 'Quit the game to come back here.'}</p>`}
    ${p.error || p.emulator === 'system' ? `<button class="btn" data-nav data-key="closeplay" data-act="closeplay">${glyph('B')}Back to library</button>` : ''}
  </div></div>`;
}

function settingsPanel() {
  if (S.panel !== 'settings') return '';
  const st = settings();
  const theme = st.theme || 'cream';
  const accent = st.accent || '#C8431F';
  const sw = ['#C8431F', '#2F5FBF', '#237A52', '#5B21B6', '#C99A1A', '#0E7490'].map((c) =>
    `<button class="swatch ${c === accent ? 'on' : ''}" style="background:${c}" data-nav data-key="acc-${c}" data-act="accent" data-c="${c}" aria-label="Accent ${c}"></button>`).join('');
  const fetching = S.fetch ? `${S.fetch.stage === 'tv' ? 'Getting TV details' : S.fetch.stage === 'movies' ? 'Getting movie posters' : S.fetch.stage === 'details' ? 'Looking up details' : 'Downloading covers'} ${S.fetch.done}/${S.fetch.total}` : null;
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Settings">
    <h2>Settings</h2>
    <div class="grp"><span class="k">Theme</span><div class="opts">
      <button class="chip ${theme === 'cream' ? 'on' : ''}" data-nav data-key="th-cream" data-act="theme" data-t="cream">Cream</button>
      <button class="chip ${theme === 'midnight' ? 'on' : ''}" data-nav data-key="th-midnight" data-act="theme" data-t="midnight">Midnight</button></div></div>
    <div class="grp"><span class="k">Accent</span><div class="opts">${sw}</div></div>
    <div class="grp"><span class="k">ROM folders</span><span class="note">Spine reads every folder here and puts the games together. Folders on a drive that's unplugged keep their games listed until it's back.</span>
      <div class="roots">${(S.boot.roots || []).map((r, i) => `<div class="root ${r.online ? '' : 'off'}"><span class="dot"></span><span class="rp">${esc(r.path)}</span><span class="rs">${r.online ? 'Connected' : 'Not connected'}</span>
        <button class="btn small" data-nav data-key="rmroot-${i}" data-act="rmroot" data-p="${esc(r.path)}">Remove</button></div>`).join('')}</div>
      <div class="opts">
      <button class="btn" data-nav data-key="addroot" data-act="addroot">${ICON.plus} Add folder</button>
      <button class="btn" data-nav data-key="rescan" data-act="rescan">Rescan</button>
      <button class="chip ${settings().autoScan !== false ? 'on' : ''}" data-nav data-key="as-tog" data-act="autoscan">${settings().autoScan !== false ? 'Watching for new games' : 'Not watching for new games'}</button></div></div>
    <div class="grp"><span class="k">Movies</span><span class="note">Folders with your movie files. They show up as a Movies shelf on Home and play in VLC or mpv. Posters and details come from TMDB (add your free key under Scrapers).</span>
      <div class="roots">${(S.boot.movie_roots || []).map((r, i) => `<div class="root ${r.online ? '' : 'off'}"><span class="dot"></span><span class="rp">${esc(r.path)}</span><span class="rs">${r.online ? 'Connected' : 'Not connected'}</span>
        <button class="btn small" data-nav data-key="rmmovie-${i}" data-act="rmmovieroot" data-p="${esc(r.path)}">Remove</button></div>`).join('')}</div>
      <div class="opts"><button class="btn" data-nav data-key="addmovieroot" data-act="addmovieroot">${ICON.plus} Add movie folder</button></div></div>
    <div class="grp"><span class="k">TV</span><span class="note">Folders with your shows. Each folder inside is one show; Spine sorts the episodes into seasons and finds names and thumbnails on TMDB.</span>
      <div class="roots">${(S.boot.tv_roots || []).map((r, i) => `<div class="root ${r.online ? '' : 'off'}"><span class="dot"></span><span class="rp">${esc(r.path)}</span><span class="rs">${r.online ? 'Connected' : 'Not connected'}</span>
        <button class="btn small" data-nav data-key="rmtv-${i}" data-act="rmtvroot" data-p="${esc(r.path)}">Remove</button></div>`).join('')}</div>
      <div class="opts"><button class="btn" data-nav data-key="addtvroot" data-act="addtvroot">${ICON.plus} Add TV folder</button></div></div>
    <div class="grp"><span class="k">PC stores</span><span class="note">Games installed with Steam, Epic Games and GOG show up in PC Games, ready to play.</span><div class="opts">
      <button class="chip ${storesOn() ? 'on' : ''}" data-nav data-key="st-on" data-act="stores" data-v="on">Include them</button>
      <button class="chip ${!storesOn() ? 'on' : ''}" data-nav data-key="st-off" data-act="stores" data-v="off">Off</button></div></div>
    <div class="grp"><span class="k">Box art &amp; details</span><span class="note">${esc(fetching || 'Covers come from the free libretro collection and Steam. Connect SteamGridDB or ScreenScraper under Scrapers for the rest.')}</span><div class="opts">
      <button class="btn" data-nav data-key="fetchnow" data-act="fetch">Get missing covers</button>
      <button class="btn" data-nav data-key="scrapers" data-act="scrapers">Scrapers</button></div></div>
    <div class="grp"><span class="k">Videos</span><span class="note">Game pages play a gameplay video or trailer when one is available.</span><div class="opts">
      <button class="chip ${videosOn() ? 'on' : ''}" data-nav data-key="vid-on" data-act="videos" data-v="on">Play videos</button>
      <button class="chip ${!videosOn() ? 'on' : ''}" data-nav data-key="vid-off" data-act="videos" data-v="off">Off</button>
      <button class="chip ${soundOn() ? 'on' : ''}" data-nav data-key="vid-sound" data-act="sound">${soundOn() ? 'Sound on' : 'Sound off'}</button></div></div>
    ${(S.boot.hidden || []).length ? `<div class="grp"><span class="k">Hidden games</span><div class="roots">${S.boot.hidden.map((h) => `<div class="root"><span></span><span class="rp">${esc(h.title)}</span><span class="rs">${esc(h.system)}</span>
      <button class="btn small" data-nav data-key="uh-${esc(h.id)}" data-act="unhide" data-id="${esc(h.id)}">Show again</button></div>`).join('')}</div></div>` : ''}
    <div class="grp"><span class="k">Sounds</span><span class="note">Little clicks and chimes as you move around, like a console menu.</span><div class="opts">
      ${[['off', 'Off'], ['soft', 'Soft'], ['normal', 'Normal']].map(([v, l]) => `<button class="chip ${(settings().uiSounds || 'soft') === v ? 'on' : ''}" data-nav data-key="sfx-${v}" data-act="uisounds" data-v="${v}">${l}</button>`).join('')}</div></div>
    <div class="grp"><span class="k">Controller</span><span class="note">Hold Select + Start for a second during a game to close it and come back to Spine. Your game is closed the normal way, so it saves like quitting from its menu.</span><div class="opts">
      <button class="chip ${exitComboOn() ? 'on' : ''}" data-nav data-key="ex-on" data-act="exitcombo" data-v="on">Select + Start quits</button>
      <button class="chip ${!exitComboOn() ? 'on' : ''}" data-nav data-key="ex-off" data-act="exitcombo" data-v="off">Off</button></div></div>
    <div class="grp"><span class="k">Interface size</span><span class="note">Auto fits Spine to your screen, from a laptop to a 4K TV.</span><div class="opts">
      ${['auto', '1', '1.25', '1.5', '1.75', '2'].map((v) => `<button class="chip ${String(st.ui_scale || 'auto') === v ? 'on' : ''}" data-nav data-key="sz-${v}" data-act="uiscale" data-v="${v}">${v === 'auto' ? 'Auto' : Math.round(v * 100) + '%'}</button>`).join('')}</div></div>
    <div class="grp"><span class="k">Updates</span><span class="note">${updateNote()}</span><div class="opts">
      ${S.update?.available ? `<button class="btn primary" data-nav data-key="upd-go" data-act="installupdate">${ICON.download} Update to ${esc(S.update.latest)}</button>` : ''}
      <button class="btn" data-nav data-key="upd-check" data-act="checkupdate">Check now</button>
      <button class="chip ${settings().autoUpdate !== false ? 'on' : ''}" data-nav data-key="upd-auto" data-act="autoupdate">${settings().autoUpdate !== false ? 'Checks when Spine starts' : 'Not checking automatically'}</button></div></div>
    <div class="grp"><span class="k">Window</span><div class="opts">
      <button class="btn" data-nav data-key="fs" data-act="fullscreen">${st.fullscreen ? 'Leave fullscreen' : 'Fullscreen (couch mode)'}</button>
      <button class="btn" data-nav data-key="quit" data-act="quit">Quit Spine</button></div></div>
  </aside></div>`;
}

function scrapersPanel() {
  if (S.panel !== 'scrapers') return '';
  const a = S.accounts;
  if (!a) return `<div class="panel" data-act="closepanel"><aside class="sheet"><h2>Scrapers</h2><div class="spinner"></div></aside></div>`;
  const field = (key, label, type, value, placeholder) => `<label class="field"><span>${label}</span>
    <input type="${type}" id="acc-${key}" data-nav data-key="acc-${key}" value="${esc(value || '')}" placeholder="${esc(placeholder || '')}" autocomplete="off" spellcheck="false"></label>`;
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Scrapers">
    <button class="back" data-nav data-key="scback" data-act="settings">${ICON.back}Settings</button>
    <h2>Scrapers</h2>
    <div class="grp"><span class="k">SteamGridDB · covers</span>
      <span class="note">Free. Sign in at steamgriddb.com with Steam, open Preferences → API, and copy your key here.</span>
      ${field('sgdb', 'API key', 'password', '', a.steamgriddb_key ? `Saved (${a.steamgriddb_key}). Type to replace.` : 'Paste your key')}
      ${a.steamgriddb_key ? `<button class="link" data-nav data-key="clr-sgdb" data-act="clearacc" data-f="steamgriddb_key">Remove key</button>` : ''}</div>
    <div class="grp"><span class="k">ScreenScraper · names, details, covers</span>
      <span class="note">${a.screenscraper_builtin ? 'Already on: this copy of Spine has its ScreenScraper access built in. Add your free screenscraper.fr account for faster lookups and a bigger daily allowance.' : 'Your screenscraper.fr account, plus the developer key the ScreenScraper team sends you for Spine.'}</span>
      ${field('ssuser', 'Username', 'text', a.screenscraper_user, 'Username')}
      ${field('sspass', 'Password', 'password', '', a.screenscraper_password ? 'Saved. Type to replace.' : 'Password')}
      ${a.screenscraper_builtin ? '' : `${field('ssdev', 'Developer ID', 'text', a.screenscraper_devid, 'From the ScreenScraper team')}
      ${field('ssdevpw', 'Developer password', 'password', '', a.screenscraper_devpassword ? 'Saved. Type to replace.' : 'From the ScreenScraper team')}`}
      <span class="note">${a.screenscraper_builtin
        ? (a.screenscraper_user && a.screenscraper_password ? 'All set, using your account.' : a.screenscraper_user ? 'Add your password to use your account.' : 'Working without an account.')
        : (a.screenscraper_user && a.screenscraper_password && a.screenscraper_devid && a.screenscraper_devpassword ? 'All set.' : 'ScreenScraper turns on once all four are filled in.')}</span></div>
    <div class="grp"><span class="k">RetroAchievements · trophies</span>
      <span class="note">Free. Sign in at retroachievements.org, open Settings → Authentication, and copy your Web API key. Game pages then show your achievements.</span>
      ${field('rauser', 'Username', 'text', a.retroachievements_user, 'Username')}
      ${field('rakey', 'Web API key', 'password', '', a.retroachievements_key ? `Saved (${a.retroachievements_key}). Type to replace.` : 'Paste your key')}
      ${a.retroachievements_key ? `<button class="link" data-nav data-key="clr-ra" data-act="clearacc" data-f="retroachievements_key">Remove key</button>` : ''}</div>
    <div class="grp"><span class="k">TMDB · movie posters &amp; details</span>
      <span class="note">Free. Make an account at themoviedb.org, open Settings → API, ask for a developer key (personal use), and copy the "API Key" here.</span>
      <button class="link" data-nav data-key="tmdbsite" data-act="openurl" data-u="https://www.themoviedb.org/settings/api">Open TMDB's API page</button>
      ${field('tmdb', 'API key', 'password', '', a.tmdb_key ? `Saved (${a.tmdb_key}). Type to replace.` : 'Paste your key')}
      ${a.tmdb_key ? `<button class="link" data-nav data-key="clr-tmdb" data-act="clearacc" data-f="tmdb_key">Remove key</button>` : ''}</div>
    <div class="opts"><button class="btn primary" data-nav data-key="accsave" data-act="saveacc">${glyph('A')}Save &amp; get covers</button></div>
  </aside></div>`;
}

function arcPanel() {
  if (S.panel !== 'arc' || !S.arcPick) return '';
  const found = findGame(S.arcPick.id);
  if (!found) return '';
  const g = found.g, group = S.arcPick.group;
  const count = (g.episodes || []).filter((e) => e.group === group && e.number != null).length;
  const cur = g.arcs?.[group];
  const opts = [];
  for (const t of g.tmdb_seasons || []) {
    opts.push({ season: t.number, start: 1 });
    if (count < t.episodes) opts.push({ season: t.number, start: t.episodes - count + 1 });
  }
  const row = (o, i) => { const on = cur && cur.season === o.season && cur.start === o.start; const t = (g.tmdb_seasons || []).find((x) => x.number === o.season);
    return `<button class="choice ${on ? 'on' : ''}" data-nav data-key="arc-${i}" data-act="setarc" data-s="${o.season}" data-st="${o.start}">
      <span class="t">${esc(arcLabel(g, o, count))}</span><span class="n">${t ? `${t.episodes} episodes on TMDB` : ''}${t && count < t.episodes ? (o.start === 1 ? ' · this arc is the first part' : ' · this arc is the last part') : ''}</span></button>`; };
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Which season is this arc?">
    <h2>Which season is ${esc(group)}?</h2>
    <span class="note">${count} episode${count === 1 ? '' : 's'} in this folder. Pick the TMDB season it matches, and Spine gets its episode names, summaries and thumbnails.</span>
    ${opts.length ? `<div class="choices">${opts.map(row).join('')}
      ${cur ? `<button class="choice" data-nav data-key="arc-none" data-act="setarc" data-s=""><span class="t">Not matched</span><span class="n">Forget this match</span></button>` : ''}</div>`
      : `<p class="note">Spine hasn't downloaded TMDB's season list for this show yet. Wait for "Getting TV shows" to finish, then try again.</p>`}
  </aside></div>`;
}

function specialPanel() {
  if (S.panel !== 'special' || !S.spFix) return '';
  const found = findGame(S.spFix);
  if (!found) return '';
  const sps = (found.g.episodes || []).filter((e) => e.number == null);
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Fix match">
    <h2>Fix match</h2>
    <span class="note">Spine looks these up among TMDB's movies. If one is wrong or missing, type its English title (for example "Demon Slayer Mugen Train") and search again.</span>
    ${sps.map((e, i) => `<div class="grp"><span class="k">${esc(e.file)}</span>
      ${e.note ? `<span class="note">Now: ${esc(e.title || '')}${e.note.startsWith('Movie') ? ' · ' + esc(e.note) : ''}</span>` : ''}
      <label class="field"><span>Search TMDB for</span><input type="text" id="sq-${i}" data-nav data-key="sq-${i}" value="${esc(e.query || e.title || '')}" autocomplete="off" spellcheck="false"></label>
      <div class="opts"><button class="btn primary" data-nav data-key="sqgo-${i}" data-act="setspecial" data-i="${i}">Search again</button></div></div>`).join('')}
  </aside></div>`;
}

function emulatorPanel() {
  if (S.panel !== 'emulator' || !S.emu) return '';
  const found = findGame(S.emu.id);
  if (!found) return '';
  const opts = S.emu.options;
  const row = (key, label, note, on) => `<button class="choice ${on ? 'on' : ''}" data-nav data-key="emu-${key || 'default'}" data-act="setemu" data-k="${esc(key || '')}">
      <span class="t">${esc(label)}</span><span class="n">${esc(note)}</span></button>`;
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Choose emulator">
    <h2>Emulator for ${esc(found.g.title)}</h2>
    <span class="note">Only this game changes. Everything else on ${esc(found.s.name)} keeps using ${esc(found.s.emulator)}.</span>
    <div class="choices">
      ${row('', `Console default (${opts[0]?.label || found.s.emulator})`, 'Recommended', !S.emu.current)}
      ${opts.map((o) => row(o.key, o.label, o.blocked ? o.blocked : 'Ready', S.emu.current === o.key)).join('')}
    </div>
  </aside></div>`;
}

// ---- Updates from Spine's GitHub releases
function updateNote() {
  const u = S.update;
  const cur = `You have Spine ${S.version || ''}.`;
  if (!u) return cur;
  if (u.busy) return u.busy;
  if (u.error) return `${cur} ${u.error}`;
  if (u.available) return `Spine ${u.latest} is ready. Spine downloads it, checks it, and restarts.${u.notes ? `<span class="relnotes">${u.notes.split('\n').filter(Boolean).slice(0, 5).map((l) => `<span>• ${esc(l)}</span>`).join('')}</span>` : ''}`;
  return `${cur} That's the newest.`;
}
async function checkUpdate(quiet) {
  try {
    S.update = await api.call('check_update');
    if (S.update.available) toast(`Spine ${S.update.latest} is available. Open Settings to update.`, 6000);
    else if (!quiet) toast("You have the newest Spine");
  } catch (e) { S.update = quiet ? null : { error: String(e) }; }
  if (S.panel === 'settings') render();
}

// ---- Screenshots: a strip on the game page, full size in a viewer.
function shotsHtml(g) {
  if (!g.shots?.length) return '';
  const pix = findGame(g.id)?.s?.id !== 'pc' ? ' pixel' : '';
  return `<section class="morerow shots"><h3>Screenshots</h3><div class="shotstrip${pix}">${g.shots.map((u, i) =>
    `<button class="shot" data-nav data-key="shot-${i}" data-act="openshot" data-i="${i}" aria-label="Screenshot ${i + 1} of ${g.shots.length}"><img src="${mediaUrl(u)}" alt="" loading="lazy"></button>`).join('')}</div></section>`;
}
function shotViewer() {
  if (S.shot == null || S.screen !== 'detail') return '';
  const g = findGame(S.game)?.g;
  if (!g?.shots?.length) return '';
  const i = Math.max(0, Math.min(S.shot, g.shots.length - 1));
  const pix = findGame(S.game)?.s?.id !== 'pc' ? ' pixel' : '';
  return `<div class="viewer${pix}" role="dialog" aria-label="Screenshot ${i + 1} of ${g.shots.length}" data-act="closeshot">
    <img src="${mediaUrl(g.shots[i])}" alt="">
    <button class="vnav prev" data-nav data-key="v-prev" data-act="stepshot" data-d="-1" aria-label="Previous">${ICON.back}</button>
    <button class="vnav next" data-nav data-key="v-next" data-act="stepshot" data-d="1" aria-label="Next">${ICON.back}</button>
    <div class="vbar"><span>${i + 1} / ${g.shots.length}</span><span>${glyph('B')} Close</span></div></div>`;
}

// ---- Colors from the cover: the game page takes its main color.
const coverColors = {};
function coverColor(url, id) {
  if (url in coverColors) return coverColors[url];
  coverColors[url] = null;
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    try {
      const c = document.createElement('canvas'); c.width = 48; c.height = 64;
      const x = c.getContext('2d', { willReadFrequently: true });
      x.imageSmoothingQuality = 'high';
      x.drawImage(img, 0, 0, 48, 64);
      coverColors[url] = pickColor(x.getImageData(0, 10, 48, 50).data); // skip the console banner along the top and the rating box
    } catch (e) { coverColors[url] = null; }
    if (coverColors[url] && S.screen === 'detail' && S.game === id && !S.panel && !S.playing && S.shot == null) {
      const m = document.querySelector('main.main'); if (m) { m.style.setProperty('--accent', coverColors[url].bg); m.style.setProperty('--accent-ink', coverColors[url].ink); }
    }
  };
  img.src = url;
  return null;
}
// The most colorful common color, kept dark enough for white text on top.
function pickColor(d) {
  const buckets = {};
  for (let i = 0; i < d.length; i += 4) {
    const [r, g, b] = [d[i], d[i + 1], d[i + 2]];
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
    const sat = mx ? (mx - mn) / mx : 0;
    if (sat < 0.28 || mx < 40) continue; // skip greys, whites, blacks
    const k = `${r >> 6},${g >> 6},${b >> 6}`;
    const bk = buckets[k] || (buckets[k] = { n: 0, r: 0, g: 0, b: 0, w: 0 });
    const w = sat * sat * (mx / 255);
    bk.n++; bk.w += w; bk.r += r * w; bk.g += g * w; bk.b += b * w;
  }
  const best = Object.values(buckets).sort((a, b) => b.w - a.w)[0];
  if (!best || best.n < 30) return null;
  let [r, g, b] = [best.r / best.w, best.g / best.w, best.b / best.w];
  // Keep the color vivid; only tame very light or very dark ones.
  const lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  const f = lum > 0.7 ? 0.7 / lum : lum < 0.14 ? 0.14 / Math.max(lum, 0.01) : 1;
  [r, g, b] = [r, g, b].map((v) => Math.round(Math.min(255, v * f)));
  const L = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  return { bg: `rgb(${r}, ${g}, ${b})`, ink: L > 0.5 ? '#16120E' : '#fff' };
}

// RetroAchievements on the game page: loaded once per game, in the background.
function achHtml(g, s) {
  S.ach = S.ach || {};
  const a = S.ach[g.id];
  if (a === undefined) { S.ach[g.id] = 'loading'; loadAch(g.id); return ''; }
  if (a === 'loading' || !a) return '';
  if (a.error) return `<section class="morerow ach"><h3>Achievements</h3><p class="note">${esc(a.error)}</p></section>`;
  const pct = a.total ? Math.round((100 * a.earned) / a.total) : 0;
  return `<section class="morerow ach"><h3>Achievements <span class="achsum">${a.earned} of ${a.total} · ${a.points_earned} / ${a.points} points</span></h3>
    <div class="achbar"><i style="width:${pct}%"></i></div>
    <div class="strip achs">${a.achievements.map((x, i) => `<button class="achv ${x.earned ? 'got' : ''}" data-nav data-key="ach-${i}" data-act="achinfo" data-i="${i}" title="${esc(x.title)}: ${esc(x.description)}">
      <img src="${esc(x.badge)}" alt="" loading="lazy" referrerpolicy="no-referrer"><span class="at">${esc(x.title)}</span><span class="ap">${x.points} pts${x.hardcore ? ' · hardcore' : ''}</span></button>`).join('')}</div>
    <button class="link" data-nav data-key="ra-open" data-act="openurl" data-u="${esc(a.url)}">See it on RetroAchievements</button></section>`;
}
async function loadAch(id) {
  let r;
  try { r = await api.call('achievements', { id }); } catch (e) { r = { error: String(e) }; }
  S.ach[id] = r || null;
  if (r && S.screen === 'detail' && S.game === id && !S.panel && !S.playing) render();
}

// Your own lists ("Couch co-op", "Finish this year").
function listsPanel() {
  if (S.panel !== 'lists' || !S.lists) return '';
  const found = findGame(S.lists.id);
  if (!found) return '';
  const names = Object.keys(S.boot.collections || {});
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Lists">
    <h2>Lists for ${esc(found.g.title)}</h2>
    <span class="note">Make your own shelves, like “Couch co-op” or “Finish this year”. Each list shows up next to Favorites on the home screen.</span>
    ${names.length ? `<div class="choices">${names.map((n, i) => { const on = S.boot.collections[n].includes(found.g.id); return `<button class="choice ${on ? 'on' : ''}" data-nav data-key="ls-${i}" data-act="togglelist" data-n="${esc(n)}">
      <span class="t">${on ? ICON.check + ' ' : ''}${esc(n)}</span><span class="n">${S.boot.collections[n].length} game${S.boot.collections[n].length === 1 ? '' : 's'}${on ? ' · in this list' : ''}</span></button>`; }).join('')}</div>` : ''}
    <div class="grp"><span class="k">New list</span>
      <label class="field"><input type="text" id="nl-name" data-nav data-key="nl-name" data-act="newlist" placeholder="List name" maxlength="40" autocomplete="off" spellcheck="false"></label>
      <div class="opts"><button class="btn" data-nav data-key="nl-make" data-act="newlist">${ICON.plus} Make list and add</button></div></div>
  </aside></div>`;
}

// "Fix name or cover": rename, pick the right cover from every source, or hide the game.
function editPanel() {
  if (S.panel !== 'edit' || !S.edit) return '';
  const found = findGame(S.edit.id);
  if (!found) return '';
  const { g, s } = found;
  const E = S.edit;
  const results = E.loading ? '<div class="spinner" style="margin:18px auto"></div>'
    : E.choices == null ? ''
    : !E.choices.length ? `<p class="note">Nothing found for “${esc(E.query)}”. Try fewer or different words${E.sgdb ? '' : ', or add a SteamGridDB key under Scrapers for more results'}.</p>`
    : `<div class="picks">${E.choices.map((c, i) => `<button class="pick" data-nav data-key="pk-${i}" data-act="usecover" data-i="${i}" title="${esc(c.label)}">
        <img src="${esc(c.thumb)}" alt="" loading="lazy" referrerpolicy="no-referrer"><span class="src">${esc(c.source)}</span><span class="lb">${esc(c.label)}</span></button>`).join('')}</div>`;
  return `<div class="panel" data-act="closepanel"><aside class="sheet wide" role="dialog" aria-label="Fix this game">
    <h2>Fix ${esc(g.title)}</h2>
    <div class="grp"><span class="k">Name</span><span class="note">Shown everywhere in Spine. ${g.renamed ? 'You named this one.' : 'Right now it comes from the file name.'}</span>
      <label class="field"><input type="text" id="ed-name" data-nav data-key="ed-name" data-act="savename" value="${esc(g.title)}" autocomplete="off" spellcheck="false" maxlength="200"></label>
      <div class="opts"><button class="btn" data-nav data-key="ed-save" data-act="savename">Save name</button>
      ${g.renamed ? `<button class="btn" data-nav data-key="ed-reset" data-act="resetname">Use the file's name</button>` : ''}</div></div>
    <div class="grp"><span class="k">Cover</span><span class="note">Search by name and pick the right one${libretroSys(s.id) ? ' (libretro lists every region and version)' : ''}.</span>
      <label class="field"><input type="text" id="ed-q" data-nav data-key="ed-q" data-act="findcovers" value="${esc(E.query)}" autocomplete="off" spellcheck="false"></label>
      <div class="opts"><button class="btn" data-nav data-key="ed-find" data-act="findcovers">${ICON.search} Search covers</button></div>
      ${results}</div>
    <div class="grp"><span class="k">Hide</span><span class="note">For junk files, demos or duplicates. Nothing is deleted, and you can bring it back from Settings.</span>
      <div class="opts"><button class="btn" data-nav data-key="ed-hide" data-act="hidegame">Hide this game</button></div></div>
  </aside></div>`;
}
const libretroSys = (id) => !['pc', 'switch', 'wiiu', 'ps3', 'psvita', 'xbox'].includes(id);

function searchPanel() {
  if (S.panel !== 'search') return '';
  const q = S.query.trim().toLowerCase();
  const hits = [];
  if (q) for (const s of systems()) for (const g of s.games) if (g.title.toLowerCase().includes(q)) { hits.push({ s, g }); if (hits.length >= 60) break; }
  return `<div class="search-panel" role="dialog" aria-label="Search">
    <div class="top">${ICON.search}<label class="sr" for="q">Search your library</label><input id="q" data-nav data-key="q" placeholder="Search your library" value="${esc(S.query)}" autocomplete="off" spellcheck="false">
      <button class="btn" data-nav data-key="qclose" data-act="closepanel">${glyph('B')}Close</button></div>
    <div class="res">${q ? (() => { const cs = topCats(1).filter((c) => c.name.toLowerCase().includes(q)).slice(0, 10); return cs.length ? `<div class="cats" style="margin-bottom:22px">${cs.map((c) =>
      `<button class="cat" data-nav data-key="scat-${esc(c.name)}" data-act="category" data-type="tag" data-v="${esc(c.name)}">${esc(c.name)} <span class="n">${c.count}</span></button>`).join('')}</div>` : ''; })() : ''}<div class="tiles">${hits.map(({ s, g }) => `<button class="game" data-nav data-key="h-${g.id}" data-act="open" data-id="${esc(g.id)}" data-from="search">
      ${cover(g, s)}<span class="t">${esc(g.title)}</span><span style="font-size:13px;color:var(--muted)">${esc(s.short)}</span></button>`).join('')}</div>
      ${q && !hits.length ? '<div class="nofav">Nothing matches that.</div>' : ''}</div>
  </div>`;
}

// ---------------------------------------------------------------------------
// Rendering & focus
// ---------------------------------------------------------------------------
// Interface size: scale everything to the window so 1080p, 4K and TVs all look right.
function applyScale() {
  const pref = settings().ui_scale || 'auto';
  const z = pref === 'auto' ? Math.max(1, Math.min(2.4, Math.min(innerWidth / 1440, innerHeight / 860))) : +pref;
  document.documentElement.style.setProperty('--z', z);
  document.documentElement.style.zoom = z;
}
window.addEventListener('resize', () => applyScale());

function applyTheme() {
  applyScale();
  const st = settings();
  document.documentElement.dataset.theme = st.theme || 'cream';
  document.documentElement.style.setProperty('--accent', st.accent || '#C8431F');
}

let carLeft = null;
// The console scroll bar: thumb size and place follow the carousel.
function syncCbar() {
  const car = document.getElementById('carousel'), bar = document.getElementById('cbar'), th = document.getElementById('cthumb');
  if (!car || !bar || !th) return;
  const max = car.scrollWidth - car.clientWidth;
  bar.hidden = max <= 2;
  if (bar.hidden) return;
  const w = bar.clientWidth, tw = Math.max(48, w * car.clientWidth / car.scrollWidth);
  th.style.width = tw + 'px';
  th.style.transform = `translateX(${(car.scrollLeft / max) * (w - tw)}px)`;
}
function setupCbar(car) {
  const bar = document.getElementById('cbar'), th = document.getElementById('cthumb');
  if (!bar || !th) return;
  syncCbar();
  requestAnimationFrame(syncCbar); // after layout settles
  // Positions in the bar's own (unzoomed) pixels.
  const toBar = (clientX) => (clientX - bar.getBoundingClientRect().left) * bar.clientWidth / bar.getBoundingClientRect().width;
  const ratio = () => { const tw = th.offsetWidth; return (car.scrollWidth - car.clientWidth) / Math.max(1, bar.clientWidth - tw); };
  bar.onpointerdown = (e) => {
    e.preventDefault();
    const tw = th.offsetWidth;
    const thumbLeft = car.scrollLeft / ratio();
    let grab = toBar(e.clientX) - thumbLeft;
    if (e.target !== th) grab = tw / 2; // clicked the track: jump the thumb there
    const move = (ev) => { car.style.scrollBehavior = 'auto'; car.scrollLeft = (toBar(ev.clientX) - grab) * ratio(); };
    move(e);
    bar.classList.add('drag');
    bar.setPointerCapture(e.pointerId);
    bar.onpointermove = move;
    bar.onpointerup = bar.onpointercancel = () => { bar.classList.remove('drag'); bar.onpointermove = null; car.style.scrollBehavior = ''; };
  };
}
window.addEventListener('resize', () => syncCbar());
function render() {
  applyTheme();
  const app = document.getElementById('app');
  const keep = document.activeElement?.dataset?.key;
  if (keep && !S.focusKey) S.focusKey = keep;
  const scrollers = {};
  app.querySelectorAll('.scroll').forEach((el) => { if (el.id) scrollers[el.id] = el.scrollTop; });
  const screens = { welcome, scanning, home, games, detail, loading: () => '' };
  app.innerHTML = (screens[S.screen] || home)() + playingOverlay() + settingsPanel() + scrapersPanel() + emulatorPanel() + arcPanel() + specialPanel() + editPanel() + listsPanel() + shotViewer() + searchPanel();
  silenceDetached();
  Object.entries(scrollers).forEach(([id, top]) => { const el = document.getElementById(id); if (el) el.scrollTop = top; });
  const car = document.getElementById('carousel');
  if (car) {
    if (carLeft != null) { car.style.scrollBehavior = 'auto'; car.scrollLeft = carLeft; car.style.scrollBehavior = ''; }
    car.onscroll = () => { carLeft = car.scrollLeft; syncCbar(); };
    setupCbar(car);
    car.onwheel = (e) => {
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
      e.preventDefault();
      car.scrollBy({ left: e.deltaY, behavior: 'auto' });
    };
  }
  restoreFocus();
  startPreview();
  const gs = document.getElementById('gamesscroll');
  if (gs) { let t = 0; gs.onscroll = () => { cancelAnimationFrame(t); t = requestAnimationFrame(() => markLetter(currentLetter())); }; markLetter(currentLetter()); }
  const q = document.getElementById('q');
  if (q) { q.oninput = () => { S.query = q.value; S.focusKey = 'q'; renderSearchOnly(); }; }
  renderToast();
}

// Game page video: fades in over the cover after a moment, like a TV menu.
let previewTimer = null;
function startPreview() {
  clearTimeout(previewTimer);
  const v = document.getElementById('preview');
  if (!v || S.playing || S.panel) return;
  previewTimer = setTimeout(() => {
    if (!document.body.contains(v)) return;
    liveMedia.add(v);
    v.muted = !soundOn();
    v.src = v.dataset.src;
    v.addEventListener('playing', () => {
      v.classList.add('on');
      v.closest('.media')?.classList.add('playing');
      v.closest('.left')?.classList.add('playing');
      startBackdrop(v);
      const b = document.getElementById('soundbtn'); if (b) b.hidden = false;
    }, { once: true });
    v.addEventListener('error', () => v.remove(), { once: true });
    v.play().catch(() => { v.muted = true; v.play().catch(() => {}); });
    syncAudio();
  }, 900);
}
// Ambient backdrop: while a video plays, paint tiny frames into a canvas that
// CSS blurs into a glow behind the page.
let bdTimer = null;
function startBackdrop(v) {
  clearInterval(bdTimer);
  const c = document.getElementById('bdcanvas');
  if (!c || !v) return;
  const ctx = c.getContext('2d');
  c.classList.add('on');
  bdTimer = setInterval(() => {
    if (!document.body.contains(v)) return clearInterval(bdTimer);
    if (!v.paused) { try { ctx.drawImage(v, 0, 0, c.width, c.height); } catch (e) { clearInterval(bdTimer); } }
  }, 125);
}

// Trailers stitched from Steam keep their sound in a separate file: play it alongside.
function syncAudio() {
  const v = document.getElementById('preview');
  const a = document.getElementById('previewAudio');
  if (!v || !a || !document.body.contains(a)) return;
  liveMedia.add(a);
  if (!a.src) a.src = a.dataset.src;
  const on = soundOn() && !v.paused;
  if (!on) { a.pause(); return; }
  a.currentTime = v.currentTime;
  a.play().catch(() => {});
  if (!v.dataset.synced) {
    v.dataset.synced = '1';
    v.addEventListener('timeupdate', () => { if (!a.paused && Math.abs(a.currentTime - v.currentTime) > 0.35) a.currentTime = v.currentTime; });
    v.addEventListener('pause', () => a.pause());
    v.addEventListener('playing', () => { if (soundOn() && document.body.contains(a)) { a.currentTime = v.currentTime; a.play().catch(() => {}); } });
  }
}
// Every video/sound a game page started. Media taken off the screen keeps playing
// unless it's stopped, so anything no longer on screen gets silenced after each render.
const liveMedia = new Set();
function silenceDetached() {
  for (const m of liveMedia) {
    if (document.body.contains(m)) continue;
    try { m.pause(); m.muted = true; m.removeAttribute('src'); m.load(); } catch (e) { /* already gone */ }
    liveMedia.delete(m);
  }
}
document.addEventListener('visibilitychange', () => {
  if (document.hidden) { for (const m of liveMedia) { try { m.pause(); } catch (e) {} } return; }
  const v = document.getElementById('preview');
  if (v && liveMedia.has(v) && !S.playing && !S.panel) v.play().then(syncAudio).catch(() => {});
});
function stopPreview() {
  clearTimeout(previewTimer);
  clearInterval(bdTimer);
  const v = document.getElementById('preview');
  if (v) { v.pause(); v.removeAttribute('src'); v.load(); }
  const a = document.getElementById('previewAudio');
  if (a) { a.pause(); a.removeAttribute('src'); }
}

function renderSearchOnly() {
  const old = document.querySelector('.search-panel');
  if (!old) return render();
  const pos = document.getElementById('q').selectionStart;
  const tmp = document.createElement('div');
  tmp.innerHTML = searchPanel();
  old.querySelector('.res').replaceWith(tmp.querySelector('.res'));
  const q = document.getElementById('q');
  q.focus(); q.setSelectionRange(pos, pos);
}

function scope() {
  return document.querySelector('.overlay') || document.querySelector('.search-panel') || document.querySelector('.panel .sheet') || document.getElementById('app');
}
function navEls() {
  return [...scope().querySelectorAll('[data-nav]')].filter((el) => el.offsetParent !== null || el.closest('.overlay'));
}
function focusEl(el, smooth = true, scroll = true) {
  if (!el) return;
  document.querySelectorAll('.focused').forEach((x) => x.classList.remove('focused'));
  el.classList.add('focused');
  el.focus({ preventScroll: true });
  if (scroll) el.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: smooth ? 'smooth' : 'auto' });
  S.focusKey = el.dataset.key;
  if (el.dataset.focus === 'selcon') {
    const i = +el.dataset.i;
    if (i !== S.sel) { S.sel = i; S.focusKey = el.dataset.key; render(); }
  }
}
function restoreFocus() {
  const els = navEls();
  let el = S.focusKey && els.find((e) => e.dataset.key === S.focusKey);
  if (!el) {
    const pref = S.panel === 'scrapers' ? 'acc-sgdb' : S.panel === 'emulator' ? `emu-${S.emu?.current || 'default'}` : { home: `con-${tiles()[S.sel]?.id}`, games: null, detail: 'play', welcome: 'pick' }[S.screen];
    el = (pref && els.find((e) => e.dataset.key === pref)) || els.find((e) => e.dataset.key?.startsWith('g-')) || els.find((e) => e.dataset.key !== 'brand') || els[0];
  }
  if (el) focusEl(el, false);
}

// Spatial navigation: move to the nearest element in a direction.
function move(dir) {
  const els = navEls();
  const cur = document.activeElement?.hasAttribute?.('data-nav') ? document.activeElement : null;
  if (!cur) return restoreFocus();
  if (cur.tagName === 'INPUT' && (dir === 'left' || dir === 'right')) return;
  const r = cur.getBoundingClientRect();
  const cx = r.left + r.width / 2, cy = r.top + r.height / 2;
  let best = null, bestScore = Infinity;
  for (const el of els) {
    if (el === cur) continue;
    const b = el.getBoundingClientRect();
    const x = b.left + b.width / 2, y = b.top + b.height / 2;
    const dx = x - cx, dy = y - cy;
    let primary, cross;
    if (dir === 'left') { if (b.right > r.left + 4) continue; primary = r.left - b.right; cross = Math.abs(dy); }
    if (dir === 'right') { if (b.left < r.right - 4) continue; primary = b.left - r.right; cross = Math.abs(dy); }
    if (dir === 'up') { if (b.bottom > r.top + 4) continue; primary = r.top - b.bottom; cross = Math.abs(dx); }
    if (dir === 'down') { if (b.top < r.bottom - 4) continue; primary = b.top - r.bottom; cross = Math.abs(dx); }
    // Prefer things in the same row/column.
    const overlap = dir === 'left' || dir === 'right' ? Math.min(r.bottom, b.bottom) - Math.max(r.top, b.top) : Math.min(r.right, b.right) - Math.max(r.left, b.left);
    const score = Math.max(primary, 0) + cross * (overlap > 0 ? 0.3 : 2.5);
    if (score < bestScore) { bestScore = score; best = el; }
  }
  if (best) { focusEl(best); SFX.play('move'); } else SFX.play('edge');
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
async function setSetting(k, v) {
  S.boot.settings = { ...settings(), [k]: v };
  render();
  await api.call('set_setting', { key: k, value: v });
}

const ACT = {
  async pick() {
    const dir = await api.call('pick_folder');
    if (!dir) return;
    S.panel = null;
    await doScan(dir);
  },
  home() { S.panel = null; S.screen = 'home'; S.focusKey = null; render(); },
  search(d) { if (!d || !d.keep) S.query = ''; S.panel = 'search'; S.focusKey = 'q'; render(); },
  settings() { S.panel = S.panel === 'settings' ? null : 'settings'; S.focusKey = S.panel ? (S.accounts && S.focusKey?.startsWith('acc') ? 'scrapers' : null) : 'gear'; render(); },
  closepanel() { const was = S.panel; S.panel = null; S.focusKey = was === 'search' ? 'searchbtn' : was === 'emulator' ? 'chemu' : 'gear'; render(); },
  tile(d) {
    const t = tiles()[+d.i];
    if (!t) return;
    S.sel = +d.i;
    if (t.kind === 'random') return ACT.random({ scope: 'all' });
    S.cat = null;
    S.con = browsable().findIndex((b) => b.id === t.id);
    S.filter = 'all'; S.screen = 'games'; S.focusKey = null;
    render();
    const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0;
    restoreFocus();
  },
  noop() {},
  category(d) {
    S.catReturn = { screen: S.screen, game: S.game, from: S.from, focusKey: document.activeElement?.dataset?.key };
    S.cat = { type: d.type, value: d.v };
    S.panel = null; S.filter = 'all'; S.screen = 'games'; S.focusKey = null;
    render();
    const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0;
    restoreFocus();
  },
  random(d) {
    let pool;
    const scope = d.scope === 'list' ? (curList()?.tile.id || 'all') : d.scope || 'all';
    if (scope === 'category') pool = curList().items.map((x) => x.g);
    else if (scope === 'all') pool = systems().flatMap((s) => s.games);
    else if (scope === 'favorites') pool = favItems().map((x) => x.g);
    else pool = systems().find((s) => s.id === scope)?.games || [];
    pool = pool.filter((g) => g.id !== S.spun);
    if (!pool.length) return toast('Nothing to pick from here yet.');
    const g = pool[Math.floor(Math.random() * pool.length)];
    if (S.screen !== 'detail') S.from = S.screen;
    S.spun = g.id; S.spinScope = scope; S.game = g.id; S.panel = null; S.screen = 'detail'; S.focusKey = 'play';
    render();
  },
  async emu(d) {
    const r = await api.call('launch_options', { id: d.id });
    S.emu = { id: d.id, ...r };
    S.panel = 'emulator'; S.focusKey = null;
    render();
  },
  spfix(d) { S.spFix = d.id; S.panel = 'special'; S.focusKey = 'sq-0'; render(); },
  async setspecial(d) {
    const found = findGame(S.spFix);
    const e = (found?.g.episodes || []).filter((x) => x.number == null)[Number(d.i)];
    if (!e) return;
    const q = document.getElementById('sq-' + d.i)?.value || '';
    S.boot = await api.call('set_special_query', { id: S.spFix, file: e.file, query: q });
    S.panel = null; S.focusKey = 'spfix';
    render();
    toast('Searching TMDB…', 4000);
    api.call('fetch_media');
  },
  arcpick(d) { S.arcPick = { id: d.id, group: d.g }; S.panel = 'arc'; S.focusKey = null; render(); },
  async setarc(d) {
    const { id, group } = S.arcPick;
    S.boot = await api.call('set_arc_season', { id, group, season: d.s ? Number(d.s) : null, start: d.s ? Number(d.st) : null });
    S.epGroup = S.epGroup || {}; S.epGroup[id] = group;
    S.panel = null; S.focusKey = 'arcpick';
    render();
    if (d.s) { toast('Matched. Getting episode details…', 4000); api.call('fetch_media'); } else toast('Match removed');
  },
  async setemu(d) {
    S.boot = await api.call('set_launcher', { id: S.emu.id, key: d.k || null });
    S.panel = null; S.focusKey = 'chemu';
    render();
    toast(d.k ? 'This game will use ' + (S.emu.options.find((o) => o.key === d.k)?.label || d.k) : 'Back to the console default');
  },
  async mycover(d) {
    try {
      const boot = await api.call('pick_cover', { id: d.id });
      if (boot) { S.boot = boot; S.focusKey = 'mycover'; render(); toast('Cover updated'); }
    } catch (e) { toast(String(e), 5000); }
  },
  async myvideo(d) {
    try {
      const boot = await api.call('pick_video', { id: d.id });
      if (boot) { S.boot = boot; S.focusKey = 'myvideo'; render(); toast('Video updated'); }
    } catch (e) { toast(String(e), 5000); }
  },
  async resetvideo(d) { S.boot = await api.call('reset_video', { id: d.id }); S.focusKey = 'myvideo'; render(); toast('Back to the downloaded video'); },
  uiscale(d) { S.focusKey = `sz-${d.v}`; setSetting('ui_scale', d.v); },
  async stores(d) {
    S.focusKey = `st-${d.v}`;
    if ((d.v === 'on') === storesOn()) return;
    await setSetting('storeGames', d.v === 'on');
    try {
      const keep = S.boot.settings;
      S.boot = { ...(await api.call('scan', { root: S.boot.rom_root })), settings: keep };
      const pc = S.boot.systems.find((x) => x.id === 'pc');
      toast(d.v === 'on' ? `PC Games now has ${pc ? pc.games.length : 0} games` : 'Store games hidden');
      render();
      if (d.v === 'on') api.call('fetch_media');
    } catch (e) { toast(String(e), 6000); }
  },
  async biosdir(d) { try { await api.call('open_bios_folder', { system: d.s }); } catch (e) { toast(String(e), 5000); } },
  fixgame(d) {
    const f = findGame(d.id);
    if (!f) return;
    S.edit = { id: d.id, query: f.g.title, choices: null, sgdb: !!S.accounts?.steamgriddb_key };
    S.panel = 'edit'; S.focusKey = 'ed-name';
    stopPreview();
    render();
    if (!S.accounts) api.call('get_accounts').then((a) => { S.accounts = a; if (S.edit) S.edit.sgdb = !!a.steamgriddb_key; }).catch(() => {});
  },
  async savename() {
    const v = document.getElementById('ed-name')?.value ?? '';
    try { S.boot = await api.call('rename_game', { id: S.edit.id, title: v.trim() || null }); } catch (e) { return toast(String(e), 4000); }
    S.edit.query = v.trim() || S.edit.query; S.focusKey = 'ed-q';
    render(); toast('Name saved');
  },
  async resetname() {
    S.boot = await api.call('rename_game', { id: S.edit.id, title: null });
    S.focusKey = 'ed-name'; render(); toast('Back to the name from the file');
  },
  async findcovers() {
    const q = (document.getElementById('ed-q')?.value ?? S.edit.query).trim();
    if (!q) return;
    const id = S.edit.id;
    Object.assign(S.edit, { query: q, loading: true, choices: [] });
    S.focusKey = 'ed-find'; render();
    let choices = [];
    try { choices = await api.call('cover_choices', { id, query: q }); } catch (e) { toast(String(e), 4000); }
    if (!S.edit || S.edit.id !== id) return;
    Object.assign(S.edit, { loading: false, choices });
    S.focusKey = choices.length ? 'pk-0' : 'ed-q'; render();
  },
  async usecover(d) {
    const c = S.edit.choices[+d.i];
    if (!c) return;
    toast('Saving cover…', 8000);
    try { S.boot = await api.call('use_cover', { id: S.edit.id, url: c.url }); } catch (e) { return toast(String(e), 5000); }
    S.panel = null; S.focusKey = 'fixgame'; render(); toast('Cover changed');
  },
  async hidegame() {
    const id = S.edit.id;
    S.boot = await api.call('set_hidden', { id, hidden: true });
    S.panel = null; S.edit = null;
    S.screen = S.from === 'home' ? 'home' : 'games'; S.focusKey = null;
    render(); toast('Hidden. Bring it back from Settings → Hidden games.', 4000);
  },
  async unhide(d) {
    S.boot = await api.call('set_hidden', { id: d.id, hidden: false });
    S.focusKey = 'addroot'; render(); toast('Back in your library');
  },
  autoscan() { S.focusKey = 'as-tog'; setSetting('autoScan', settings().autoScan === false); },
  sortby() {
    const order = ['title', 'played', 'recent', 'year', 'added'];
    const next = order[(order.indexOf(sortMode()) + 1) % order.length];
    S.focusKey = 'sortby';
    document.getElementById('gamesscroll')?.scrollTo(0, 0);
    setSetting('sort', next);
    toast(`Sorted: ${sortLabel(curList().tile, next)}`, 1500);
  },
  async dellist(d) {
    S.boot = await api.call('delete_collection', { name: d.n });
    S.screen = 'home'; S.sel = 0; S.focusKey = null; render();
    toast(`Deleted the list “${d.n}” (the games are still in your library)`, 4000);
  },
  lists(d) {
    S.lists = { id: d.id };
    S.panel = 'lists'; S.focusKey = null;
    stopPreview(); render();
  },
  async togglelist(d) {
    const name = d.n, id = S.lists.id;
    const on = ((S.boot.collections || {})[name] || []).includes(id);
    S.boot = await api.call('set_collection', { name, id, member: !on });
    S.focusKey = document.activeElement?.dataset?.key || null; render();
  },
  async newlist() {
    const name = (document.getElementById('nl-name')?.value || '').trim();
    if (!name) return;
    try { S.boot = await api.call('set_collection', { name, id: S.lists.id, member: true }); } catch (e) { return toast(String(e), 4000); }
    S.focusKey = 'nl-name'; render(); toast(`Added to “${name}”`);
  },
  async openurl(d) { try { await api.call('open_url', { url: d.u }); } catch (e) { toast(String(e), 4000); } },
  achinfo(d) { const a = S.ach[S.game]?.achievements?.[+d.i]; if (a) toast(`${a.title}: ${a.description}${a.earned ? ` · earned ${a.date.slice(0, 10)}` : ''}`, 4000); },
  async checkupdate() { S.update = { busy: 'Checking…' }; S.focusKey = 'upd-check'; render(); await checkUpdate(false); },
  async installupdate() {
    S.update = { ...S.update, busy: `Downloading Spine ${S.update.latest}… Spine restarts by itself when it's done.` };
    S.focusKey = 'upd-check'; render();
    try { await api.call('install_update'); } catch (e) { S.update = { ...S.update, busy: null, error: String(e) }; render(); }
  },
  autoupdate() { S.focusKey = 'upd-auto'; setSetting('autoUpdate', settings().autoUpdate === false); },
  openshot(d) { S.shot = +d.i; S.shotFrom = `shot-${d.i}`; stopPreview(); S.focusKey = 'v-next'; render(); },
  stepshot(d) {
    const n = findGame(S.game)?.g?.shots?.length || 1;
    S.shot = (S.shot + +d.d + n) % n; S.focusKey = +d.d < 0 ? 'v-prev' : 'v-next'; render();
  },
  closeshot() { S.focusKey = `shot-${S.shot}`; S.shot = null; render(); },
  async uisounds(d) { S.focusKey = `sfx-${d.v}`; await setSetting('uiSounds', d.v); SFX.play('select'); },
  exitcombo(d) { S.focusKey = `ex-${d.v}`; setSetting('exitCombo', d.v === 'on'); },
  videos(d) { S.focusKey = `vid-${d.v}`; setSetting('videos', d.v === 'on'); },
  sound() {
    const on = !soundOn();
    const v = document.getElementById('preview');
    if (v) v.muted = !on;
    S.boot.settings = { ...settings(), video_sound: on };
    syncAudio();
    api.call('set_setting', { key: 'video_sound', value: on });
    const b = document.getElementById('soundbtn');
    if (b) { b.innerHTML = on ? ICON.soundOn : ICON.soundOff; b.setAttribute('aria-label', on ? 'Mute video' : 'Turn on video sound'); }
    if (S.panel === 'settings') { S.focusKey = 'vid-sound'; render(); }
  },
  async resetcover(d) { S.boot = await api.call('reset_cover', { id: d.id }); S.focusKey = 'mycover'; render(); toast('Back to the downloaded cover'); },
  async scrapers() {
    S.panel = 'scrapers'; S.accounts = null; S.focusKey = null; render();
    S.accounts = await api.call('get_accounts'); render();
  },
  async clearacc(d) { S.accounts = await api.call('set_accounts', { changes: { [d.f]: '' } }); S.focusKey = 'acc-sgdb'; render(); },
  async saveacc() {
    const v = (k) => document.getElementById('acc-' + k)?.value;
    const changes = {};
    const sg = v('sgdb'); if (sg) changes.steamgriddb_key = sg;
    const u = v('ssuser'); if (u !== undefined && u !== (S.accounts.screenscraper_user || '')) changes.screenscraper_user = u;
    const pw = v('sspass'); if (pw) changes.screenscraper_password = pw;
    const di = v('ssdev'); if (di !== undefined && di !== (S.accounts.screenscraper_devid || '')) changes.screenscraper_devid = di;
    const dp = v('ssdevpw'); if (dp) changes.screenscraper_devpassword = dp;
    const ru = v('rauser'); if (ru !== undefined && ru !== (S.accounts.retroachievements_user || '')) changes.retroachievements_user = ru;
    const rk = v('rakey'); if (rk) changes.retroachievements_key = rk;
    const tk = v('tmdb'); if (tk) changes.tmdb_key = tk;
    if (ru || rk) S.ach = {};
    try {
      S.accounts = await api.call('set_accounts', { changes });
      S.panel = null; S.focusKey = 'gear'; render();
      toast('Saved. Looking for missing covers…');
      api.call('fetch_media');
    } catch (e) { toast(String(e), 5000); }
  },
  open(d) {
    if ((d.from || '').startsWith('detail-')) { S.history = S.history || []; S.history.push(S.from); }
    else S.history = [];
    S.from = d.from || S.screen; S.spun = null; S.game = d.id;
    const ds = document.getElementById('detailscroll'); if (ds) ds.scrollTop = 0; S.panel = null; S.screen = 'detail'; S.focusKey = 'play'; render(); },
  back() {
    if (S.panel) return ACT.closepanel();
    if (S.screen === 'detail') {
      const id = S.game;
      S.spun = null;
      if (S.from?.startsWith('detail-')) { const back = S.from.slice(7); S.history = S.history || []; const prev = S.history.pop(); S.game = back; S.from = prev || 'games'; S.focusKey = null; document.getElementById('detailscroll')?.scrollTo(0, 0); return render(); }
      if (S.from === 'home' || S.from === 'search' || S.from === 'welcome') { S.screen = 'home'; S.focusKey = null; }
      else { S.screen = 'games'; S.focusKey = `g-${id}`; }
      return render();
    }
    if (S.screen === 'games' && S.cat) {
      const r = S.catReturn || { screen: 'home' };
      S.cat = null; S.screen = r.screen; S.game = r.game; S.from = r.from; S.focusKey = r.focusKey;
      return render();
    }
    if (S.screen === 'games') {
      const id = curList()?.tile.id;
      S.screen = 'home'; S.sel = Math.max(0, tiles().findIndex((t) => t.id === id)); S.focusKey = `con-${id}`;
      return render();
    }
  },
  filter(d) { S.filter = d.f; S.focusKey = `f-${d.f}`; render(); },
  layout(d) {
    const s = curList().tile;
    S.focusKey = `lay-${d.l}`;
    setSetting('layouts', { ...(settings().layouts || {}), [s.id]: d.l });
  },
  jump(d) {
    // Letters with no games jump to the next letter that has some (or the last one).
    const order = RAIL.indexOf(d.l);
    const has = (c) => document.getElementById(`L-${c === '#' ? 'num' : c}`);
    let c = RAIL.slice(order).find(has) || [...RAIL.slice(0, order)].reverse().find(has);
    if (!c) return;
    const el = has(c);
    const box = document.getElementById('gamesscroll');
    // With Interface size above 100%, screen positions come zoomed but scrollTop
    // doesn't (newer WebView2/Chromium): convert back to the list's own pixels.
    const br = box.getBoundingClientRect();
    const k = box.offsetHeight ? br.height / box.offsetHeight : 1;
    box.scrollTo({ top: (el.getBoundingClientRect().top - br.top) / k + box.scrollTop - 10, behavior: 'smooth' });
    const btn = el.matches('[data-nav]') ? el : el.querySelector('[data-nav]');
    if (btn) focusEl(btn, false, false);
    markLetter(c);
  },
  async fav(d) {
    const id = d.id || S.game;
    const on = await api.call('toggle_favorite', { id });
    SFX.play(on ? 'favOn' : 'favOff');
    const f = new Set(S.boot.favorites || []);
    on ? f.add(id) : f.delete(id);
    S.boot.favorites = [...f];
    render();
  },
  async play(d) {
    const found = findGame(d.id);
    if (!found) return;
    stopPreview();
    const ep = d.ep != null && d.ep !== '' ? Number(d.ep) : null;
    const e = ep != null ? (found.g.episodes || [])[ep] : null;
    S.playing = { title: e ? `${found.g.title} · ${[epCode(e), e.title].filter(Boolean).join(' ')}` : found.g.title, emulator: found.s.emulator, opener: opener(found.g), movie: isMovies(found.s) };
    render();
    try {
      const emu = await api.call('play', { id: d.id, episode: ep, fromStart: d.from === 'start' });
      S.playing = { ...S.playing, emulator: emu, started: true };
      S.boot.last_played = { ...(S.boot.last_played || {}), [d.id]: Math.floor(Date.now() / 1000) };
    } catch (e) {
      S.playing = { ...S.playing, error: String(e) };
    }
    S.focusKey = 'closeplay';
    render();
  },
  closeplay() { S.playing = null; S.focusKey = 'play'; render(); },
  theme(d) { S.focusKey = `th-${d.t}`; setSetting('theme', d.t); },
  accent(d) { S.focusKey = `acc-${d.c}`; setSetting('accent', d.c); },
  async rescan() { S.panel = null; await doScan(S.boot.rom_root); },
  async addroot() {
    try {
      const b = await api.call('add_root');
      if (!b) return;
      const before = allItems().length;
      S.boot = b; S.focusKey = 'addroot';
      render();
      const n = allItems().length - before;
      toast(n > 0 ? `Added ${n} game${n === 1 ? '' : 's'}` : 'Folder added');
      api.call('fetch_media');
    } catch (e) { toast(String(e), 5000); }
  },
  async rmroot(d) {
    S.boot = await api.call('remove_root', { path: d.p });
    S.focusKey = 'addroot';
    if (!S.boot.roots.length && !(S.boot.movie_roots || []).length && !(S.boot.tv_roots || []).length) { S.panel = null; S.screen = 'welcome'; }
    render();
    toast('Folder removed from Spine (nothing was deleted)');
  },
  async addmovieroot() {
    try {
      const b = await api.call('add_movie_root');
      if (!b) return;
      const count = (x) => (x.systems.find((s) => s.id === 'movies')?.games.length || 0);
      const n = count(b) - count(S.boot);
      S.boot = b; S.focusKey = 'addmovieroot';
      render();
      toast(n > 0 ? `Found ${n} movie${n === 1 ? '' : 's'}` : 'Folder added, but no movies found in it yet', 4000);
      api.call('fetch_media');
    } catch (e) { toast(String(e), 5000); }
  },
  async rmmovieroot(d) {
    S.boot = await api.call('remove_movie_root', { path: d.p });
    S.focusKey = 'addmovieroot';
    render();
    toast('Folder removed from Spine (nothing was deleted)');
  },
  async watched(d) {
    const on = d.on !== '1';
    const p = await api.call('set_watched', { key: d.k, watched: on });
    const all = { ...(S.boot.progress || {}) };
    if (p) all[d.k] = p; else delete all[d.k];
    S.boot.progress = all;
    SFX.play(on ? 'favOn' : 'favOff');
    S.focusKey = d.key;
    render();
  },
  epgroup(d) { S.epGroup = S.epGroup || {}; S.epGroup[d.id] = d.g; S.focusKey = `grp-${d.g}`; render(); },
  async addtvroot() {
    try {
      const b = await api.call('add_tv_root');
      if (!b) return;
      const count = (x) => (x.systems.find((s) => s.id === 'tv')?.games.length || 0);
      const n = count(b) - count(S.boot);
      S.boot = b; S.focusKey = 'addtvroot';
      render();
      toast(n > 0 ? `Found ${n} show${n === 1 ? '' : 's'}` : 'Folder added, but no shows found in it yet', 4000);
      api.call('fetch_media');
    } catch (e) { toast(String(e), 5000); }
  },
  async rmtvroot(d) {
    S.boot = await api.call('remove_tv_root', { path: d.p });
    S.focusKey = 'addtvroot';
    render();
    toast('Folder removed from Spine (nothing was deleted)');
  },
  fetch() { api.call('fetch_media'); },
  creds() { api.call('open_path', { which: 'credentials' }); },
  fullscreen() { const on = !settings().fullscreen; api.call('set_fullscreen', { on }); S.focusKey = 'fs'; setSetting('fullscreen', on); },
  quit() { api.call('quit'); },
};

async function doScan(dir) {
  S.boot = { ...(S.boot || {}), rom_root: dir };
  S.screen = 'scanning';
  render();
  try {
    S.boot = await api.call('scan', { root: dir });
    S.screen = 'home'; S.sel = tiles()[0]?.kind === 'fav' ? 1 : 0; S.focusKey = null;
  } catch (e) {
    S.screen = 'welcome';
    toast(String(e), 6000);
  }
  render();
  api.call('fetch_media');
}

// Clicks / taps
document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-act]');
  if (!el) return;
  if (el.classList.contains('panel') && e.target !== el) return;
  SFX.play(actSound(el.dataset.act));
  ACT[el.dataset.act]?.(el.dataset);
});
document.addEventListener('mousedown', (e) => { if (e.target.closest('.rail, .cbar')) e.preventDefault(); });
document.addEventListener('mousemove', () => document.body.classList.remove('kb'), { passive: true });

// ---------------------------------------------------------------------------
// Interface sounds: tiny tones made on the fly (no sound files), so moving
// around Spine feels like a console menu. Settings → Sounds turns them down or off.
// ---------------------------------------------------------------------------
const SFX = (() => {
  let ctx = null, master = null, last = 0;
  const level = () => ({ off: 0, soft: 0.45, normal: 1 })[settings().uiSounds || 'soft'] ?? 0.45;
  function out() {
    if (!ctx) {
      const AC = window.AudioContext || window.webkitAudioContext;
      if (!AC) return null;
      ctx = new AC();
      master = ctx.createGain();
      const lp = ctx.createBiquadFilter(); lp.type = 'lowpass'; lp.frequency.value = 5200; // keep them soft, never shrill
      master.connect(lp); lp.connect(ctx.destination);
    }
    if (ctx.state === 'suspended') ctx.resume().catch(() => {});
    master.gain.value = level();
    return ctx;
  }
  // One note: frequency (optionally gliding to f2), length, loudness, wave, start delay.
  function note(f, { f2 = f, len = 0.06, vol = 0.05, wave = 'sine', at = 0 } = {}) {
    const c = out(); if (!c) return;
    const t = c.currentTime + at;
    const o = c.createOscillator(), g = c.createGain();
    o.type = wave;
    o.frequency.setValueAtTime(f, t);
    if (f2 !== f) o.frequency.exponentialRampToValueAtTime(f2, t + len * 0.8);
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(vol, t + 0.004);
    g.gain.exponentialRampToValueAtTime(0.0001, t + len);
    o.connect(g); g.connect(master);
    o.start(t); o.stop(t + len + 0.02);
  }
  const sounds = {
    move: () => note(1180, { len: 0.035, vol: 0.035 }),
    edge: () => note(240, { f2: 200, len: 0.05, vol: 0.03, wave: 'triangle' }),
    tick: () => note(820, { len: 0.03, vol: 0.03, wave: 'triangle' }),
    select: () => { note(780, { len: 0.05, vol: 0.045 }); note(1170, { len: 0.07, vol: 0.04, at: 0.035 }); },
    back: () => { note(900, { len: 0.05, vol: 0.04 }); note(600, { len: 0.07, vol: 0.035, at: 0.035 }); },
    open: () => note(520, { f2: 900, len: 0.11, vol: 0.04 }),
    favOn: () => { note(988, { len: 0.06, vol: 0.04 }); note(1319, { len: 0.1, vol: 0.04, at: 0.06 }); },
    favOff: () => { note(1319, { len: 0.05, vol: 0.035 }); note(988, { len: 0.08, vol: 0.03, at: 0.05 }); },
    spin: () => [660, 740, 830, 880, 990, 1110].forEach((f, i) => note(f, { len: 0.035, vol: 0.03, wave: 'triangle', at: i * 0.045 })),
    launch: () => [523, 659, 784, 1047].forEach((f, i) => note(f, { len: i === 3 ? 0.35 : 0.09, vol: 0.045, at: i * 0.06 })),
    welcome: () => [784, 1047].forEach((f, i) => note(f, { len: 0.12, vol: 0.035, at: i * 0.08 })),
  };
  return {
    play(name) {
      if (!level() || S.playing && name !== 'welcome' || document.hidden) return;
      const now = performance.now();
      if (name === 'move' && now - last < 30) return; // fast scrolling: don't machine-gun
      last = now;
      try { sounds[name]?.(); } catch (e) { /* no audio device */ }
    },
  };
})();
// Which sound an action makes.
function actSound(act) {
  if (!act) return null;
  if (['back', 'closepanel', 'closeshot', 'closeplay', 'scback'].includes(act)) return 'back';
  if (['settings', 'scrapers', 'search', 'emu', 'fixgame', 'lists', 'openshot'].includes(act)) return 'open';
  if (act === 'play') return 'launch';
  if (act === 'random') return 'spin';
  if (act === 'fav' || act === 'uisounds') return null; // they play their own once the result is known
  if (['jump', 'stepshot', 'sortby', 'filter', 'layout', 'sound', 'videos'].includes(act)) return 'tick';
  return 'select';
}

// ---------------------------------------------------------------------------
// Keyboard & controller
// ---------------------------------------------------------------------------
function press(btn) {
  document.body.classList.add('kb');
  const cur = document.activeElement;
  const typing = cur && cur.tagName === 'INPUT';
  if (btn === 'A' && cur?.dataset?.act) SFX.play(actSound(cur.dataset.act));
  else if (btn === 'B' && (S.panel || S.screen !== 'home' || S.shot != null)) SFX.play('back');
  else if (btn === 'START' || btn === 'X') SFX.play('open');
  else if (btn === 'RANDOM') SFX.play('spin');
  else if (['LB', 'RB', 'LT', 'RT', 'SELECT'].includes(btn)) SFX.play('tick');
  if (S.shot != null && S.screen === 'detail') {
    if (btn === 'left' || btn === 'LB') return ACT.stepshot({ d: -1 });
    if (btn === 'right' || btn === 'RB') return ACT.stepshot({ d: 1 });
    if (btn === 'B' || btn === 'A') return ACT.closeshot();
    return;
  }
  switch (btn) {
    case 'up': case 'down': case 'left': case 'right': return move(btn);
    case 'A': if (cur?.dataset?.act) ACT[cur.dataset.act]?.(cur.dataset); else if (typing) move('down'); return;
    case 'B':
      if (S.playing && (S.playing.error || S.playing.emulator === 'system')) return ACT.closeplay();
      if (S.panel === 'scrapers') return ACT.settings();
      return ACT.back();
    case 'X': if (!S.panel && S.boot?.systems?.length && !S.playing) ACT.search(); return;
    case 'Y': {
      if (typing) return;
      const id = cur?.dataset?.id || (S.screen === 'detail' ? S.game : null);
      if (id) ACT.fav({ id });
      return;
    }
    case 'START': if (S.boot?.rom_root && !S.playing) ACT.settings(); return;
    case 'RANDOM':
      if (S.panel || S.playing || !systems().length) return;
      return ACT.random({ scope: S.screen === 'games' ? 'list' : S.screen === 'detail' && S.spun ? S.spinScope : 'all' });
    case 'SELECT': {
      if (S.screen !== 'games' || S.panel) return;
      const order = ['tiles', 'list', 'cards', 'shelf'];
      const s = curList().tile;
      const next = order[(order.indexOf(layoutFor(s.id)) + 1) % order.length];
      return ACT.layout({ l: next });
    }
    case 'LT': case 'RT':
      if (S.screen === 'games' && !S.panel && !S.playing) stepLetter(btn === 'RT' ? 1 : -1);
      return;
    case 'LB': case 'RB': {
      if (!systems().length || S.panel || S.playing) return;
      if (S.screen === 'games' && S.cat) return;
      if (S.screen === 'games') {
        const n = browsable().length;
        S.con = (S.con + (btn === 'RB' ? 1 : n - 1)) % n; S.filter = 'all'; S.focusKey = null; render();
        const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0; restoreFocus();
      } else if (S.screen === 'home') {
        const T = tiles(), n = T.length;
        S.sel = (S.sel + (btn === 'RB' ? 1 : n - 1)) % n; S.focusKey = `con-${T[S.sel].id}`; render();
      }
    }
  }
}

document.addEventListener('keydown', (e) => {
  const typing = e.target.tagName === 'INPUT';
  const map = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right', Enter: 'A', Escape: 'B', PageUp: 'LB', PageDown: 'RB', F2: 'START' };
  if (!typing) Object.assign(map, { ' ': 'A', Backspace: 'B', '/': 'X', f: 'Y', F: 'Y', l: 'SELECT', L: 'SELECT', q: 'LB', e: 'RB', '[': 'LT', ']': 'RT', Home: 'LT', End: 'RT', r: 'RANDOM', R: 'RANDOM' });
  const b = map[e.key];
  if (!b) {
    // Typing anywhere on home/games starts a search.
    if (!typing && !S.panel && e.key.length === 1 && /\w/.test(e.key) && S.boot?.systems?.length) { S.query = e.key; ACT.search({ keep: true }); e.preventDefault(); }
    return;
  }
  if (typing && (b === 'left' || b === 'right')) return;
  e.preventDefault();
  press(b);
});

// Standard gamepad mapping (Xbox layout names; PlayStation/Switch map the same positions).
const PAD = { 0: 'A', 1: 'B', 2: 'X', 3: 'Y', 4: 'LB', 5: 'RB', 6: 'LT', 7: 'RT', 8: 'SELECT', 9: 'START', 10: 'RANDOM', 11: 'RANDOM', 12: 'up', 13: 'down', 14: 'left', 15: 'right' };
const held = {};
let padMute = false;
function pollPads(t) {
  const pads = navigator.getGamepads ? [...navigator.getGamepads()].filter(Boolean) : [];
  const now = performance.now();
  const down = new Set();
  for (const p of pads) {
    p.buttons.forEach((b, i) => { if (b.pressed && PAD[i]) down.add(PAD[i]); });
    const [ax, ay] = [p.axes[0] || 0, p.axes[1] || 0];
    if (ax < -0.55) down.add('left'); if (ax > 0.55) down.add('right');
    if (ay < -0.55) down.add('up'); if (ay > 0.55) down.add('down');
  }
  if (padMute) { if (down.size) for (const b of down) held[b] = { next: Infinity }; else padMute = false; }
  if (!padMute && document.hasFocus() && !(S.playing && !S.playing.error && S.playing.emulator !== 'system')) {
    for (const b of down) {
      const h = held[b];
      const repeat = ['up', 'down', 'left', 'right'].includes(b);
      if (!h) { held[b] = { next: now + 380 }; press(b); }
      else if (repeat && now >= h.next) { h.next = now + 85; press(b); }
    }
  }
  for (const b of Object.keys(held)) if (!down.has(b)) delete held[b];
  requestAnimationFrame(pollPads);
}
requestAnimationFrame(pollPads);
window.addEventListener('gamepadconnected', (e) => { document.body.classList.add('kb'); toast(`Controller connected: ${e.gamepad.id.split('(')[0].trim()}`, 2500); });

// ---------------------------------------------------------------------------
// Toasts, clock, events from the backend
// ---------------------------------------------------------------------------
let toastTimer = null;
function toast(msg, ms = 3000) {
  S.toast = msg;
  renderToast();
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { S.toast = null; renderToast(); }, ms);
}
function renderToast() {
  const el = document.getElementById('toast');
  if (S.toast) { el.hidden = false; el.textContent = S.toast; return; }
  if (S.fetch && S.fetch.total) {
    const pct = Math.round((100 * S.fetch.done) / S.fetch.total);
    el.hidden = false;
    el.innerHTML = `<span>${S.fetch.stage === 'tv' ? 'Getting TV shows and episodes' : S.fetch.stage === 'movies' ? 'Getting movie posters and details' : S.fetch.stage === 'details' ? 'Looking up game details' : S.fetch.stage === 'videos' ? 'Finding trailers' : S.fetch.stage === 'art' ? 'Getting banners, logos and screenshots' : S.fetch.stage === 'categories' ? 'Sorting games into categories' : 'Downloading covers'} ${S.fetch.done}/${S.fetch.total}</span><span class="bar"><i style="width:${pct}%"></i></span>`;
    return;
  }
  el.hidden = true;
}
setInterval(() => { const c = document.getElementById('clock'); if (c) c.textContent = new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }); }, 15000);

api.on('fetch-progress', (p) => { S.fetch = p; renderToast(); });
api.on('fetch-error', (msg) => toast(String(msg), 6000));
api.on('library-updated', (boot) => {
  const before = (S.boot?.systems || []).reduce((n, s) => n + s.games.filter((g) => g.cover).length, 0);
  S.boot = boot; S.fetch = null;
  const after = boot.systems.reduce((n, s) => n + s.games.filter((g) => g.cover).length, 0);
  if (!S.playing) render();
  if (after > before) toast(`${after - before} new cover${after - before > 1 ? 's' : ''}`);
  else renderToast();
});
api.on('library-changed', (p) => {
  S.boot = p.boot;
  const msgs = [];
  for (const r of p.reconnected || []) msgs.push(`${r} is connected again`);
  for (const r of p.unplugged || []) msgs.push(`${r} was unplugged`);
  if (p.added) msgs.push(`${p.added} new game${p.added === 1 ? '' : 's'} added`);
  else if (p.removed && !(p.unplugged || []).length) msgs.push(`${p.removed} game${p.removed === 1 ? '' : 's'} removed`);
  if (S.screen === 'detail' && !findGame(S.game)) { S.screen = 'home'; S.focusKey = null; }
  if (!S.playing) render();
  if (msgs.length) toast(msgs.join(' · '), 4000);
});
api.on('game-closing', () => { if (S.playing) { S.playing.closing = true; render(); } });
api.on('game-exited', (p) => {
  if (p.error && S.playing) {
    // The emulator closed right away: keep the overlay up with the reason.
    S.playing = { ...S.playing, started: false, error: p.error };
    S.focusKey = 'closeplay';
    SFX.play('edge');
    return render();
  }
  setTimeout(() => SFX.play('welcome'), 250);
  if (S.ach) delete S.ach[p.id]; // pick up anything earned while playing
  padMute = true; // buttons still held from the exit combo shouldn't press anything here
  if (p.progress) S.boot.progress = { ...(S.boot.progress || {}), [p.key]: p.progress };
  S.boot.playtime = { ...(S.boot.playtime || {}), [p.id]: ((S.boot.playtime || {})[p.id] || 0) + (S.playing?.emulator === 'system' ? 0 : p.seconds) };
  S.playing = null;
  S.focusKey = 'play';
  render();
  if (p.seconds > 60) toast(`${/^(movies|tv)\//.test(String(p.id)) ? 'Watched' : 'Played'} ${fmtTime(p.seconds)}`);
});

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------
(async function start() {
  S.boot = await api.call('boot');
  applyTheme();
  const hasLibrary = S.boot.rom_root || (S.boot.movie_roots || []).length || (S.boot.tv_roots || []).length;
  if (!hasLibrary) S.screen = 'welcome';
  else { S.screen = 'home'; S.sel = Math.max(0, tiles().findIndex((t) => t.kind === 'sys')); }
  render();
  if (settings().fullscreen) api.call('set_fullscreen', { on: true });
  if (hasLibrary) api.call('fetch_media');
  api.call('app_version').then((v) => { S.version = v; }).catch(() => {});
  if (settings().autoUpdate !== false) setTimeout(() => checkUpdate(true), 4000);
})();
