// Streaming addons (Stremio-style): the Streaming tile on Home opens its own screen
// (screen 'stream') with Movies / Shows, a Search tab and each addon catalog, plus a
// details page (screen 'sdetail'). Kept apart from your own Movies and TV on purpose.
// Loaded before app.js.
// The backend never sends an addon's link here (it can hold your keys), only a `key`.

// Which addon catalog types go under Movies and under Shows.
const STREAM_TYPES = { movies: ['movie'], tv: ['series', 'anime'] };
const STREAM_KINDS = [['movies', 'Movies'], ['tv', 'Shows']];
const STREAM_COLOR = '#0E6E77';
const S_STREAM = { kind: null, tab: {}, cache: {}, meta: {}, season: {}, search: {}, genre: {}, filt: {} };

function streamAddons() { return S.boot?.addons || []; }
// Browsable catalogs for the Movies or TV screen, in addon order.
function streamCats(sysId) {
  const types = STREAM_TYPES[sysId];
  if (!types) return [];
  const out = [];
  for (const a of streamAddons()) for (const c of a.catalogs) {
    if (c.needs_input || !types.includes(c.type)) continue;
    out.push({ ck: `${a.key}|${c.type}|${c.id}`, addon: a.key, addonName: a.name, type: c.type, id: c.id, name: c.name, genres: c.genres || [], genreRequired: !!c.genre_required });
  }
  // Two addons with a "Popular" catalog: say which is which.
  const counts = {};
  out.forEach((c) => { counts[c.name] = (counts[c.name] || 0) + 1; });
  out.forEach((c) => { c.label = counts[c.name] > 1 ? `${c.name} · ${c.addonName}` : c.name; });
  return out;
}
// Catalogs to search for the Movies or TV screen: one per addon and type, preferring the
// addon's own search catalog (Stremio addons search through a catalog with a "search" extra).
function streamSearchCats(sysId) {
  const types = STREAM_TYPES[sysId];
  if (!types) return [];
  const out = [];
  for (const a of streamAddons()) for (const t of types) {
    const cs = a.catalogs.filter((c) => c.type === t && c.searchable);
    const c = cs.find((x) => x.needs_input) || cs[0];
    if (c) out.push({ addon: a.key, addonName: a.name, type: c.type, id: c.id });
  }
  return out;
}
// The Home tile: shown once any addon is added.
function streamHomeTile() {
  const n = streamAddons().length;
  if (!n) return null;
  return { kind: 'stream', id: 'streaming', name: 'Streaming', short: 'Streaming', color: STREAM_COLOR, count: null, label: `${n} addon${n === 1 ? '' : 's'}` };
}
// Movies / Shows that have something to browse or search.
function streamKinds() { return STREAM_KINDS.filter(([k]) => streamCats(k).length || streamSearchCats(k).length); }
function streamKind() {
  const ks = streamKinds().map(([k]) => k);
  if (!ks.includes(S_STREAM.kind)) S_STREAM.kind = ks[0] || 'movies';
  return S_STREAM.kind;
}
// ---- Your streaming lists: Favorites, Watch later and your own, kept apart from game lists.
// Stored in spine-state.json `stream_lists` (name -> titles, newest first; no links).
const SL_FAV = 'Favorites', SL_LATER = 'Watch later';
const sLists = () => S.boot?.stream_lists || {};
const sInList = (name, item) => (sLists()[name] || []).some((r) => r.item === item);
const sOwnLists = () => Object.keys(sLists()).filter((n) => n !== SL_FAV && n !== SL_LATER);
// What a list keeps of a title.
const sListInfo = (it, addon, addonName) => ({ item: `${it.type}:${it.id}`, type: it.type, id: it.id, name: it.name, poster: it.poster || null, year: it.year || null, addon: addon || null, addonName: addonName || null });
// Tabs for your lists on Movies or Shows, each shown once it has something of that kind.
function streamMyTabs(kind) {
  const types = STREAM_TYPES[kind];
  const out = [];
  if (streamContinue(kind, 99).length) out.push({ ck: 'cw', mine: true, label: 'Continue watching', addonName: 'What you were watching' });
  for (const n of [SL_FAV, SL_LATER, ...sOwnLists()]) {
    const items = (sLists()[n] || []).filter((r) => types.includes(r.type));
    if (items.length) out.push({ ck: `list:${n}`, mine: true, list: n, own: n !== SL_FAV && n !== SL_LATER, items, label: n, addonName: n === SL_FAV ? 'Your favorites' : n === SL_LATER ? 'Saved for later' : 'Your list' });
  }
  return out;
}
// Tabs for a kind: Search first (when an addon can search), your lists, then every catalog.
function streamTabList(kind) {
  const out = [];
  if (streamSearchCats(kind).length) out.push({ ck: `search:${kind}`, search: true, sys: kind, label: 'Search', addonName: 'Searches your addons' });
  return out.concat(streamMyTabs(kind), streamCats(kind));
}
// The tab shown right now: the one picked last, else the first catalog (else Search).
function streamCur(kind) {
  const list = streamTabList(kind);
  return list.find((c) => c.ck === S_STREAM.tab[kind]) || list.find((c) => !c.search && !c.mine) || list[0] || null;
}

const streamImg = (u, cls = '') => (u ? `<img ${cls ? `class="${cls}" ` : ''}src="${esc(u)}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror="this.remove()">` : '');
// A poster over a title card: if the picture can't load, the title shows instead.
const streamCover = (it, src) => `<div class="cover ph scover" style="background:${PAL[hash(it.name) % PAL.length]}"><span class="pt">${esc(it.name)}</span><span class="ps">${esc(it.year || '')}</span>${streamImg(src)}</div>`;
// Stremio's poster service hands out small posters by default; pages look better with bigger ones.
const bigPoster = (u) => (u || '').replace('/poster/small/', '/poster/medium/');

function streamTabs(kind) {
  const cur = streamCur(kind);
  return `<div class="streamtabs" role="tablist" aria-label="Catalogs">
    ${streamTabList(kind).map((c, i, all) => `${i && all[i - 1].mine && !c.mine ? '<span class="tabsep" aria-hidden="true"></span>' : ''}<button class="chip ${cur?.ck === c.ck ? 'on' : ''} ${c.mine ? 'mine' : ''}" role="tab" aria-selected="${cur?.ck === c.ck}" data-nav data-key="st-${esc(c.ck)}" data-act="stab" data-s="${esc(kind)}" data-ck="${esc(c.ck)}" title="${esc(c.addonName)}">${c.search ? ICON.search + ' ' : c.list === SL_FAV ? ICON.heartA + ' ' : ''}${esc(c.label)}</button>`).join('')}
  </div>`;
}

