// Streaming addons (Stremio-style): catalogs on the Movies and TV screens, and a
// details page for titles that aren't on your drives. Loaded before app.js.
// The backend never sends an addon's link here (it can hold your keys), only a `key`.

// Which addon catalog types belong on which screen.
const STREAM_TYPES = { movies: ['movie'], tv: ['series', 'anime'] };
const S_STREAM = { tab: {}, cache: {}, meta: {}, season: {}, search: {} };

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
// The catalog shown on this screen right now (null = your own files).
function streamActive(s) {
  if (!s || s.kind !== 'sys') return null;
  const ck = S_STREAM.tab[s.id];
  if (ck === `search:${s.id}`) return streamSearchCats(s.id).length ? { ck, search: true, sys: s.id, label: 'Search', addonName: 'Search your addons' } : null;
  return ck ? streamCats(s.id).find((c) => c.ck === ck) || null : null;
}

const streamImg = (u, cls = '') => (u ? `<img ${cls ? `class="${cls}" ` : ''}src="${esc(u)}" alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror="this.remove()">` : '');
// A poster over a title card: if the picture can't load, the title shows instead.
const streamCover = (it, src) => `<div class="cover ph scover" style="background:${PAL[hash(it.name) % PAL.length]}"><span class="pt">${esc(it.name)}</span><span class="ps">${esc(it.year || '')}</span>${streamImg(src)}</div>`;
// Stremio's poster service hands out small posters by default; pages look better with bigger ones.
const bigPoster = (u) => (u || '').replace('/poster/small/', '/poster/medium/');

