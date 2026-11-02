// Blades: a Home style made of big sliding panels, one per console (plus
// Recent, Favorites, your lists and Surprise me). Earlier blades fold into
// thin tabs on the left, later ones on the right; LB/RB or left/right slide
// between them. Each blade has a list on the left and a large picture of the
// highlighted item on the right. No music; it has its own set of interface
// sounds (see SFX in app.js). Settings → Home style → Blades. Loaded before app.js.
// Reuses waveCats() and waveItems() from wave.js for what's on each blade.

const bladesOn = () => settings().home_style === 'blades';

function bladeState() {
  S.blade = S.blade || { cat: null, at: {}, dir: 0 };
  const cats = waveCats();
  let i = cats.findIndex((c) => c.id === S.blade.cat);
  if (i < 0) i = Math.max(0, cats.findIndex((c) => c.ti === S.sel && c.kind !== 'recent'));
  S.blade.cat = cats[i].id;
  return { cats, i, cat: cats[i] };
}
const bladeFocusKey = () => { const { cat } = bladeState(); const it = waveItems(cat); return it[Math.min(S.blade.at[cat.id] || 0, it.length - 1)]?.key; };

// The big picture on the right for the highlighted item.
function bladePreview(c, it) {
  if (!it) return '';
  if (it.g) {
    const { g, s } = it, inf = g.info || {};
    const meta = [s.short, yearOf(g), inf.genre].filter(Boolean).map(esc).join(' · ');
    return `<div class="bl-art">${g.hero ? `<img class="bg" src="${mediaUrl(g.hero)}" alt="">` : g.cover ? `<img class="bg blur" src="${mediaUrl(g.cover)}" alt="">` : ''}
        ${g.cover ? `<img class="cv" src="${mediaUrl(g.cover)}" alt="">` : ''}</div>
      <div class="bl-info">${g.logo ? `<img class="logo" src="${mediaUrl(g.logo)}" alt="${esc(g.title)}">` : `<h3>${esc(g.title)}</h3>`}
        <p class="meta">${meta}</p><p class="sub">${esc(it.sub)}</p>
        ${inf.description ? `<p class="desc">${esc(inf.description)}</p>` : ''}</div>`;
  }
  // Openers and Surprise me: the console's icon over a mosaic of its covers.
  const pool = c.kind === 'sys' ? c.sys.games : c.kind === 'fav' ? favItems().map((x) => x.g) : c.kind === 'coll' ? collItems(c.coll).map((x) => x.g)
    : systems().filter((x) => !isMovies(x)).flatMap((x) => x.games);
  const covers = pool.filter((g) => g.cover);
  const step = Math.max(1, Math.floor(covers.length / 8));
  const pick = covers.filter((_, k) => k % step === 0).slice(0, 8);
  return `<div class="bl-art mosaic">${pick.map((g) => `<img src="${mediaUrl(g.cover)}" alt="" loading="lazy">`).join('')}
      <span class="bigicon" style="background:${c.color}">${sysIcon(it.random ? 'random' : tileIcon(c), 120)}</span></div>
    <div class="bl-info"><h3>${esc(it.title)}</h3><p class="sub">${esc(it.sub)}</p></div>`;
}

function homeBlades() {
  const { cats, i, cat } = bladeState();
  const items = waveItems(cat);
  const at = Math.min(S.blade.at[cat.id] || 0, items.length - 1);
  S.blade.at[cat.id] = at;
  const tab = (c, k) => `<button class="bl-tab" style="--tc:${c.color}" data-act="bladego" data-i="${k}" tabindex="-1" aria-label="${esc(c.name)}"><span>${esc(c.short)}</span></button>`;
  const left = cats.slice(0, i).map((c, k) => tab(c, k)).join('');
  const right = cats.slice(i + 1).map((c, k) => tab(c, i + 1 + k)).join('');
  const dir = S.blade.dir; S.blade.dir = 0;
  const list = items.map((it) => `<button class="bl-item${it.off ? ' off' : ''}" data-nav data-key="${esc(it.key)}" ${it.attrs}>
      ${it.thumb}<span class="t">${esc(it.title)}</span>${it.fav ? `<span class="wv-fav">${ICON.heart}</span>` : ''}</button>`).join('');
  const count = cat.kind === 'recent' ? `${items.length} recently played` : cat.count != null ? `${cat.count.toLocaleString()} ${cat.kind === 'sys' ? things(cat.sys, cat.count) : cat.count === 1 ? 'game' : 'games'}` : 'Random pick';
  return topbar() + `<main class="main blades" id="blades" style="--bc:${cat.color}">
      <div class="bl-tabs left" style="--n:${i}">${left}</div>
      <section class="bl-panel ${dir > 0 ? 'in-r' : dir < 0 ? 'in-l' : ''}">
        <header class="bl-head"><span class="ic">${sysIcon(tileIcon(cat), 40)}</span><h2>${esc(cat.name)}</h2><span class="n">${esc(count)}</span></header>
        <div class="bl-body">
          <div class="bl-list scroll" id="bllist">${list}</div>
          <div class="bl-preview" id="blprev">${bladePreview(cat, items[at])}</div>
        </div>
      </section>
      <div class="bl-tabs right" style="--n:${cats.length - i - 1}">${right}</div>
    </main>` + hints('LB / RB slide between blades', [['A', 'Open'], ['Y', 'Favorite'], ['X', 'Search'], ['☰', 'Settings']]);
}

// Go to blade j (slides in from the side it came from).
function bladeGo(j) {
  const { cats, i } = bladeState();
  if (j < 0 || j >= cats.length || j === i) return false;
  S.blade.dir = j > i ? 1 : -1;
  S.blade.cat = cats[j].id;
  SFX.play(j > i ? 'bladeR' : 'bladeL');
  S.focusKey = bladeFocusKey();
  render();
  return true;
}

// Controller / keyboard: left/right and LB/RB slide blades; up/down is the normal list move.
function bladesKey(btn) {
  if (!['left', 'right', 'LB', 'RB'].includes(btn)) return false;
  const cur = document.activeElement;
  if ((btn === 'left' || btn === 'right') && !cur?.classList?.contains('bl-item')) return false; // topbar: normal moves
  const { i } = bladeState();
  if (!bladeGo(i + (btn === 'right' || btn === 'RB' ? 1 : -1))) SFX.play('edge');
  return true;
}

// Keep the preview and the remembered spot in step with focus and mouse hover.
function bladesMount() {
  const list = document.getElementById('bllist');
  if (!list) return;
  const show = (el) => {
    const { cat } = bladeState();
    const k = [...list.children].indexOf(el);
    if (k < 0) return;
    const prev = document.getElementById('blprev');
    if (prev && prev.dataset.k !== String(k)) { prev.dataset.k = k; prev.innerHTML = bladePreview(cat, waveItems(cat)[k]); }
    return k;
  };
  list.addEventListener('focusin', (e) => { const el = e.target.closest('.bl-item'); if (!el) return; const k = show(el); if (k != null) S.blade.at[bladeState().cat.id] = k; });
  list.addEventListener('mouseover', (e) => { const el = e.target.closest('.bl-item'); if (el) show(el); });
  list.addEventListener('mouseleave', () => { const f = list.querySelector('.bl-item.focused'); if (f) show(f); });
}