// The Streaming screen.
function stream() {
  if (!streamAddons().length) { S.screen = 'home'; return home(); }
  const kinds = streamKinds();
  const kind = streamKind();
  const cur = streamCur(kind);
  if (cur && !cur.search && !cur.mine && !S_STREAM.cache[sCacheKey(cur)]) setTimeout(() => streamLoad(cur.ck));
  const body = !kinds.length
    ? `<div class="nofav">Your addons don't have catalogs or search. Add a catalog addon in Settings → Streaming addons, for example Cinemeta:<br><span class="path">https://v3-cinemeta.strem.io/manifest.json</span></div>`
    : cur ? streamGrid(cur) : '';
  return topbar() + `<main class="main">
    <div class="gamesbar">
      <button class="iconbtn" data-nav data-key="sgback" data-act="back" aria-label="Back to Home">${ICON.back}</button>
      <span class="hdricon" style="background:${STREAM_COLOR}">${sysIcon('streaming', 30)}</span><h2>Streaming</h2>
      <span style="color:var(--muted)">${esc(cur ? (cur.search || cur.mine ? cur.addonName : `From ${cur.addonName}`) : '')}</span>
      <div class="spacer"></div>
      ${kinds.length > 1 ? `<div class="seg skinds" role="group" aria-label="Movies or shows">${kinds.map(([k, l]) => `<button class="${k === kind ? 'on' : ''}" data-nav data-key="sk-${k}" data-act="skind" data-k="${k}">${l}</button>`).join('')}</div>` : ''}
    </div>
    ${kinds.length ? streamTabs(kind) : ''}
    <div class="games-wrap"><div class="games scroll" id="gamesscroll">${body}</div></div>
  </main>` + hints('Movies and shows from your streaming addons', [['A', 'Open'], ['B', 'Back'], ['Y', 'Favorite'], ['LB / RB', 'Catalog'], kinds.length > 1 ? ['LT / RT', 'Movies / Shows'] : null].filter(Boolean));
}

// Controller on the Streaming screen: LB/RB step through the tabs, LT/RT switch Movies / Shows.
function streamKey(btn) {
  if (S.screen !== 'stream') return false;
  const kind = streamKind();
  if (btn === 'LB' || btn === 'RB') {
    const list = streamTabList(kind);
    if (list.length < 2) return true;
    const i = Math.max(0, list.findIndex((c) => c.ck === streamCur(kind)?.ck));
    const next = list[(i + (btn === 'RB' ? 1 : list.length - 1)) % list.length];
    STREAM_ACT.stab({ s: kind, ck: next.ck });
    return true;
  }
  if (btn === 'LT' || btn === 'RT') {
    const ks = streamKinds().map(([k]) => k);
    if (ks.length < 2) return true;
    const i = ks.indexOf(kind);
    STREAM_ACT.skind({ k: ks[(i + (btn === 'RT' ? 1 : ks.length - 1)) % ks.length] });
    return true;
  }
  return false;
}

// ---- Filters (0.17). Genre is the addon's own option, so it filters the whole catalog
// (some catalogs, like Cinemeta's "New", list years there instead). Anime, decade and
// minimum rating filter what's loaded; Spine loads more pages until enough match.
const S_DECADES = ['', '2020s', '2010s', '2000s', '1990s', '1980s', 'older'];
const S_RATINGS = [0, 6, 7, 8];
const sYearPicker = (c) => c.genres.length > 0 && c.genres.every((g) => /^\d{4}$/.test(g));
// The genre asked of the addon for a catalog ('' = all).
function sGenre(c) {
  const g = S_STREAM.genre[c.ck];
  if (g && c.genres.includes(g)) return g;
  return c.genreRequired ? c.genres[0] || '' : '';
}
const sCacheKey = (c) => { const g = sGenre(c); return g ? `${c.ck}#${g}` : c.ck; };
function sFilt(kind) { return S_STREAM.filt[kind] || (S_STREAM.filt[kind] = { anime: false, decade: '', rating: 0 }); }
const sFiltOn = (f) => !!(f.anime || f.decade || f.rating);
const sYearOf = (it) => { const m = String(it.year || '').match(/\d{4}/); return m ? +m[0] : null; };
function sIsAnime(it) {
  if (it.type === 'anime' || /^(kitsu|mal|anilist|anidb):/.test(it.id || '')) return true;
  const g = (it.genres || []).map((x) => String(x).toLowerCase());
  if (g.includes('anime')) return true;
  return g.includes('animation') && String(it.country || '').split(',')[0].trim().toLowerCase() === 'japan';
}
function sMatch(it, f) {
  if (f.anime && !sIsAnime(it)) return false;
  if (f.decade) {
    const y = sYearOf(it);
    if (y == null) return false;
    if (f.decade === 'older' ? y >= 1980 : (y < +f.decade.slice(0, 4) || y >= +f.decade.slice(0, 4) + 10)) return false;
  }
  if (f.rating) { const r = parseFloat(it.rating); if (!(r >= f.rating)) return false; }
  return true;
}
const sDecadeLabel = (d) => (!d ? 'Any year' : d === 'older' ? 'Before 1980' : d);
function streamFilterBar(c) {
  const f = sFilt(streamKind()), g = sGenre(c), yp = sYearPicker(c);
  const d = S_STREAM.cache[sCacheKey(c)];
  const shown = d ? d.items.filter((it) => sMatch(it, f)).length : 0;
  const btn = (key, act, label, on, extra = '') => `<button class="chip ${on ? 'on' : ''}" data-nav data-key="${key}" data-act="${act}" ${extra}>${label}</button>`;
  return `<div class="sfilters" role="group" aria-label="Filters">
    ${c.genres.length ? btn('sfgenre', 'sgenre', `${yp ? 'Year' : 'Genre'}: ${esc(g || (yp ? 'All years' : 'All genres'))} ▾`, !!g, `data-ck="${esc(c.ck)}"`) : ''}
    ${yp ? '' : btn('sfdecade', 'sfstep', sDecadeLabel(f.decade), !!f.decade, 'data-f="decade"')}
    ${btn('sfrating', 'sfstep', f.rating ? `★ ${f.rating}+` : 'Any rating', !!f.rating, 'data-f="rating"')}
    ${btn('sfanime', 'sfstep', `${f.anime ? ICON.check + ' ' : ''}Anime only`, f.anime, 'data-f="anime"')}
    ${sFiltOn(f) || (g && !c.genreRequired) ? btn('sfclear', 'sfclear', 'Clear filters', false, `data-ck="${esc(c.ck)}"`) : ''}
    ${sFiltOn(f) && d ? `<span class="note">${shown} of ${d.items.length} loaded</span>` : ''}
  </div>`;
}