function streamTabs(s) {
  const cats = streamCats(s.id);
  const canSearch = streamSearchCats(s.id).length > 0;
  if (!cats.length && !canSearch) return '';
  const cur = streamActive(s);
  const sk = `search:${s.id}`;
  return `<div class="streamtabs" role="tablist" aria-label="Catalogs">
    <button class="chip ${!cur ? 'on' : ''}" role="tab" aria-selected="${!cur}" data-nav data-key="st-own" data-act="stab" data-s="${esc(s.id)}" data-ck="">${s.id === 'tv' ? 'Your shows' : 'Your movies'}</button>
    ${canSearch ? `<button class="chip ${cur?.search ? 'on' : ''}" role="tab" aria-selected="${!!cur?.search}" data-nav data-key="st-${sk}" data-act="stab" data-s="${esc(s.id)}" data-ck="${sk}" title="Search your addons (not your own files)">${ICON.search} Search addons</button>` : ''}
    ${cats.map((c) => `<button class="chip ${cur?.ck === c.ck ? 'on' : ''}" role="tab" aria-selected="${cur?.ck === c.ck}" data-nav data-key="st-${esc(c.ck)}" data-act="stab" data-s="${esc(s.id)}" data-ck="${esc(c.ck)}" title="${esc(c.addonName)}">${esc(c.label)}</button>`).join('')}
  </div>`;
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
    if (S_STREAM.search[sys] === st && S.screen === 'games') render();
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
  if (S.screen !== 'games' || S.panel) return false;
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
  if (S.screen === 'games') render();
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
  if (S.screen === 'games') render();
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

// Details page for a title from an addon catalog.
function sdetail() {
  const x = S.sitem;
  if (!x) { S.screen = 'games'; return games(); }
  const k = `${x.item.type}:${x.item.id}`;
  const m = S_STREAM.meta[k];
  const loading = m === 'loading';
  const it = m && m !== 'loading' ? { ...x.item, ...Object.fromEntries(Object.entries(m).filter(([, v]) => v != null && !(Array.isArray(v) && !v.length))) } : x.item;
  const eps = (m && m !== 'loading' && m.episodes) || [];
  const seasonOf = (e) => (e.season == null ? 'Other' : e.season === 0 ? 'Specials' : `Season ${e.season}`);
  const groups = [...new Set(eps.map(seasonOf))];
  const group = groups.includes(S_STREAM.season[k]) ? S_STREAM.season[k] : groups[0];
  const shown = eps.filter((e) => seasonOf(e) === group);
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
  const epsHtml = !eps.length ? '' : `<section class="eps" aria-label="Episodes">
    <h3 style="font-family:var(--body);font-size:18px;font-weight:700">Episodes</h3>
    ${groups.length > 1 ? `<div class="seasons" role="tablist">${groups.map((g) => `<button class="chip ${g === group ? 'on' : ''}" role="tab" aria-selected="${g === group}" data-nav data-key="sgrp-${esc(g)}" data-act="sseason" data-g="${esc(g)}">${esc(g)}</button>`).join('')}</div>` : ''}
    <div class="eplist">${shown.map((e) => `<div class="eprow"><button class="ep ${upcoming(e) ? 'soon' : ''}" data-nav data-key="sep-${esc(e.id)}" data-act="noop" title="${esc(e.title)}">
      <span class="thumb">${e.thumbnail ? streamImg(e.thumbnail) : `<span style="position:relative">${e.episode ?? ICON.film}</span>`}</span>
      <span><span class="et">${e.season != null && e.episode != null ? `<span class="n">S${e.season} · E${e.episode}</span>` : ''}${esc(e.title)}</span>
      ${e.released ? `<span class="eo" style="font-weight:600">${upcoming(e) ? 'Coming ' : ''}${esc(day(e.released))}</span>` : ''}${e.overview ? `<span class="eo">${esc(e.overview)}</span>` : ''}</span></button></div>`).join('')}</div></section>`;
  const bg = it.background;
  return topbar() + `<main class="main"><div class="backdrop" aria-hidden="true">${it.poster ? streamImg(it.poster) : ''}</div>
  <div class="detail scroll ${bg ? 'has-hero' : ''}" id="detailscroll">
   ${bg ? `<div class="herobanner" aria-hidden="true">${streamImg(bg)}</div>` : ''}
   <div class="detail-top">
    <div class="left"><div class="media">${streamCover(it, bigPoster(it.poster))}</div></div>
    <div class="right">
      <button class="back" data-nav data-key="sback" data-act="back">${ICON.back}${esc(backLabel)}</button>
      ${it.logo ? `<h2 class="haslogo"><img class="gamelogo" src="${esc(it.logo)}" alt="${esc(it.name)}" referrerpolicy="no-referrer" onerror="this.parentNode.classList.remove('haslogo'); this.replaceWith(this.alt)"></h2>` : `<h2>${esc(it.name)}</h2>`}
      ${meta ? `<div class="metaline">${meta}</div>` : ''}
      ${it.genres?.length ? `<div class="cats" role="list" aria-label="Genres">${it.genres.slice(0, 6).map((g) => `<span class="cat" role="listitem">${esc(g)}</span>`).join('')}</div>` : ''}
      ${it.description ? `<p class="desc">${esc(it.description)}</p>` : loading ? '<p class="desc" style="opacity:.6">Getting details…</p>' : ''}
      ${people.length ? `<p class="note">${esc(people.join(' · '))}</p>` : ''}
      <div class="actions"><span class="note">${x.item.type === 'movie' ? 'Choosing a source and playing come in the next Spine update.' : 'Choosing a source and playing episodes come in the next Spine update.'}</span></div>
      <div class="facts"><div class="fact"><span class="k">From</span><span class="v">${esc(x.addonName || 'Addon')}</span><span class="n">Streaming addon</span></div></div>
    </div>
   </div>
   ${epsHtml}
   ${loading && x.item.type !== 'movie' ? '<div class="nofav" style="padding:30px 0">Getting episodes…</div>' : ''}
  </div></main>` + hints('From a streaming addon', [['B', 'Back']]);
}

// ---- Settings → Addons
function addonSettings() {
  const list = streamAddons();
  const sys = systems().map((s) => s.id);
  const hidden = list.some((a) => a.catalogs.some((c) => !c.needs_input)) && !sys.includes('movies') && !sys.includes('tv');
  return `<div class="grp"><span class="k">Streaming addons</span><span class="note">Stremio-style addons add catalogs like Popular and Trending to your Movies and TV screens. Paste an addon's link (the same one you'd use in Stremio or Nuvio). Links with your keys in them stay on this PC and are never shown again.</span>
    ${list.length ? `<div class="roots">${list.map((a, i) => `<div class="root addonrow">${a.logo ? `<img class="alogo" src="${esc(a.logo)}" alt="" referrerpolicy="no-referrer" onerror="this.style.visibility='hidden'">` : '<span class="alogo"></span>'}
      <span class="rp" title="${esc(a.description || '')}">${esc(a.name)}${a.version ? ` <span class="rs">${esc(a.version)}</span>` : ''}</span>
      <span class="rs">${esc(addonWhat(a))}</span>
      <span class="abtns">${i > 0 ? `<button class="btn small" data-nav data-key="aup-${a.key}" data-act="mvaddon" data-k="${a.key}" data-up="1" aria-label="Move up">↑</button>` : ''}
      <button class="btn small" data-nav data-key="arm-${a.key}" data-act="rmaddon" data-k="${a.key}">Remove</button></span></div>`).join('')}</div>` : ''}
    ${hidden ? '<span class="note">Add a movie or TV folder too: addon catalogs show up on those screens.</span>' : ''}
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

const STREAM_ACT = {
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
    S.sitem = { item: it, addon: c.addon, addonName: c.addonName, catLabel: c.label };
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
