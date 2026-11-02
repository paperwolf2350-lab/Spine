// Streaming addons (Stremio-style): the Streaming tile on Home opens its own screen
// (screen 'stream') with Movies / Shows, a Search tab and each addon catalog, plus a
// details page (screen 'sdetail'). Kept apart from your own Movies and TV on purpose.
// Loaded before app.js.
// The backend never sends an addon's link here (it can hold your keys), only a `key`.

// Which addon catalog types go under Movies and under Shows.
const STREAM_TYPES = { movies: ['movie'], tv: ['series', 'anime'] };
const STREAM_KINDS = [['movies', 'Movies'], ['tv', 'Shows']];
const STREAM_COLOR = '#0E6E77';
const S_STREAM = { kind: null, tab: {}, cache: {}, meta: {}, season: {}, search: {} };

function streamAddons() { return S.boot?.addons || []; }
// Browsable catalogs for the Movies or TV screen, in addon order.
function streamCats(sysId) {
  const types = STREAM_TYPES[sysId];
  if (!types) return [];
  const out = [];
  for (const a of streamAddons()) for (const c of a.catalogs) {
    if (c.needs_input || !types.includes(c.type)) continue;
    out.push({ ck: `${a.key}|${c.type}|${c.id}`, addon: a.key, addonName: a.name, type: c.type, id: c.id, name: c.name });
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
// Tabs for a kind: Search first (when an addon can search), then every catalog.
function streamTabList(kind) {
  const out = [];
  if (streamSearchCats(kind).length) out.push({ ck: `search:${kind}`, search: true, sys: kind, label: 'Search', addonName: 'Searches your addons' });
  return out.concat(streamCats(kind));
}
// The tab shown right now: the one picked last, else the first catalog (else Search).
function streamCur(kind) {
  const list = streamTabList(kind);
  return list.find((c) => c.ck === S_STREAM.tab[kind]) || list.find((c) => !c.search) || list[0] || null;
}

const streamImg = (u, cls = '') => (u ? `<img ${cls ? `class="${cls}" ` : ''}src="${esc(u)}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror="this.remove()">` : '');
// A poster over a title card: if the picture can't load, the title shows instead.
const streamCover = (it, src) => `<div class="cover ph scover" style="background:${PAL[hash(it.name) % PAL.length]}"><span class="pt">${esc(it.name)}</span><span class="ps">${esc(it.year || '')}</span>${streamImg(src)}</div>`;
// Stremio's poster service hands out small posters by default; pages look better with bigger ones.
const bigPoster = (u) => (u || '').replace('/poster/small/', '/poster/medium/');

function streamTabs(kind) {
  const cur = streamCur(kind);
  return `<div class="streamtabs" role="tablist" aria-label="Catalogs">
    ${streamTabList(kind).map((c) => `<button class="chip ${cur?.ck === c.ck ? 'on' : ''}" role="tab" aria-selected="${cur?.ck === c.ck}" data-nav data-key="st-${esc(c.ck)}" data-act="stab" data-s="${esc(kind)}" data-ck="${esc(c.ck)}" title="${esc(c.addonName)}">${c.search ? ICON.search + ' ' : ''}${esc(c.label)}</button>`).join('')}
  </div>`;
}

// The Streaming screen.
function stream() {
  if (!streamAddons().length) { S.screen = 'home'; return home(); }
  const kinds = streamKinds();
  const kind = streamKind();
  const cur = streamCur(kind);
  if (cur && !cur.search && !S_STREAM.cache[cur.ck]) setTimeout(() => streamLoad(cur.ck));
  const body = !kinds.length
    ? `<div class="nofav">Your addons don't have catalogs or search. Add a catalog addon in Settings → Streaming addons, for example Cinemeta:<br><span class="path">https://v3-cinemeta.strem.io/manifest.json</span></div>`
    : cur ? (cur.search ? '' : streamContinueRow(kind)) + streamGrid(cur) : '';
  return topbar() + `<main class="main">
    <div class="gamesbar">
      <button class="iconbtn" data-nav data-key="sgback" data-act="back" aria-label="Back to Home">${ICON.back}</button>
      <span class="hdricon" style="background:${STREAM_COLOR}">${sysIcon('streaming', 30)}</span><h2>Streaming</h2>
      <span style="color:var(--muted)">${esc(cur ? (cur.search ? cur.addonName : `From ${cur.addonName}`) : '')}</span>
      <div class="spacer"></div>
      ${kinds.length > 1 ? `<div class="seg skinds" role="group" aria-label="Movies or shows">${kinds.map(([k, l]) => `<button class="${k === kind ? 'on' : ''}" data-nav data-key="sk-${k}" data-act="skind" data-k="${k}">${l}</button>`).join('')}</div>` : ''}
    </div>
    ${kinds.length ? streamTabs(kind) : ''}
    <div class="games-wrap"><div class="games scroll" id="gamesscroll">${body}</div></div>
  </main>` + hints('Movies and shows from your streaming addons', [['A', 'Open'], ['B', 'Back'], ['LB / RB', 'Catalog'], kinds.length > 1 ? ['LT / RT', 'Movies / Shows'] : null].filter(Boolean));
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

function streamGrid(c) {
  if (c.search) return streamSearchView(c);
  const d = S_STREAM.cache[c.ck];
  if (!d || (d.loading && !d.items.length)) return `<div class="nofav"><div class="spinner" style="margin:0 auto 18px"></div>Loading ${esc(c.label)}…</div>`;
  if (d.error && !d.items.length) return `<div class="nofav">${esc(d.error)}<div style="margin-top:18px"><button class="btn" data-nav data-key="sretry" data-act="smore" data-ck="${esc(c.ck)}">Try again</button></div></div>`;
  if (!d.items.length) return `<div class="nofav">This catalog is empty right now.</div>`;
  const tiles = d.items.map((it, i) => `<button class="game" data-nav data-key="si-${esc(it.id)}" data-act="sopen" data-ck="${esc(c.ck)}" data-i="${i}">
      ${streamCover(it, it.poster)}
      <span class="t">${esc(it.name)}</span>${it.year ? `<span class="s">${esc(it.year)}</span>` : ''}</button>`).join('');
  const more = d.done ? '' : `<div class="smore">${d.error ? `<span class="note">${esc(d.error)}</span>` : ''}<button class="btn" data-nav data-key="smore" data-act="smore" data-ck="${esc(c.ck)}" ${d.loading ? 'disabled' : ''}>${d.loading ? 'Loading…' : 'Show more'}</button></div>`;
  return `<div class="tiles stiles">${tiles}</div>${more}`;
}

// ---- Search tab: only searches addons, never your own files (the X search does those).
function streamTile(it, ck, i) {
  return `<button class="game" data-nav data-key="si-${esc(it.id)}" data-act="sopen" data-ck="${esc(ck)}" data-i="${i}">
      ${streamCover(it, it.poster)}
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
  const d = S_STREAM.cache[ck] || (S_STREAM.cache[ck] = { items: [], done: false, loading: false, error: null });
  if (d.loading || d.done) return;
  d.loading = true; d.error = null;
  if (S.screen === 'stream') render();
  try {
    const page = await api.call('addon_catalog', { key: c.addon, kind: c.type, id: c.id, skip: d.items.length });
    const seen = new Set(d.items.map((x) => x.id));
    const fresh = page.filter((x) => !seen.has(x.id));
    d.items.push(...fresh);
    if (!fresh.length) d.done = true;
  } catch (e) {
    d.error = String(e || 'Something went wrong.');
  }
  d.loading = false;
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
function streamContinue(kind) {
  const types = STREAM_TYPES[kind];
  return Object.values(S.boot.stream_recent || {}).filter((r) => types.includes(r.type)).sort((a, b) => (b.at || 0) - (a.at || 0)).map((r) => {
    const vid = r.episode ? r.episode.id : r.id;
    const p = sProg(r.type, vid);
    const done = p && (p.watched || finished(p));
    if (done && (!r.episode || r.last)) return null;
    const code = r.episode ? epCodeOf(r.episode) : '';
    const line = canResume(p) ? `Resume ${code ? code + ' · ' : ''}${clock(p.pos)}` : done ? `Next after ${code}` : code ? `Watch ${code}` : 'Watch';
    return { r, line, pct: canResume(p) && p.len ? Math.round((100 * p.pos) / p.len) : 0 };
  }).filter(Boolean).slice(0, 12);
}
function streamContinueRow(kind) {
  const list = streamContinue(kind);
  if (!list.length) return '';
  return `<section class="morerow scontinue"><h3>Continue watching</h3><div class="strip">${list.map(({ r, line, pct }) => `<button class="game" data-nav data-key="cw-${esc(r.item)}" data-act="scontinue" data-item="${esc(r.item)}">
      <span class="cwimg">${streamCover({ name: r.name, year: r.year }, bigPoster(r.poster))}${pct ? `<span class="pbar"><i style="width:${pct}%"></i></span>` : ''}</span>
      <span class="t">${esc(r.name)}</span><span class="s">${esc(line)}</span></button>`).join('')}</div></section>`;
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
  const bg = it.background;
  return topbar() + `<main class="main"><div class="backdrop" aria-hidden="true">${it.poster ? streamImg(it.poster) : ''}</div>
  <div class="detail scroll ${bg ? 'has-hero' : ''}" id="detailscroll">
   ${bg ? `<div class="herobanner" aria-hidden="true">${streamImg(bg)}</div>` : ''}
   <div class="detail-top">
    <div class="left"><div class="media">${streamCover(it, bigPoster(it.poster))}</div>
      ${inContinue ? `<div class="coverbtns"><button class="btn small" data-nav data-key="sforget" data-act="sforget" data-item="${esc(k)}">Remove from Continue watching</button></div>` : ''}</div>
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
  </div></main>` + hints('From a streaming addon', [['A', 'Select'], ['B', 'Back']]);
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
    const c = S_STREAM.cache[d.ck];
    if (c) { c.error = null; if (!c.items.length) delete S_STREAM.cache[d.ck]; }
    S.focusKey = 'smore';
    streamLoad(d.ck);
  },
  sopen(d) {
    let c, it;
    if (d.ck.startsWith('search:')) {
      it = S_STREAM.search[d.ck.slice(7)]?.items[+d.i];
      c = it && { addon: it._addon, addonName: it._addonName, label: 'Search' };
    } else {
      c = streamCats('movies').concat(streamCats('tv')).find((x) => x.ck === d.ck);
      it = S_STREAM.cache[d.ck]?.items[+d.i];
    }
    if (!c || !it) return;
    S.sitem = { item: it, addon: c.addon, addonName: c.addonName, catLabel: c.label, backKey: `si-${it.id}` };
    S.panel = null; S.screen = 'sdetail'; S.focusKey = 'sback';
    render();
    document.getElementById('detailscroll')?.scrollTo(0, 0);
    streamMeta(it, c.addon);
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