function streamGrid(c) {
  if (c.search) return streamSearchView(c);
  if (c.mine) return streamListView(c);
  return streamFilterBar(c) + streamCatalogView(c);
}
function streamCatalogView(c) {
  const d = S_STREAM.cache[sCacheKey(c)];
  const f = sFilt(streamKind()), on = sFiltOn(f);
  if (!d || (d.loading && !d.items.length)) return `<div class="nofav"><div class="spinner" style="margin:0 auto 18px"></div>Loading ${esc(c.label)}…</div>`;
  if (d.error && !d.items.length) return `<div class="nofav">${esc(d.error)}<div style="margin-top:18px"><button class="btn" data-nav data-key="sretry" data-act="smore" data-ck="${esc(c.ck)}">Try again</button></div></div>`;
  if (!d.items.length) return `<div class="nofav">${sGenre(c) ? `Nothing under ${esc(sGenre(c))} in this catalog.` : 'This catalog is empty right now.'}</div>`;
  const list = d.items.map((it, i) => [it, i]).filter(([it]) => !on || sMatch(it, f));
  if (!list.length) {
    return d.done || d.error
      ? `<div class="nofav">${d.error ? esc(d.error) : 'Nothing in this catalog matches your filters.'}<div style="margin-top:18px"><button class="btn" data-nav data-key="sfclear2" data-act="sfclear" data-ck="${esc(c.ck)}">Clear filters</button></div></div>`
      : `<div class="nofav"><div class="spinner" style="margin:0 auto 18px"></div>Looking for matches…</div>`;
  }
  const tiles = list.map(([it, i]) => `<button class="game" data-nav data-key="si-${esc(it.id)}" data-act="sopen" data-ck="${esc(c.ck)}" data-i="${i}">
      <span class="cwimg">${streamCover(it, it.poster)}${sFavDot(`${it.type}:${it.id}`)}</span>
      <span class="t">${esc(it.name)}</span>${it.year ? `<span class="s">${esc(it.year)}</span>` : ''}</button>`).join('');
  const more = d.done ? '' : `<div class="smore">${d.error ? `<span class="note">${esc(d.error)}</span>` : ''}<button class="btn" data-nav data-key="smore" data-act="smore" data-ck="${esc(c.ck)}" ${d.loading ? 'disabled' : ''}>${d.loading ? (on ? 'Looking for more…' : 'Loading…') : 'Show more'}</button></div>`;
  return `<div class="tiles stiles">${tiles}</div>${more}`;
}

// A poster with a heart when it's a favorite.
const sFavDot = (item) => (sInList(SL_FAV, item) ? `<span class="favdot">${ICON.heart}</span>` : '');
// Continue watching, Favorites, Watch later or one of your lists, as a grid.
function streamListView(c) {
  if (c.ck === 'cw') {
    return `<div class="tiles stiles scontinue">${streamContinue(streamKind(), 99).map(({ r, line, pct }) => `<button class="game" data-nav data-key="cw-${esc(r.item)}" data-act="scontinue" data-item="${esc(r.item)}">
      <span class="cwimg">${streamCover({ name: r.name, year: r.year }, bigPoster(r.poster))}${sFavDot(r.item)}${pct ? `<span class="pbar"><i style="width:${pct}%"></i></span>` : ''}</span>
      <span class="t">${esc(r.name)}</span><span class="s">${esc(line)}</span></button>`).join('')}</div>`;
  }
  const tiles = c.items.map((r, i) => `<button class="game" data-nav data-key="si-${esc(r.id)}" data-act="sopen" data-ck="${esc(c.ck)}" data-i="${i}">
      <span class="cwimg">${streamCover(r, r.poster)}${c.list !== SL_FAV ? sFavDot(r.item) : ''}</span>
      <span class="t">${esc(r.name)}</span>${r.year || r.addonName ? `<span class="s">${esc(r.year || '')}${r.year && r.addonName ? ' · ' : ''}${esc(r.addonName || '')}</span>` : ''}</button>`).join('');
  const del = c.own ? `<div class="smore"><button class="btn" data-nav data-key="sldel" data-act="sldelete" data-n="${esc(c.list)}">Delete the list “${esc(c.list)}”</button></div>` : '';
  return `<div class="tiles stiles">${tiles}</div>${del}`;
}

// ---- Search tab: only searches addons, never your own files (the X search does those).
function streamTile(it, ck, i) {
  return `<button class="game" data-nav data-key="si-${esc(it.id)}" data-act="sopen" data-ck="${esc(ck)}" data-i="${i}">
      <span class="cwimg">${streamCover(it, it.poster)}${sFavDot(`${it.type}:${it.id}`)}</span>
      <span class="t">${esc(it.name)}</span>${it.year ? `<span class="s">${esc(it.year)}</span>` : ''}</button>`;
}
function streamSearchView(c) {
  const st = S_STREAM.search[c.sys] || {};
  const box = `<div class="ssearch"><label class="field"><span>${c.sys === 'tv' ? 'Find a show in your addons' : 'Find a movie in your addons'}</span>
    <span class="srow"><input type="text" id="sq" data-nav data-key="sq" data-enter="ssearch" data-s="${esc(c.sys)}" value="${esc(st.typed ?? st.q ?? '')}" placeholder="${c.sys === 'tv' ? 'Show name' : 'Movie name'}" autocomplete="off" spellcheck="false">
    <button class="btn primary" data-nav data-key="sqgo" data-act="ssearch" data-s="${esc(c.sys)}">${ICON.search} Search</button></span></label></div>`;
  if (!st.q) return box + `<div class="nofav" style="padding-top:40px">Searches ${esc(streamSearchCats(c.sys).map((x) => x.addonName).filter((n, i, a) => a.indexOf(n) === i).join(', '))} only. Your own files are under ${c.sys === 'tv' ? 'Your shows' : 'Your movies'}.</div>`;
  const groups = st.groups || [];
  const body = groups.map((g) => {
    const items = g.idx.map((i) => streamTile(st.items[i], c.ck, i)).join('');
    return `<section class="sgroup">${groups.length > 1 ? `<h3>${esc(g.addonName)}</h3>` : ''}${g.loading ? '<div class="note">Searching…</div>' : g.error ? `<div class="note">${esc(g.error)}</div>` : !g.idx.length ? '<div class="note">Nothing found.</div>' : `<div class="tiles stiles">${items}</div>`}</section>`;
  }).join('');
  const none = groups.length && groups.every((g) => !g.loading && !g.idx.length && !g.error);
  return box + (none ? `<div class="nofav" style="padding-top:40px">Nothing found for “${esc(st.q)}”.</div>` : body);
}
async function streamSearch(sys, q) {
  const cats = streamSearchCats(sys);
  // Group by addon (an addon with both series and anime search is one group).
  const order = [...new Set(cats.map((x) => x.addon))];
  const st = { q, typed: q, items: [], groups: order.map((a) => ({ addon: a, addonName: cats.find((x) => x.addon === a).addonName, idx: [], loading: true, error: null, pending: cats.filter((x) => x.addon === a).length })) };
  S_STREAM.search[sys] = st;
  render();
  await Promise.all(cats.map(async (c) => {
    const g = st.groups.find((x) => x.addon === c.addon);
    try {
      const found = await api.call('addon_search', { key: c.addon, kind: c.type, id: c.id, query: q });
      if (S_STREAM.search[sys] !== st) return; // a newer search replaced this one
      const seen = new Set(g.idx.map((i) => st.items[i].id));
      for (const it of found) if (!seen.has(it.id)) { seen.add(it.id); st.items.push({ ...it, _addon: c.addon, _addonName: c.addonName }); g.idx.push(st.items.length - 1); }
    } catch (e) {
      if (!g.idx.length) g.error = String(e || 'Something went wrong.');
    }
    if (--g.pending === 0) g.loading = false;
    if (S_STREAM.search[sys] === st && S.screen === 'stream') render();
  }));
}
// Remember what's typed in the addon search box across redraws.
document.addEventListener('input', (e) => {
  if (e.target.id !== 'sq') return;
  const sys = e.target.dataset.s;
  S_STREAM.search[sys] = { ...(S_STREAM.search[sys] || {}), typed: e.target.value };
});
// On the Search tab, typing or X goes to the addon search box, not the library search.
function streamSearchFocus() {
  if (S.screen !== 'stream' || S.panel) return false;
  const el = document.getElementById('sq');
  if (!el) return false;
  if (document.activeElement !== el) { el.focus(); el.setSelectionRange(el.value.length, el.value.length); S.focusKey = 'sq'; }
  return true;
}

