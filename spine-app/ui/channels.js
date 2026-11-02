// Channels: a Home style with a grid of rounded tiles, one per console, each
// with its covers slowly drifting by and a clock along the bottom
// (Settings → Home style → Channels). Its music is in music.js. Loaded before app.js.

const channelsOn = () => settings().home_style === 'channels';
const CH_PER_PAGE = 12; // 4 across, 3 down

function chPageOf(i) { return Math.floor(i / CH_PER_PAGE); }

function chTileArt(t) {
  const pool = t.kind === 'sys' ? t.sys.games.map((g) => ({ s: t.sys, g })) : t.kind === 'fav' ? favItems() : t.kind === 'coll' ? collItems(t.coll)
    : systems().filter((x) => !isMovies(x)).flatMap((x) => x.games.map((g) => ({ s: x, g })));
  const withCovers = pool.filter((x) => x.g.cover);
  const step = t.kind === 'random' ? 29 : Math.max(1, Math.floor(withCovers.length / 7));
  const pick = withCovers.filter((_, i) => i % step === 0).slice(0, 7);
  if (pick.length < 3) return `<span class="ch-plain" style="background:${t.color}">${sysIcon(tileIcon(t), 96)}</span>`;
  const imgs = pick.map((x) => `<img src="${mediaUrl(x.g.cover)}" alt="" loading="lazy" decoding="async">`).join('');
  // Every tile drifts at its own pace and starting point so the grid never moves in lockstep.
  const h = hash(t.id);
  return `<span class="ch-strip" style="animation-duration:${38 + (h % 17)}s;animation-delay:${-(h % 40)}s">${imgs}${imgs}</span>`
    + (t.kind === 'random' ? `<span class="ch-badge">${sysIcon('random', 64)}</span>` : '');
}

function homeChannels() {
  const T = tiles();
  // Keep the page that holds the console you're on (Back from a library, LB/RB).
  const want = S.focusKey?.startsWith('con-') ? T.findIndex((t) => `con-${t.id}` === S.focusKey) : -1;
  const pages = Math.max(1, Math.ceil(T.length / CH_PER_PAGE));
  if (want >= 0) S.chPage = chPageOf(want);
  else if (S.chPage == null) S.chPage = chPageOf(Math.min(S.sel, T.length - 1));
  S.chPage = Math.min(S.chPage, pages - 1);
  const first = S.chPage * CH_PER_PAGE;
  const slots = [];
  for (let k = 0; k < CH_PER_PAGE; k++) {
    const i = first + k, t = T[i];
    if (!t) { slots.push('<span class="ch-tile empty" aria-hidden="true"></span>'); continue; }
    const count = t.count != null ? `${t.count.toLocaleString()} ${t.kind === 'sys' ? things(t.sys, t.count) : t.count === 1 ? 'game' : 'games'}` : 'Random pick';
    slots.push(`<button class="ch-tile" style="--chip:${t.color}" data-nav data-key="con-${esc(t.id)}" data-act="tile" data-i="${i}" aria-label="${esc(t.name)}, ${esc(count)}">
        <span class="ch-art">${chTileArt(t)}</span>
        <span class="ch-name"><span class="n">${esc(t.short)}</span><span class="c">${esc(count)}</span></span>
        ${t.kind === 'sys' && t.sys.blocked ? `<span class="ch-warn" title="${esc(t.sys.blocked)}">!</span>` : ''}</button>`);
  }
  const now = new Date();
  const time = now.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const date = now.toLocaleDateString([], { weekday: 'short', month: 'numeric', day: 'numeric' });
  const arrow = (d, on) => on ? `<button class="ch-page ${d < 0 ? 'prev' : 'next'}" data-act="chpage" data-d="${d}" tabindex="-1" aria-label="${d < 0 ? 'Previous' : 'Next'} page">
      <svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="${d < 0 ? 'M15 5l-7 7 7 7' : 'M9 5l7 7-7 7'}"/></svg></button>` : '';
  const offline = (S.boot.roots || []).some((r) => !r.online);
  return `<main class="main channels" id="channels">
      <div class="ch-grid">${slots.join('')}</div>
      ${arrow(-1, S.chPage > 0)}${arrow(1, S.chPage < pages - 1)}
      ${pages > 1 ? `<div class="ch-dots">${Array.from({ length: pages }, (_, p) => `<span class="${p === S.chPage ? 'on' : ''}"></span>`).join('')}</div>` : ''}
      <footer class="ch-bar">
        <button class="ch-round" data-nav data-key="gear" data-act="settings" aria-label="Settings">${ICON.gear}<span>Settings</span></button>
        <div class="ch-clock"><span class="t" id="clock">${time}</span><span class="d" id="chdate">${date}${offline ? ' · a drive is unplugged' : ''}</span></div>
        <button class="ch-round" data-nav data-key="searchbtn" data-act="search" aria-label="Search">${ICON.search}<span>Search</span></button>
      </footer>
    </main>`;
}

// Left/right past the edge of a page turns the page. Returns true when handled.
function channelsKey(btn) {
  if (btn !== 'left' && btn !== 'right') return false;
  const cur = document.activeElement;
  if (!cur?.classList?.contains('ch-tile')) return false;
  const T = tiles();
  const i = +cur.dataset.i, col = (i % CH_PER_PAGE) % 4;
  const pages = Math.ceil(T.length / CH_PER_PAGE);
  const d = btn === 'right' ? 1 : -1;
  if ((d > 0 && col !== 3 && T[i + 1]) || (d < 0 && col !== 0)) return false; // normal move inside the page
  const p = S.chPage + d;
  if (p < 0 || p >= pages) return false; // let the normal move say "edge"
  const row = Math.floor((i % CH_PER_PAGE) / 4);
  let j = p * CH_PER_PAGE + row * 4 + (d > 0 ? 0 : 3);
  j = Math.min(j, T.length - 1);
  S.chPage = p; S.focusKey = `con-${T[j].id}`;
  SFX.play('tick');
  render();
  return true;
}
