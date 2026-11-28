// Theme packs & Simple Mode runtime (Phases 1 + 2): applies custom theme CSS,
// fonts, colors, wallpapers (global & per-console), custom Settings options,
// console icon packs, Home music packs, UI sound effect packs, custom mascot
// packs, and Simple Mode visual customizations layered cleanly on top.
// Loaded before music.js, mascot.js and app.js.
'use strict';

const THEMES = (() => {
  const BUILTIN_THEMES = ['cream', 'midnight', 'crt', 'glass-light', 'glass-dark'];
  const sfxCache = new Map(); // url -> AudioBuffer | 'loading' | 'err'
  let appliedOpts = [];
  let wpLayer = null;

  // Curated Simple Mode color palettes (one-tap controller-friendly presets).
  const PALETTES = [
    { id: 'oled',     name: 'OLED Pitch',    base: 'midnight', bg: '#050608', surface: '#111319', text: '#F4F5F8', accent: '#3B82F6' },
    { id: 'espresso', name: 'Cozy Espresso', base: 'midnight', bg: '#171311', surface: '#231D1A', text: '#F5EDE6', accent: '#E07A3E' },
    { id: 'nord',     name: 'Nord Frost',    base: 'midnight', bg: '#1C222D', surface: '#262E3D', text: '#ECEFF4', accent: '#88C0D0' },
    { id: 'synth',    name: 'Synthwave',     base: 'midnight', bg: '#120B21', surface: '#1E1336', text: '#F6ECFF', accent: '#FF3B9A' },
    { id: 'forest',   name: 'Forest Sage',   base: 'midnight', bg: '#111915', surface: '#1A2620', text: '#EDF5EF', accent: '#3FB979' },
    { id: 'amber',    name: 'Amber Console', base: 'midnight', bg: '#100D08', surface: '#1B160D', text: '#FFD580', accent: '#F59E0B' },
    { id: 'sakura',   name: 'Sakura Cream',  base: 'cream',    bg: '#FAF4F5', surface: '#FFFFFF', text: '#271C20', accent: '#D64572' },
    { id: 'paper',    name: 'Warm Paper',    base: 'cream',    bg: '#F3EFE4', surface: '#FFFDF8', text: '#1F1B16', accent: '#B45309' },
  ];

  // Controller-friendly quick swatches for individual color slots in Simple Mode.
  const SWATCHES = {
    bg:      ['#050608', '#121318', '#171311', '#1C222D', '#120B21', '#111915', '#F6F2EA', '#FAF4F5'],
    surface: ['#111319', '#1C1E25', '#231D1A', '#262E3D', '#1E1336', '#1A2620', '#FFFFFF', '#FFFDF8'],
    text:    ['#F4F5F8', '#F2F0EB', '#F5EDE6', '#FFD580', '#B6FFA6', '#1C1A17', '#271C20', '#1E293B'],
    accent:  ['#C8431F', '#3B82F6', '#237A52', '#7C3AED', '#FF3B9A', '#E09A3E', '#0E7490', '#D64572'],
  };

  // Bundled font choices for Simple Mode.
  const FONTS = [
    { id: '',       label: 'Theme default', display: '', body: '' },
    { id: 'modern', label: 'Modern',        display: "'Space Grotesk', 'Segoe UI', system-ui, sans-serif", body: "'Inter', 'Segoe UI', system-ui, sans-serif" },
    { id: 'clean',  label: 'Clean (Lexend)', display: "'Lexend', 'Segoe UI', system-ui, sans-serif", body: "'Lexend', 'Segoe UI', system-ui, sans-serif" },
    { id: 'retro',  label: 'Retro Pixel',   display: "'VT323', ui-monospace, Consolas, monospace", body: "'VT323', ui-monospace, Consolas, monospace" },
    { id: 'system', label: 'System UI',     display: "system-ui, -apple-system, 'Segoe UI', sans-serif", body: "system-ui, -apple-system, 'Segoe UI', sans-serif" },
    { id: 'serif',  label: 'Classic Serif', display: "Georgia, 'Times New Roman', serif", body: "Georgia, 'Times New Roman', serif" },
    { id: 'mono',   label: 'Monospace',     display: "ui-monospace, 'Cascadia Mono', Consolas, monospace", body: "ui-monospace, 'Cascadia Mono', Consolas, monospace" },
  ];

  // Built-in theme base color defaults (used to derive matching colors in Simple Mode).
  const BASE_COLORS = {
    cream:        { bg: '#F6F2EA', surface: '#FFFFFF', text: '#1C1A17', accent: '#C8431F' },
    midnight:     { bg: '#121318', surface: '#1C1E25', text: '#F2F0EB', accent: '#C8431F' },
    crt:          { bg: '#070B07', surface: '#0E160E', text: '#B6FFA6', accent: '#4ADE80' },
    'glass-light': { bg: '#E8F0FC', surface: '#FFFFFF', text: '#101626', accent: '#007AFF' },
    'glass-dark':  { bg: '#080C16', surface: '#1C243A', text: '#F4F7FF', accent: '#4DA3FF' },
    tv:           { bg: '#101218', surface: '#1A1D26', text: '#F2EFE8', accent: '#E09A3E' },
  };

  const list = () => S.boot?.themes || [];
  const byId = (id) => (id ? list().find((t) => t.id === id) || null : null);

  // Active visual theme id for the current mode (Desktop vs TV mode are kept separate).
  const activeId = () => {
    const st = settings();
    return tvOn() ? (st.tvTheme || 'tv') : (st.theme || 'cream');
  };
  const activePack = () => byId(activeId());

  // Simple Mode customizer state (kept separate for Desktop and TV mode).
  const customKey = () => (tvOn() ? 'tvCustomLook' : 'customLook');
  const customSlot = (kind) => (tvOn() ? `tv-${kind}` : kind);
  const custom = () => settings()[customKey()] || {};
  const hasCustom = () => {
    const c = custom();
    return Object.keys(c).some((k) => c[k] !== undefined && c[k] !== null && c[k] !== '');
  };

  // Color mixing utilities for auto-deriving surface2, muted, line, and focus.
  const isHex = (s) => typeof s === 'string' && /^#[0-9a-fA-F]{6}$/.test(s.trim());
  function parseHex(h) {
    const x = h.trim().slice(1);
    return [parseInt(x.slice(0, 2), 16), parseInt(x.slice(2, 4), 16), parseInt(x.slice(4, 6), 16)];
  }
  function mixHex(a, b, t) {
    if (!isHex(a) || !isHex(b)) return a || b || '#888888';
    const [r1, g1, b1] = parseHex(a), [r2, g2, b2] = parseHex(b);
    const ch = (x, y) => Math.round(x + (y - x) * t).toString(16).padStart(2, '0');
    return '#' + ch(r1, r2) + ch(g1, g2) + ch(b1, b2);
  }
  function lum(h) {
    if (!isHex(h)) return 0.5;
    const [r, g, b] = parseHex(h);
    return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  }

  // Compute the current effective core colors (base -> active pack -> Simple Mode overrides).
  function effectiveCoreColors() {
    const id = activeId();
    const pack = activePack();
    const baseKey = pack ? (pack.base || (tvOn() ? 'tv' : 'midnight')) : (BASE_COLORS[id] ? id : (tvOn() ? 'tv' : 'cream'));
    const b = BASE_COLORS[baseKey] || BASE_COLORS.midnight;
    const pc = pack?.colors || {};
    const c = custom();
    return {
      bg:      isHex(c.bg)      ? c.bg      : (isHex(pc.bg)      ? pc.bg      : b.bg),
      surface: isHex(c.surface) ? c.surface : (isHex(pc.surface) ? pc.surface : b.surface),
      text:    isHex(c.text)    ? c.text    : (isHex(pc.text)    ? pc.text    : b.text),
      accent:  isHex(c.accent)  ? c.accent  : (isHex(settings().accent) ? settings().accent : (isHex(pc.accent) ? pc.accent : b.accent)),
    };
  }

  function derivePalette(core) {
    const dark = lum(core.bg) < 0.45;
    return {
      bg: core.bg,
      surface: core.surface,
      surface2: dark ? mixHex(core.surface, core.text, 0.08) : mixHex(core.surface, core.bg, 0.45),
      text: core.text,
      muted: mixHex(core.text, core.bg, 0.36),
      line: mixHex(core.surface, core.text, dark ? 0.14 : 0.16),
      focus: core.text,
      accent: core.accent,
    };
  }

  // Resolve a sub-pack (icons, music, sounds, mascot):
  //   setting === '' / unset  -> use active theme if it bundles that asset, else built-in
  //   setting === 'builtin'   -> always use Spine's built-in asset
  //   setting === '<packId>'  -> use that specific pack's asset
  function resolvePack(settingKey, flagKey) {
    const pref = settings()[settingKey] || '';
    if (pref === 'builtin' || pref === 'paige') return null;
    if (pref) {
      const p = byId(pref);
      if (p && p[flagKey]) return p;
    }
    const cur = activePack();
    return cur && cur[flagKey] ? cur : null;
  }

  const iconPack = () => resolvePack('iconPack', 'has_icons');
  const musicPack = () => resolvePack('musicPack', 'has_music');
  const soundPack = () => resolvePack('soundPack', 'has_sounds');
  const mascotPack = () => resolvePack('mascotPack', 'has_mascot');

  // Effective built-in sound style override ('default' | 'blades' | 'cube' | '').
  function soundStyle() {
    const c = custom();
    if (c.soundStyle) return c.soundStyle;
    return activePack()?.style?.soundStyle || '';
  }

  // Read the type ("toggle" | "choice") of a creator-defined option (Rust serializes it as `type`).
  function optionType(opt) {
    return String(opt?.type || opt?.kind || 'toggle').toLowerCase();
  }

  // Read the current value of a creator-defined option on a theme pack.
  function optionValue(pack, opt) {
    const kind = optionType(opt);
    const saved = settings().themeOptions?.[pack.id]?.[opt.id];
    if (kind === 'toggle') {
      if (saved !== undefined && saved !== null) return saved === true || saved === 'on' || saved === 'true';
      return opt.default === true || opt.default === 'on' || opt.default === 'true';
    }
    if (typeof saved === 'string' && (!opt.choices?.length || opt.choices.some((c) => String(c.value) === saved))) {
      return saved;
    }
    if (typeof opt.default === 'string' && opt.default) return opt.default;
    return opt.choices?.[0]?.value || '';
  }

  function ensureWpLayer() {
    if (wpLayer && wpLayer.isConnected) return wpLayer;
    wpLayer = document.getElementById('theme-bg');
    if (!wpLayer) {
      wpLayer = document.createElement('div');
      wpLayer.id = 'theme-bg';
      wpLayer.setAttribute('aria-hidden', 'true');
      wpLayer.innerHTML = '<div class="tbg-img"></div><div class="tbg-scrim"></div>';
      document.body.insertBefore(wpLayer, document.body.firstChild);
    }
    return wpLayer;
  }

  function fontFormat(path) {
    const clean = path.split('?')[0].toLowerCase();
    if (clean.endsWith('.woff2')) return 'woff2';
    if (clean.endsWith('.woff')) return 'woff';
    if (clean.endsWith('.otf')) return 'opentype';
    return 'truetype';
  }

  // Apply the active theme + any Simple Mode customizations (called by applyTheme() in app.js).
  function apply() {
    const st = settings();
    const tv = tvOn();
    const root = document.documentElement;
    const style = root.style;
    const id = activeId();
    const pack = byId(id);
    const c = custom();

    root.dataset.tv = tv ? 'on' : '';

    // Clear previous theme option attributes.
    for (const k of appliedOpts) root.removeAttribute('data-opt-' + k);
    appliedOpts = [];

    let varEl = document.getElementById('spine-theme-vars');
    let cssEl = document.getElementById('spine-theme-css');
    let customEl = document.getElementById('spine-custom-look');

    // Reset style dataset attributes before layering theme + Simple Mode.
    delete root.dataset.themeRadius;
    delete root.dataset.themeGlass;
    delete root.dataset.themeCoverwall;

    if (pack) {
      root.dataset.theme = tv && pack.base === 'tv' ? 'tv' : (pack.base || (tv ? 'tv' : 'midnight'));
      root.dataset.themePack = pack.id;

      const ts = pack.style || {};
      if (ts.radius && ts.radius !== 'rounded') root.dataset.themeRadius = ts.radius;
      if (ts.glass) root.dataset.themeGlass = 'on';
      if (ts.coverWall === false) root.dataset.themeCoverwall = 'off';

      let cssText = '';
      for (const f of pack.fonts || []) {
        const fam = String(f.family || '').replace(/[^a-zA-Z0-9 _-]/g, '').trim();
        if (!fam || !f.path) continue;
        cssText += `@font-face { font-family: '${fam}'; src: url('${mediaUrl(f.path)}') format('${fontFormat(f.path)}'); font-display: swap; }\n`;
      }
      const vars = [];
      for (const [k, v] of Object.entries(pack.colors || {})) {
        if (!/^[a-z0-9_-]+$/i.test(k)) continue;
        const val = String(v ?? '').trim();
        if (!val || val.length > 160 || /[;{}<>\\\r\n]/.test(val) || /url\(|@import|expression\(/i.test(val)) continue;
        vars.push(`  --${k}: ${val};`);
      }
      if (typeof ts.wallpaperDim === 'number') vars.push(`  --theme-wp-dim: ${Math.max(0, Math.min(0.95, ts.wallpaperDim))};`);
      if (typeof ts.wallpaperBlur === 'number') vars.push(`  --theme-wp-blur: ${Math.max(0, Math.min(40, ts.wallpaperBlur))}px;`);
      if (vars.length) cssText += `:root {\n${vars.join('\n')}\n}\n`;

      if (!varEl) {
        varEl = document.createElement('style');
        varEl.id = 'spine-theme-vars';
        document.head.appendChild(varEl);
      }
      if (varEl.textContent !== cssText) varEl.textContent = cssText;

      for (const opt of pack.options || []) {
        if (!/^[a-z0-9_-]+$/i.test(String(opt.id || ''))) continue;
        const val = optionValue(pack, opt);
        const attrVal = optionType(opt) === 'toggle' ? (val ? 'on' : 'off') : String(val);
        root.setAttribute('data-opt-' + opt.id, attrVal);
        appliedOpts.push(opt.id);
      }

      if (pack.css) {
        const href = mediaUrl(pack.css);
        if (!cssEl) {
          cssEl = document.createElement('link');
          cssEl.id = 'spine-theme-css';
          cssEl.rel = 'stylesheet';
          document.head.appendChild(cssEl);
        }
        if (cssEl.getAttribute('href') !== href) cssEl.setAttribute('href', href);
        if (cssEl.previousElementSibling !== varEl && document.head.lastElementChild !== cssEl) {
          document.head.appendChild(cssEl);
        }
      } else if (cssEl) {
        cssEl.remove();
        cssEl = null;
      }
    } else {
      root.dataset.theme = tv ? (BUILTIN_THEMES.includes(id) ? id : 'tv') : (BUILTIN_THEMES.includes(id) ? id : 'cream');
      delete root.dataset.themePack;
      if (varEl) varEl.remove();
      if (cssEl) { cssEl.remove(); cssEl = null; }
    }

    // Layer Simple Mode customizations cleanly on top of the active theme.
    if (hasCustom()) {
      if (c.radius === 'sharp' || c.radius === 'soft') root.dataset.themeRadius = c.radius;
      else if (c.radius === 'rounded') delete root.dataset.themeRadius;
      if (c.glass === true) root.dataset.themeGlass = 'on';
      else if (c.glass === false) delete root.dataset.themeGlass;
      if (c.coverWall === false) root.dataset.themeCoverwall = 'off';
      else if (c.coverWall === true) delete root.dataset.themeCoverwall;

      let customCss = '';
      const cvars = [];

      // If any core color is customized, derive matching surface2/muted/line/focus so contrast stays clean.
      if (isHex(c.bg) || isHex(c.surface) || isHex(c.text)) {
        const pal = derivePalette(effectiveCoreColors());
        cvars.push(`  --bg: ${pal.bg};`);
        cvars.push(`  --surface: ${pal.surface};`);
        cvars.push(`  --surface2: ${pal.surface2};`);
        cvars.push(`  --text: ${pal.text};`);
        cvars.push(`  --muted: ${pal.muted};`);
        cvars.push(`  --line: ${pal.line};`);
        cvars.push(`  --focus: ${pal.focus};`);
      }

      // Custom uploaded font file wins over bundled font preset.
      if (c.fontFile) {
        const fam = 'SpineCustomFont';
        customCss += `@font-face { font-family: '${fam}'; src: url('${mediaUrl(c.fontFile)}') format('${fontFormat(c.fontFile)}'); font-display: swap; }\n`;
        cvars.push(`  --display: '${fam}', system-ui, sans-serif;`);
        cvars.push(`  --body: '${fam}', system-ui, sans-serif;`);
      } else if (c.font) {
        const fp = FONTS.find((f) => f.id === c.font);
        if (fp && fp.display) {
          cvars.push(`  --display: ${fp.display};`);
          cvars.push(`  --body: ${fp.body};`);
        }
      }

      if (typeof c.wallpaperDim === 'number') cvars.push(`  --theme-wp-dim: ${Math.max(0, Math.min(0.95, c.wallpaperDim))};`);
      if (typeof c.wallpaperBlur === 'number') cvars.push(`  --theme-wp-blur: ${Math.max(0, Math.min(40, c.wallpaperBlur))}px;`);

      if (cvars.length) customCss += `:root {\n${cvars.join('\n')}\n}\n`;

      if (customCss) {
        if (!customEl) {
          customEl = document.createElement('style');
          customEl.id = 'spine-custom-look';
          document.head.appendChild(customEl);
        } else if (document.head.lastElementChild !== customEl) {
          document.head.appendChild(customEl);
        }
        if (customEl.textContent !== customCss) customEl.textContent = customCss;
      } else if (customEl) {
        customEl.remove();
      }
    } else if (customEl) {
      customEl.remove();
    }

    // Accent color priority: Simple Mode accent -> Settings accent swatch -> Theme pack accent (via #spine-theme-vars / theme.css) -> Built-in theme default.
    const builtinAccent = id === 'glass-light' ? '#007AFF' : id === 'glass-dark' ? '#4DA3FF' : (tv ? (typeof TV_ACCENT !== 'undefined' ? TV_ACCENT : '#E09A3E') : '#C8431F');
    const userAccent = (isHex(c.accent) ? c.accent : '') || st.accent || '';
    if (userAccent) {
      style.setProperty('--accent', userAccent);
    } else if (pack?.colors?.accent) {
      style.removeProperty('--accent');
    } else {
      style.setProperty('--accent', builtinAccent);
    }
    const effAccent = userAccent || pack?.colors?.accent || builtinAccent;
    if (tv && typeof tvInk === 'function') style.setProperty('--accent-ink', tvInk(effAccent));
    else style.removeProperty('--accent-ink');

    syncWallpaper();
  }

  // Build a complete Theme Pack spec from current Simple Mode settings for Save & Export.
  function buildCustomThemeSpec(name) {
    const c = custom();
    const pack = activePack();
    const core = effectiveCoreColors();
    const pal = derivePalette(core);
    const colors = { ...pal };

    if (!c.fontFile && c.font) {
      const fp = FONTS.find((f) => f.id === c.font);
      if (fp && fp.display) {
        colors.display = fp.display;
        colors.body = fp.body;
      }
    }

    const base = lum(core.bg) < 0.45 ? (tvOn() ? 'tv' : 'midnight') : 'cream';
    const style = {};
    const radius = c.radius || pack?.style?.radius;
    if (radius && radius !== 'rounded') style.radius = radius;
    const glass = c.glass !== undefined ? c.glass : pack?.style?.glass;
    if (glass) style.glass = true;
    const dim = typeof c.wallpaperDim === 'number' ? c.wallpaperDim : pack?.style?.wallpaperDim;
    if (typeof dim === 'number') style.wallpaperDim = dim;
    const blur = typeof c.wallpaperBlur === 'number' ? c.wallpaperBlur : pack?.style?.wallpaperBlur;
    if (typeof blur === 'number') style.wallpaperBlur = blur;
    const cw = c.coverWall !== undefined ? c.coverWall : pack?.style?.coverWall;
    if (cw === false) style.coverWall = false;
    const ss = soundStyle();
    if (ss) style.soundStyle = ss;

    const fontFamily = c.fontName ? c.fontName.replace(/\.[^.]+$/, '') : 'CustomFont';
    return {
      name: String(name || '').trim(),
      author: 'Custom',
      base,
      colors,
      style: Object.keys(style).length ? style : null,
      wallpaperRel: c.wallpaper || null,
      fontRel: c.fontFile || null,
      fontFamily: c.fontFile ? fontFamily : null,
      musicRel: c.music || null,
    };
  }

  // Determine the currently highlighted/open console id for per-console wallpapers.
  function currentSystemId() {
    if (S.screen === 'games' && typeof curList === 'function') {
      const cl = curList();
      return cl?.tile?.id || null;
    }
    if (S.screen === 'detail' && S.game && typeof findGame === 'function') {
      const f = findGame(S.game);
      return f?.s?.id || null;
    }
    if (S.screen === 'stream' || S.screen === 'sdetail' || S.screen === 'sperson') return 'streaming';
    if (S.screen === 'home') {
      if (tvOn() && S.tv) {
        if (S.tv.sec === 'watch') return 'movies';
        return S.tv.con || (typeof tvState === 'function' ? tvState().con?.id : null) || null;
      }
      if (waveOn() && S.wave?.cat) return S.wave.cat;
      if (bladesOn() && S.blade?.cat) return S.blade.cat;
      if (cubeOn() && typeof cubeState === 'function') {
        const cs = cubeState();
        return cs?.belt?.[S.cube?.i || 0]?.id || null;
      }
      const T = typeof tiles === 'function' ? tiles() : [];
      const t = T[Math.min(S.sel || 0, Math.max(0, T.length - 1))];
      return t?.id || null;
    }
    return null;
  }

  let lastWp = '';
  function syncWallpaper(explicitSysId) {
    const c = custom();
    const pack = activePack();
    const wpMap = pack?.wallpapers || {};
    const sysId = explicitSysId !== undefined ? explicitSysId : currentSystemId();
    const rel = c.wallpaper || (sysId && wpMap[sysId]) || wpMap.default || null;
    const layer = ensureWpLayer();
    const root = document.documentElement;
    if (!rel) {
      if (lastWp) {
        lastWp = '';
        layer.classList.remove('on');
        layer.querySelector('.tbg-img').style.backgroundImage = '';
        root.classList.remove('has-theme-wp');
        root.style.removeProperty('--theme-wp');
      }
      return;
    }
    const url = mediaUrl(rel);
    if (url !== lastWp) {
      lastWp = url;
      layer.querySelector('.tbg-img').style.backgroundImage = `url('${url.replace(/'/g, "\\'")}')`;
      root.style.setProperty('--theme-wp', `url('${url.replace(/'/g, "\\'")}')`);
    }
    layer.classList.add('on');
    root.classList.add('has-theme-wp');
  }

  // Custom console icon lookup.
  function iconFor(id) {
    const pack = iconPack();
    const rel = pack?.icons?.[id];
    if (!rel) return null;
    return { url: mediaUrl(rel), mode: pack.icon_mode === 'mono' ? 'mono' : 'color' };
  }

  // Custom Home music lookup (Simple Mode custom music -> active/overridden Music Pack).
  function musicFor(styleName) {
    const c = custom();
    const pref = settings().musicPack || '';
    if (c.music && (!pref || pref === '')) return mediaUrl(c.music);
    const pack = musicPack();
    if (!pack) return null;
    const rel = pack.music?.[styleName] || pack.music?.default || null;
    return rel ? mediaUrl(rel) : null;
  }

  // Custom UI sound effect playback via Web Audio (works identically on Windows & Linux).
  function playSound(name, ctx, master) {
    const pack = soundPack();
    const rel = pack?.sounds?.[name];
    if (!rel || !ctx || !master) return false;
    const url = mediaUrl(rel);
    const cached = sfxCache.get(url);
    if (cached && cached !== 'loading' && cached !== 'err') {
      try {
        const src = ctx.createBufferSource();
        src.buffer = cached;
        src.connect(master);
        src.start(0);
        return true;
      } catch (e) {
        return false;
      }
    }
    if (cached === 'err') return false;
    if (!cached) {
      sfxCache.set(url, 'loading');
      fetch(url)
        .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject()))
        .then((buf) => ctx.decodeAudioData(buf))
        .then((decoded) => { sfxCache.set(url, decoded); })
        .catch(() => { sfxCache.set(url, 'err'); });
    }
    return false;
  }

  function preloadSounds(ctx) {
    const pack = soundPack();
    if (!pack || !ctx) return;
    for (const rel of Object.values(pack.sounds || {})) {
      const url = mediaUrl(rel);
      if (sfxCache.has(url)) continue;
      sfxCache.set(url, 'loading');
      fetch(url)
        .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject()))
        .then((buf) => ctx.decodeAudioData(buf))
        .then((decoded) => sfxCache.set(url, decoded))
        .catch(() => sfxCache.set(url, 'err'));
    }
  }

  function mascot() {
    const pack = mascotPack();
    return pack?.mascot || null;
  }

  // Safe reset: immediately restore default theme & clear Simple Mode + pack overrides.
  async function safeReset() {
    const tv = tvOn();
    const key = tv ? 'tvTheme' : 'theme';
    const def = tv ? 'tv' : 'cream';
    const ck = customKey();
    S.boot.settings = {
      ...settings(),
      [key]: def,
      [ck]: {},
      iconPack: '',
      musicPack: '',
      soundPack: '',
      mascotPack: '',
    };
    apply();
    render();
    try {
      await api.call('set_setting', { key, value: def });
      await api.call('set_setting', { key: ck, value: {} });
      for (const k of ['iconPack', 'musicPack', 'soundPack', 'mascotPack']) {
        await api.call('set_setting', { key: k, value: '' });
      }
    } catch (e) { /* best effort */ }
    if (typeof toast === 'function') toast('Theme reset to default (Ctrl+Shift+R)', 4000);
  }

  function clearCache() {
    sfxCache.clear();
    lastWp = '';
  }

  return {
    PALETTES,
    SWATCHES,
    FONTS,
    list,
    byId,
    activeId,
    activePack,
    customKey,
    customSlot,
    custom,
    hasCustom,
    isHex,
    effectiveCoreColors,
    derivePalette,
    buildCustomThemeSpec,
    iconPack,
    musicPack,
    soundPack,
    mascotPack,
    soundStyle,
    optionValue,
    apply,
    syncWallpaper,
    iconFor,
    musicFor,
    playSound,
    preloadSounds,
    mascot,
    safeReset,
    clearCache,
  };
})();