async function streamLoad(ck) {
  const c = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === ck);
  if (!c) return;
  const cache = sCacheKey(c), genre = sGenre(c) || null;
  const d = S_STREAM.cache[cache] || (S_STREAM.cache[cache] = { items: [], done: false, loading: false, error: null, auto: 0 });
  if (d.loading || d.done) return;
  d.loading = true; d.error = null;
  if (S.screen === 'stream') render();
  try {
    const page = await api.call('addon_catalog', { key: c.addon, kind: c.type, id: c.id, skip: d.items.length, genre });
    const seen = new Set(d.items.map((x) => x.id));
    const fresh = page.filter((x) => !seen.has(x.id));
    d.items.push(...fresh);
    if (!fresh.length) d.done = true;
  } catch (e) {
    d.error = String(e || 'Something went wrong.');
  }
  d.loading = false;
  // With filters on, keep loading pages (up to 8 more) until enough titles match.
  const f = sFilt(STREAM_TYPES.tv.includes(c.type) ? 'tv' : 'movies');
  if (!d.error && !d.done && sFiltOn(f) && d.items.filter((it) => sMatch(it, f)).length < 24 && (d.auto || 0) < 8 && sCacheKey(c) === cache) {
    d.auto = (d.auto || 0) + 1;
    return streamLoad(ck);
  }
  if (S.screen === 'stream') render();
}

async function streamMeta(it, addon) {
  const k = `${it.type}:${it.id}`;
  if (k in S_STREAM.meta) return;
  S_STREAM.meta[k] = 'loading';
  let m = null;
  try { m = await api.call('addon_meta', { key: addon, kind: it.type, id: it.id }); } catch (e) { m = null; }
  S_STREAM.meta[k] = m || null;
  if (S.screen === 'sdetail' && S.sitem && S.sitem.item.id === it.id) render();
}

// ---- Resume, watched marks and Continue watching (positions saved as "stream/<type>:<id>").
const sKey = (type, vid) => `stream/${type}:${vid}`;
const sProg = (type, vid) => (S.boot.progress || {})[sKey(type, vid)] || null;
const sHasLink = (type, vid) => (S.boot.stream_links || []).includes(sKey(type, vid));
const epCodeOf = (e) => (e && e.season != null && e.episode != null ? `S${e.season} E${e.episode}` : e?.title || '');
// Episodes in viewing order (specials last, as the backend sorts them).
function sEpisodes(x) { const m = S_STREAM.meta[`${x.item.type}:${x.item.id}`]; return (m && m !== 'loading' && m.episodes) || []; }
// What a show's main button does: back into the episode you stopped in, else the next one
// after the last you watched, else nothing (the page then offers the season's first episode).
function sShowNext(x) {
  const eps = sEpisodes(x).filter((e) => e.season !== 0);
  let last = -1, at = 0;
  eps.forEach((e, i) => { const p = sProg(x.item.type, e.id); if (p && p.at >= at) { at = p.at; last = i; } });
  if (last < 0) return null;
  const e = eps[last], p = sProg(x.item.type, e.id);
  if (canResume(p) && sHasLink(x.item.type, e.id)) return { kind: 'resume', e, pos: p.pos };
  if (p.watched || finished(p)) return eps[last + 1] ? { kind: 'next', e: eps[last + 1] } : null;
  return { kind: 'again', e };
}
// What Continue watching needs to show a title again (sent with each play).
function streamInfo(vid) {
  const x = S.sitem; if (!x) return null;
  const it = { ...x.item, ...((m) => (m && m !== 'loading' ? m : {}))(S_STREAM.meta[`${x.item.type}:${x.item.id}`]) };
  const eps = sEpisodes(x);
  const i = eps.findIndex((e) => e.id === vid);
  const e = eps[i];
  return { item: `${x.item.type}:${x.item.id}`, type: x.item.type, id: x.item.id, name: it.name, poster: it.poster || null, background: it.background || null, year: it.year || null,
    addon: x.addon || null, addonName: x.addonName || null,
    episode: e ? { id: e.id, season: e.season, episode: e.episode, title: e.title } : null, last: e ? !eps.slice(i + 1).some((n) => n.season !== 0) : true };
}
// Continue watching for Movies or Shows: newest first; finished movies and shows with
// nothing left drop off.
function streamContinue(kind, max = 12) {
  const types = STREAM_TYPES[kind];
  return Object.values(S.boot.stream_recent || {}).filter((r) => types.includes(r.type)).sort((a, b) => (b.at || 0) - (a.at || 0)).map((r) => {
    const vid = r.episode ? r.episode.id : r.id;
    const p = sProg(r.type, vid);
    const done = p && (p.watched || finished(p));
    if (done && (!r.episode || r.last)) return null;
    const code = r.episode ? epCodeOf(r.episode) : '';
    const line = canResume(p) ? `Resume ${code ? code + ' · ' : ''}${clock(p.pos)}` : done ? `Next after ${code}` : code ? `Watch ${code}` : 'Watch';
    return { r, line, pct: canResume(p) && p.len ? Math.round((100 * p.pos) / p.len) : 0 };
  }).filter(Boolean).slice(0, max);
}

// Details page for a title from an addon catalog.
function sdetail() {
  const x = S.sitem;
  if (!x) { S.screen = 'stream'; return stream(); }
  const k = `${x.item.type}:${x.item.id}`;
  const T = x.item.type;
  const m = S_STREAM.meta[k];
  const loading = m === 'loading';
  const it = m && m !== 'loading' ? { ...x.item, ...Object.fromEntries(Object.entries(m).filter(([, v]) => v != null && !(Array.isArray(v) && !v.length))) } : x.item;
  const eps = (m && m !== 'loading' && m.episodes) || [];
  const next = eps.length ? sShowNext(x) : null;
  const seasonOf = (e) => (e.season == null ? 'Other' : e.season === 0 ? 'Specials' : `Season ${e.season}`);
  const groups = [...new Set(eps.map(seasonOf))];
  // Open on the season you're in the middle of.
  const group = groups.includes(S_STREAM.season[k]) ? S_STREAM.season[k] : next ? seasonOf(next.e) : groups[0];
  const shown = eps.filter((e) => seasonOf(e) === group);
  const epTitle = (e) => `${it.name} · ${e.season != null && e.episode != null ? `S${e.season} E${e.episode} ` : ''}${e.title}`;
  const backLabel = x.catLabel || 'Back';
  const meta = [
    it.year ? `<span>${esc(it.year)}</span>` : null,
    it.runtime ? `<span>${esc(it.runtime)}</span>` : null,
    it.rating ? `<span>★ ${esc(it.rating)}</span>` : null,
    eps.length ? `<span>${eps.filter((e) => e.season !== 0).length} episodes</span>` : null,
  ].filter(Boolean).join('<span class="dotsep">·</span>');
  const people = [it.director?.length ? `Directed by ${it.director.slice(0, 2).join(', ')}` : null, it.cast?.length ? `With ${it.cast.slice(0, 4).join(', ')}` : null].filter(Boolean);
  const day = (d) => { if (!d) return ''; const t = new Date(d + 'T12:00:00'); return isNaN(t) ? d : t.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); };
  const upcoming = (e) => e.released && new Date(e.released + 'T00:00:00') > new Date();
  // An episode row resumes where you stopped (same source), otherwise opens the source list.
  const epRow = (e) => {
    const p = sProg(T, e.id), resume = canResume(p) && sHasLink(T, e.id);
    const act = resume ? `data-act="sresume" data-k="${esc(T)}" data-v="${esc(e.id)}"` : `data-act="sources" data-k="${esc(T)}" data-id="${esc(e.id)}"`;
    return `<div class="eprow"><button class="ep ${upcoming(e) ? 'soon' : ''}" data-nav data-key="sep-${esc(e.id)}" ${act} data-t="${esc(epTitle(e))}" title="${resume ? `Resume at ${clock(p.pos)}` : `Choose a source for ${esc(e.title)}`}">
      <span class="thumb">${e.thumbnail ? streamImg(e.thumbnail) : `<span style="position:relative">${e.episode ?? ICON.film}</span>`}${p?.watched ? `<span class="seen" aria-label="Watched">${ICON.check}</span>` : ''}${canResume(p) ? `<span class="pbar" aria-label="Stopped at ${clock(p.pos)}"><i style="width:${Math.round((100 * p.pos) / (p.len || 1))}%"></i></span>` : ''}</span>
      <span><span class="et">${e.season != null && e.episode != null ? `<span class="n">S${e.season} · E${e.episode}</span>` : ''}${esc(e.title)}</span>
      ${e.released ? `<span class="eo" style="font-weight:600">${upcoming(e) ? 'Coming ' : ''}${esc(day(e.released))}</span>` : ''}${e.overview ? `<span class="eo">${esc(e.overview)}</span>` : ''}</span></button>
      <button class="epcheck ${p?.watched ? 'on' : ''}" data-nav data-key="sw-${esc(e.id)}" data-act="watched" data-k="${esc(sKey(T, e.id))}" data-on="${p?.watched ? 1 : 0}" title="${p?.watched ? 'Watched · click to unmark' : 'Mark as watched'}" aria-pressed="${!!p?.watched}" aria-label="Watched">${ICON.check}</button></div>`;
  };
  const epsHtml = !eps.length ? '' : `<section class="eps" aria-label="Episodes">
    <h3 style="font-family:var(--body);font-size:18px;font-weight:700">Episodes</h3>
    ${groups.length > 1 ? `<div class="seasons" role="tablist">${groups.map((g) => `<button class="chip ${g === group ? 'on' : ''}" role="tab" aria-selected="${g === group}" data-nav data-key="sgrp-${esc(g)}" data-act="sseason" data-g="${esc(g)}">${esc(g)}</button>`).join('')}</div>` : ''}
    <div class="eplist">${shown.map(epRow).join('')}</div></section>`;
  const first = shown.find((e) => !upcoming(e)) || shown[0];
  const canStream = streamAddons().some((a) => a.stream.includes(T));
  const srcBtn = (vid, title, label, primary = true, key = 'play') => `<button class="btn ${primary ? 'primary' : ''}" data-nav data-key="${key}" data-act="sources" data-k="${esc(T)}" data-id="${esc(vid)}" data-t="${esc(title)}">${primary ? glyph('A') : ''}${label}</button>`;
  let action;
  if (!canStream) action = `<span class="note">To watch, add a source addon (one set up with your TorBox key, for example) in Settings → Streaming addons.</span>`;
  else if (T === 'movie' || (!eps.length && !loading)) {
    const p = sProg(T, x.item.id), link = sHasLink(T, x.item.id);
    action = (canResume(p) && link
      ? `<button class="btn primary" data-nav data-key="play" data-act="sresume" data-k="${esc(T)}" data-v="${esc(x.item.id)}" data-t="${esc(it.name)}">${glyph('A')}Resume · ${clock(p.pos)}</button>
         <button class="btn" data-nav data-key="restart" data-act="sresume" data-k="${esc(T)}" data-v="${esc(x.item.id)}" data-t="${esc(it.name)}" data-from="start">Start over</button>
         ${srcBtn(x.item.id, it.name, 'Other source', false, 'othersrc')}`
      : srcBtn(x.item.id, it.name, 'Choose a source'))
      + `<button class="btn ${p?.watched ? 'on' : ''}" data-nav data-key="mwatched" data-act="watched" data-k="${esc(sKey(T, x.item.id))}" data-on="${p?.watched ? 1 : 0}" aria-pressed="${!!p?.watched}">${ICON.check} ${p?.watched ? 'Watched' : 'Mark as watched'}</button>`;
  } else if (next?.kind === 'resume') {
    action = `<button class="btn primary" data-nav data-key="play" data-act="sresume" data-k="${esc(T)}" data-v="${esc(next.e.id)}" data-t="${esc(epTitle(next.e))}">${glyph('A')}Resume ${esc(epCodeOf(next.e))} · ${clock(next.pos)}</button>
      ${srcBtn(next.e.id, epTitle(next.e), 'Other source', false, 'othersrc')}`;
  } else if (next) {
    action = srcBtn(next.e.id, epTitle(next.e), `${next.kind === 'next' ? 'Next:' : 'Watch'} ${esc(epCodeOf(next.e))}`) + `<span class="note">Choose a source for it, or pick any episode below</span>`;
  } else if (first) {
    action = srcBtn(first.id, epTitle(first), `Choose a source · ${first.season != null ? `S${first.season} E${first.episode}` : esc(first.title)}`) + `<span class="note">or pick any episode below</span>`;
  } else action = '';
  const inContinue = !!(S.boot.stream_recent || {})[k];
  const fav = sInList(SL_FAV, k), later = sInList(SL_LATER, k), inOwn = sOwnLists().some((n) => sInList(n, k));
  const listBtns = `<button class="btn small ${fav ? 'on' : ''}" data-nav data-key="sfav" data-act="sfav" aria-pressed="${fav}">${glyph('Y')}${fav ? 'Remove favorite' : 'Favorite'}</button>
      <button class="btn small ${later ? 'on' : ''}" data-nav data-key="slater" data-act="slater" aria-pressed="${later}">${later ? ICON.check + ' In Watch later' : 'Watch later'}</button>
      <button class="btn small" data-nav data-key="slists" data-act="slists">${ICON.list} ${inOwn ? 'In your lists' : 'Add to list'}</button>`;
  const bg = it.background;
  return topbar() + `<main class="main"><div class="backdrop" aria-hidden="true">${it.poster ? streamImg(it.poster) : ''}</div>
  <div class="detail scroll ${bg ? 'has-hero' : ''}" id="detailscroll">
   ${bg ? `<div class="herobanner" aria-hidden="true">${streamImg(bg)}</div>` : ''}
   <div class="detail-top">
    <div class="left"><div class="media">${streamCover(it, bigPoster(it.poster))}</div>
      <div class="coverbtns">${listBtns}${inContinue ? `<button class="btn small" data-nav data-key="sforget" data-act="sforget" data-item="${esc(k)}">Remove from Continue watching</button>` : ''}</div></div>
    <div class="right">
      <button class="back" data-nav data-key="sback" data-act="back">${ICON.back}${esc(backLabel)}</button>
      ${it.logo ? `<h2 class="haslogo"><img class="gamelogo" src="${esc(it.logo)}" alt="${esc(it.name)}" referrerpolicy="no-referrer" onerror="this.parentNode.classList.remove('haslogo'); this.replaceWith(this.alt)"></h2>` : `<h2>${esc(it.name)}</h2>`}
      ${meta ? `<div class="metaline">${meta}</div>` : ''}
      ${it.genres?.length ? `<div class="cats" role="list" aria-label="Genres">${it.genres.slice(0, 6).map((g) => `<span class="cat" role="listitem">${esc(g)}</span>`).join('')}</div>` : ''}
      ${it.description ? `<p class="desc">${esc(it.description)}</p>` : loading ? '<p class="desc" style="opacity:.6">Getting details…</p>' : ''}
      ${people.length ? `<p class="note">${esc(people.join(' · '))}</p>` : ''}
      <div class="actions">${action}</div>
      <div class="facts"><div class="fact"><span class="k">From</span><span class="v">${esc(x.addonName || 'Addon')}</span><span class="n">Streaming addon</span></div></div>
    </div>
   </div>
   ${epsHtml}
   ${loading && T !== 'movie' ? '<div class="nofav" style="padding:30px 0">Getting episodes…</div>' : ''}
  </div></main>` + hints('From a streaming addon', [['A', 'Select'], ['B', 'Back'], ['Y', 'Favorite']]);
}

// ---- "Choose a source": what your source addons offer for a movie or episode.
function sourcesPanel() {
  if (S.panel !== 'sources' || !S.src) return '';
  const x = S.src;
  let body;
  if (x.loading) body = `<div class="nofav" style="padding:40px 0"><div class="spinner" style="margin:0 auto 18px"></div>Asking your addons…</div>`;
  else if (x.error) body = `<div class="warning">${esc(x.error)}</div>`;
  else if (!x.groups.length) body = `<div class="note">None of your addons offers sources for this. Add a source addon (set up with your TorBox key) in Settings → Streaming addons.</div>`;
  else {
    let i = 0;
    body = x.groups.map((g) => {
      const rows = g.sources.map((src) => {
        const lines = src.details.split('\n').map((l) => l.trim()).filter(Boolean);
        const head = src.name.split('\n').map((l) => l.trim()).filter(Boolean).join(' · ');
        return `<button class="srcrow" data-nav data-key="src-${i++}" data-act="splay" data-tok="${esc(src.token)}">
          <span class="badges">${src.quality ? `<span class="q">${esc(src.quality)}</span>` : ''}${src.cached === true ? '<span class="ready">Ready</span>' : src.cached === false ? '<span class="wait">Not cached</span>' : ''}${src.size ? `<span class="sz">${esc(src.size)}</span>` : ''}</span>
          <span class="sn">${esc(head || g.addon)}</span>${lines.length ? `<span class="sd">${lines.slice(0, 3).map(esc).join('<br>')}</span>` : ''}</button>`;
      }).join('');
      const note = g.error ? `<div class="note">${esc(g.error)}</div>` : !g.sources.length ? `<div class="note">${g.torrents ? `${g.torrents} torrent result${g.torrents === 1 ? '' : 's'}, but Spine plays links from a debrid service (like TorBox). Set this addon up with your TorBox key.` : 'Nothing found.'}</div>` : '';
      return `<section class="srcgroup"><h3>${esc(g.addon)}${g.sources.length ? ` <span class="rs">${g.sources.length}</span>` : ''}</h3>${note}${rows ? `<div class="srclist">${rows}</div>` : ''}</section>`;
    }).join('');
  }
  return `<div class="panel" data-act="closepanel"><aside class="sheet wide" role="dialog" aria-label="Choose a source">
    <h2>Choose a source</h2><p class="note" style="margin-top:-6px">${esc(x.title)}</p>
    <span class="note">"Ready" sources are already on your debrid service and start right away. Plays in VLC.</span>
    ${body}
  </aside></div>`;
}

// ---- "Add to list" for a streamed title: Watch later and your own streaming lists.
function streamListsPanel() {
  if (S.panel !== 'slists' || !S.slist) return '';
  const x = S.slist;
  const names = [SL_LATER, ...sOwnLists()];
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Streaming lists">
    <h2>Lists for ${esc(x.name)}</h2>
    <span class="note">Streaming lists show up as tabs on the Streaming screen. They're separate from your game lists.</span>
    <div class="choices">${names.map((n, i) => { const on = sInList(n, x.item), cnt = (sLists()[n] || []).length; return `<button class="choice ${on ? 'on' : ''}" data-nav data-key="sls-${i}" data-act="sltoggle" data-n="${esc(n)}">
      <span class="t">${esc(n)}</span><span class="n">${cnt} title${cnt === 1 ? '' : 's'}${on ? ' · in this list' : ''}</span></button>`; }).join('')}</div>
    <div class="grp"><span class="k">New list</span>
      <label class="field"><input type="text" id="snl-name" data-nav data-key="snl-name" data-enter="slnew" placeholder="List name, like “Movie night”" maxlength="40" autocomplete="off" spellcheck="false"></label>
      <div class="opts"><button class="btn" data-nav data-key="snl-make" data-act="slnew">${ICON.plus} Make list and add</button></div></div>
  </aside></div>`;
}
// The title a Y press means: the focused poster on the Streaming screen, or the open details page.
function streamTitleAt(el) {
  const d = el?.dataset || {};
  if (S.screen === 'stream' && d.act === 'sopen') { const f = streamFind(d.ck, d.i); return f && sListInfo(f.it, f.c.addon, f.c.addonName); }
  if (S.screen === 'stream' && d.act === 'scontinue') { const r = (S.boot.stream_recent || {})[d.item]; return r && sListInfo(r, r.addon, r.addonName); }
  if (S.screen === 'sdetail' && S.sitem) return sDetailInfo();
  return null;
}
function sDetailInfo() {
  const x = S.sitem;
  const m = S_STREAM.meta[`${x.item.type}:${x.item.id}`];
  const it = { ...x.item, ...(m && m !== 'loading' ? Object.fromEntries(['name', 'poster', 'year'].filter((f) => m[f]).map((f) => [f, m[f]])) : {}) };
  return sListInfo(it, x.addon, x.addonName);
}
// Y on the Streaming screens: favorite / unfavorite (true = handled here).
function streamY(el) {
  if (S.screen !== 'stream' && S.screen !== 'sdetail') return false;
  const info = streamTitleAt(el);
  if (info) streamToggle(SL_FAV, info);
  return true;
}
async function streamToggle(name, info, quiet) {
  const on = sInList(name, info.item);
  const key = document.activeElement?.dataset?.key || null;
  try { S.boot = await api.call('stream_list_set', { name, info, member: !on }); } catch (e) { return toast(String(e), 4000); }
  SFX.play(on ? 'back' : 'select');
  S.focusKey = key; render();
  if (!quiet) toast(name === SL_FAV ? (on ? `Removed ${info.name} from Favorites` : `Added ${info.name} to Favorites`) : on ? `Removed from “${name}”` : `Added to “${name}”`, 1800);
}
// A title behind a poster: from a catalog, a search or one of your lists.
function streamFind(ck, i) {
  if (!ck) return null;
  if (ck.startsWith('search:')) {
    const it = S_STREAM.search[ck.slice(7)]?.items[+i];
    return it && { it, c: { addon: it._addon, addonName: it._addonName, label: 'Search' } };
  }
  if (ck.startsWith('list:')) {
    const tab = streamMyTabs(streamKind()).find((t) => t.ck === ck);
    const r = tab?.items[+i];
    return r && { it: { id: r.id, type: r.type, name: r.name, poster: r.poster, year: r.year, genres: [] }, c: { addon: r.addon, addonName: r.addonName, label: tab.label } };
  }
  const c = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === ck);
  const it = c && S_STREAM.cache[sCacheKey(c)]?.items[+i];
  return c && it ? { it, c } : null;
}

// ---- Pick a genre (or year) for a catalog: the addon sends only those titles.
function streamGenrePanel() {
  if (S.panel !== 'sgenre') return '';
  const c = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === S_STREAM.gpick);
  if (!c) return '';
  const yp = sYearPicker(c), cur = sGenre(c);
  const opts = (c.genreRequired ? [] : ['']).concat(c.genres);
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="${yp ? 'Year' : 'Genre'}">
    <h2>${yp ? 'Year' : 'Genre'}</h2><span class="note">${esc(c.label)} · from ${esc(c.addonName)}</span>
    <div class="choices sgenres">${opts.map((g, i) => `<button class="choice ${g === cur ? 'on' : ''}" data-nav data-key="sg-${i}" data-act="sgenrepick" data-g="${esc(g)}"><span class="t">${esc(g || (yp ? 'All years' : 'All genres'))}</span></button>`).join('')}</div>
  </aside></div>`;
}

// After a filter changes: redraw and, if few titles match, look further in the catalog.
function sFiltChanged(focus) {
  S.focusKey = focus;
  const cur = streamCur(streamKind());
  if (cur && !cur.search && !cur.mine) { const d = S_STREAM.cache[sCacheKey(cur)]; if (d) { d.auto = 0; if (!d.loading) { render(); return streamLoad(cur.ck); } } }
  render();
}

// ---- Settings → Addons
function addonSettings() {
  const list = streamAddons();
  return `<div class="grp"><span class="k">Streaming addons</span><span class="note">Paste an addon's link from Stremio or Nuvio. Links stay on this PC.</span>
    ${list.length ? `<div class="roots">${list.map((a, i) => `<div class="root addonrow">${a.logo ? `<img class="alogo" src="${esc(a.logo)}" alt="" referrerpolicy="no-referrer" onerror="this.style.visibility='hidden'">` : '<span class="alogo"></span>'}
      <span class="rp" title="${esc(a.description || '')}">${esc(a.name)}${a.version ? ` <span class="rs">${esc(a.version)}</span>` : ''}</span>
      <span class="rs">${esc(addonWhat(a))}</span>
      <span class="abtns">${i > 0 ? `<button class="btn small" data-nav data-key="aup-${a.key}" data-act="mvaddon" data-k="${a.key}" data-up="1" aria-label="Move up">↑</button>` : ''}
      <button class="btn small" data-nav data-key="arm-${a.key}" data-act="rmaddon" data-k="${a.key}">Remove</button></span></div>`).join('')}</div>` : ''}
    <div class="opts"><button class="btn" data-nav data-key="addaddon" data-act="addaddon">${ICON.plus} Add addon</button></div></div>`;
}
function addonWhat(a) {
  const n = a.catalogs.filter((c) => !c.needs_input).length;
  const parts = [];
  if (n) parts.push(`${n} catalog${n === 1 ? '' : 's'}`);
  if (a.stream.length) parts.push('sources');
  if (a.meta.length && !n) parts.push('details');
  return parts.join(' · ') || 'Nothing for Spine yet';
}

function addonPanel() {
  if (S.panel !== 'addon') return '';
  return `<div class="panel" data-act="closepanel"><aside class="sheet" role="dialog" aria-label="Add addon">
    <button class="back" data-nav data-key="adback" data-act="settings">${ICON.back}Settings</button>
    <h2>Add addon</h2>
    <div class="grp"><span class="note">Copy the addon's link from its setup page (the "Copy link" or "Install" button), or from Stremio or Nuvio, and paste it here. It usually ends in <b>manifest.json</b>. If you set the addon up with your TorBox key, that key is inside the link, so it's saved on this PC only.</span>
      <label class="field"><span>Addon link</span><input type="password" id="addonurl" data-nav data-key="addonurl" placeholder="https://…/manifest.json" autocomplete="off" spellcheck="false"></label>
      ${S.addonErr ? `<div class="warning">${esc(S.addonErr)}</div>` : ''}</div>
    <div class="opts"><button class="btn primary" data-nav data-key="addonsave" data-act="addonsave" ${S.addonBusy ? 'disabled' : ''}>${S.addonBusy ? 'Checking the addon…' : `${glyph('A')}Add`}</button></div>
  </aside></div>`;
}

// After a play starts: the interface's copy of Continue watching and Resume links.
function streamPlayed(type, vid, info) {
  const k = sKey(type, vid);
  if (!(S.boot.stream_links || []).includes(k)) S.boot.stream_links = [...(S.boot.stream_links || []), k];
  if (info?.item) S.boot.stream_recent = { ...(S.boot.stream_recent || {}), [info.item]: { ...info, key: `${type}:${vid}`, at: Math.floor(Date.now() / 1000) } };
}

const STREAM_ACT = {
  async sources(d) {
    const req = { title: d.t, kind: d.k, id: d.id, info: streamInfo(d.id), loading: true, groups: [] };
    S.src = req; S.srcFrom = document.activeElement?.dataset?.key || 'play';
    S.panel = 'sources'; S.focusKey = null; render();
    try {
      const groups = await api.call('addon_streams', { kind: d.k, id: d.id });
      if (S.src !== req) return;
      req.groups = groups;
    } catch (e) {
      if (S.src !== req) return;
      req.error = String(e);
    }
    req.loading = false;
    if (S.panel === 'sources') { S.focusKey = 'src-0'; render(); }
  },
  // Resume (or Start over) with the source played last time.
  async sresume(d) {
    stopPreview();
    const info = streamInfo(d.v);
    S.playing = { title: d.t, emulator: 'VLC', movie: true };
    render();
    try {
      const emu = await api.call('play_stream', { token: null, key: `${d.k}:${d.v}`, title: d.t, info, fromStart: d.from === 'start' });
      S.playing = { ...S.playing, emulator: emu, started: true };
      streamPlayed(d.k, d.v, info);
    } catch (e) {
      S.playing = { ...S.playing, error: String(e) };
    }
    S.focusKey = 'closeplay';
    render();
  },
  scontinue(d) {
    const r = (S.boot.stream_recent || {})[d.item];
    if (!r) return;
    const it = { id: r.id, type: r.type, name: r.name, poster: r.poster, background: r.background, year: r.year, genres: [] };
    S.sitem = { item: it, addon: r.addon, addonName: r.addonName, catLabel: 'Streaming', backKey: `cw-${d.item}` };
    S.panel = null; S.screen = 'sdetail'; S.focusKey = 'play';
    render();
    document.getElementById('detailscroll')?.scrollTo(0, 0);
    streamMeta(it, r.addon);
  },
  async sforget(d) {
    S.boot = await api.call('stream_forget', { item: d.item });
    S.focusKey = 'play'; render();
  },
  async splay(d) {
    const x = S.src;
    if (!x) return;
    S.panel = null; stopPreview();
    S.playing = { title: x.title, emulator: 'VLC', movie: true };
    render();
    try {
      const emu = await api.call('play_stream', { token: d.tok, key: `${x.kind}:${x.id}`, title: x.title, info: x.info, fromStart: true });
      S.playing = { ...S.playing, emulator: emu, started: true };
      streamPlayed(x.kind, x.id, x.info);
    } catch (e) {
      S.playing = { ...S.playing, error: String(e) };
    }
    S.focusKey = 'closeplay';
    render();
  },
  skind(d) {
    S_STREAM.kind = d.k; S.focusKey = `sk-${d.k}`;
    render();
    const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0;
  },
  stab(d) {
    S_STREAM.tab[d.s] = d.ck || null;
    const search = (d.ck || '').startsWith('search:');
    S.focusKey = search && !S_STREAM.search[d.s]?.q ? 'sq' : `st-${d.ck || 'own'}`;
    render();
    const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0;
    if (d.ck && !search) streamLoad(d.ck);
  },
  ssearch(d) {
    const el = document.getElementById('sq');
    const q = (el?.value || '').trim();
    if (!q) { el?.focus(); S.focusKey = 'sq'; return; }
    S.focusKey = 'sqgo';
    el?.blur();
    streamSearch(d.s, q);
  },
  smore(d) {
    const cat = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === d.ck);
    const key = cat ? sCacheKey(cat) : d.ck;
    const c = S_STREAM.cache[key];
    if (c) { c.error = null; c.auto = 0; if (!c.items.length) delete S_STREAM.cache[key]; }
    S.focusKey = 'smore';
    streamLoad(d.ck);
  },
  sopen(d) {
    const f = streamFind(d.ck, d.i);
    if (!f) return;
    const { c, it } = f;
    S.sitem = { item: it, addon: c.addon, addonName: c.addonName, catLabel: c.label, backKey: `si-${it.id}` };
    S.panel = null; S.screen = 'sdetail'; S.focusKey = 'sback';
    render();
    document.getElementById('detailscroll')?.scrollTo(0, 0);
    streamMeta(it, c.addon);
  },
  sfav() { if (S.sitem) streamToggle(SL_FAV, sDetailInfo()); },
  slater() { if (S.sitem) streamToggle(SL_LATER, sDetailInfo()); },
  slists() { if (!S.sitem) return; S.slist = sDetailInfo(); S.panel = 'slists'; S.focusKey = null; render(); },
  sltoggle(d) { if (S.slist) streamToggle(d.n, S.slist, true); },
  async slnew() {
    const name = (document.getElementById('snl-name')?.value || '').trim();
    if (!name || !S.slist) return;
    try { S.boot = await api.call('stream_list_set', { name, info: S.slist, member: true }); } catch (e) { return toast(String(e), 4000); }
    S.focusKey = 'snl-name'; render(); toast(`Added to “${name}”`);
  },
  async sldelete(d) {
    S.boot = await api.call('stream_list_delete', { name: d.n });
    const kind = streamKind();
    if (S_STREAM.tab[kind] === `list:${d.n}`) S_STREAM.tab[kind] = null;
    S.focusKey = null; render();
    toast(`Deleted the list “${d.n}”`, 3000);
  },
  sgenre(d) {
    S_STREAM.gpick = d.ck;
    const c = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === d.ck);
    const opts = (c && !c.genreRequired ? [''] : []).concat(c ? c.genres : []);
    S.panel = 'sgenre'; S.focusKey = `sg-${Math.max(0, opts.indexOf(c ? sGenre(c) : ''))}`; render();
  },
  sgenrepick(d) {
    S_STREAM.genre[S_STREAM.gpick] = d.g || '';
    S.panel = null; S.focusKey = 'sfgenre';
    render();
    const g = document.getElementById('gamesscroll'); if (g) g.scrollTop = 0;
    streamLoad(S_STREAM.gpick);
  },
  sfstep(d) {
    const f = sFilt(streamKind());
    if (d.f === 'anime') f.anime = !f.anime;
    if (d.f === 'decade') f.decade = S_DECADES[(S_DECADES.indexOf(f.decade) + 1) % S_DECADES.length];
    if (d.f === 'rating') f.rating = S_RATINGS[(S_RATINGS.indexOf(f.rating) + 1) % S_RATINGS.length];
    sFiltChanged(`sf${d.f}`);
  },
  sfclear(d) {
    S_STREAM.filt[streamKind()] = { anime: false, decade: '', rating: 0 };
    if (d.ck) S_STREAM.genre[d.ck] = '';
    sFiltChanged('sfrating');
    const cur = streamCur(streamKind()); if (cur && !cur.search && !cur.mine) streamLoad(cur.ck);
  },
  sseason(d) {
    const k = `${S.sitem.item.type}:${S.sitem.item.id}`;
    S_STREAM.season[k] = d.g; S.focusKey = `sgrp-${d.g}`; render();
  },
  addaddon() { S.addonErr = null; S.addonBusy = false; S.panel = 'addon'; S.focusKey = 'addonurl'; render(); },
  async addonsave() {
    const el = document.getElementById('addonurl');
    const url = (el?.value || '').trim();
    if (!url) { S.addonErr = 'Paste the addon link first.'; S.focusKey = 'addonurl'; return render(); }
    S.addonBusy = true; S.addonErr = null; S.focusKey = 'addonsave'; render();
    const keep = document.getElementById('addonurl'); if (keep) keep.value = url;
    try {
      const before = new Set(streamAddons().map((a) => a.key));
      S.boot = await api.call('add_addon', { url });
      const added = streamAddons().find((a) => !before.has(a.key));
      S.addonBusy = false; S.panel = 'settings'; S.focusKey = 'addaddon';
      render();
      toast(added ? `Added ${added.name}.` : 'Addon added.');
    } catch (e) {
      S.addonBusy = false; S.addonErr = String(e); S.focusKey = 'addonurl';
      render();
      const again = document.getElementById('addonurl'); if (again) again.value = url;
    }
  },
  async rmaddon(d) {
    const a = streamAddons().find((x) => x.key === d.k);
    S.boot = await api.call('remove_addon', { key: d.k });
    for (const k of Object.keys(S_STREAM.cache)) if (k.startsWith(d.k + '|')) delete S_STREAM.cache[k];
    S.focusKey = 'addaddon'; render();
    if (a) toast(`Removed ${a.name}.`);
  },
  async mvaddon(d) {
    S.boot = await api.call('move_addon', { key: d.k, up: d.up === '1' });
    S.focusKey = `aup-${d.k}`; render();
  },
};
