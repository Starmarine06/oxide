// Page bridge and plugin runtime. Runs before YouTube Music's own scripts
// on every load. Reports player state to Rust, executes commands sent from
// Rust, and starts/stops page plugins as their settings change.
//
// The page enforces Trusted Types, so never assign HTML strings (innerHTML
// etc.); build DOM nodes with pear.el() instead.
(function () {
  if (location.hostname !== 'music.youtube.com' || window.__pear) return;

  const invoke = (cmd, args) => {
    const ipc = window.__TAURI_INTERNALS__;
    if (!ipc) return Promise.reject(new Error('IPC unavailable'));
    return ipc.invoke(cmd, args);
  };

  // ---- events --------------------------------------------------------------

  const listeners = new Map();
  const on = (name, fn) => {
    if (!listeners.has(name)) listeners.set(name, new Set());
    listeners.get(name).add(fn);
    return () => listeners.get(name).delete(fn);
  };
  const emit = (name, payload) => {
    for (const fn of [...(listeners.get(name) || [])]) {
      try { fn(payload); } catch (e) { console.error('[oxide]', name, e); }
    }
  };

  // YouTube Music asks "Leave site?" on reload or navigation while playing.
  // Pear Desktop (Electron) never shows that prompt, so drop those handlers.
  const addListener = EventTarget.prototype.addEventListener;
  EventTarget.prototype.addEventListener = function (type, ...rest) {
    if (type === 'beforeunload' && this === window) return;
    return addListener.call(this, type, ...rest);
  };
  try {
    Object.defineProperty(window, 'onbeforeunload', { configurable: true, get: () => null, set: () => {} });
  } catch (_) {}

  const playerApi = () => document.querySelector('#movie_player');
  // The song's own element. Pages can hold other <video>s (hover previews,
  // animated thumbnails) that come first in the DOM; reading their time and
  // length throws the lyrics, media controls and plugins off.
  const video = () =>
    document.querySelector('#movie_player video.html5-main-video') ||
    document.querySelector('#movie_player video') ||
    document.querySelector('ytmusic-player video') ||
    document.querySelector('video');

  // Song-relative time. YouTube Music can keep one media timeline running
  // across tracks (gapless playback), so the <video>'s own currentTime and
  // duration count from the start of the session, not the song. The player
  // API counts from the start of the current song.
  const position = () => {
    const t = playerApi()?.getCurrentTime?.();
    if (Number.isFinite(t)) return t;
    return video()?.currentTime || 0;
  };
  const duration = () => {
    const d = playerApi()?.getDuration?.();
    if (Number.isFinite(d) && d > 0) return d;
    const v = video();
    return v && Number.isFinite(v.duration) ? v.duration : 0;
  };
  const seekTo = (s) => {
    const api = playerApi();
    if (api?.seekTo) api.seekTo(Math.max(0, s), true);
    else { const v = video(); if (v) v.currentTime = Math.max(0, s); }
  };

  // ---- selectors -----------------------------------------------------------
  // YouTube Music ships two player UIs: the 2026 "miniplayer" redesign and
  // the older ytmusic-player-bar. Every selector lists both.
  const SEL = {
    bar: 'ytmusic-miniplayer, ytmusic-player-bar',
    buttonHost: '.ytMusicMiniPlayerRightSection, ytmusic-player-bar .middle-controls-buttons',
    playPause: '.ytmusicPlayerControlsPlayPauseButton button, #play-pause-button',
    next: '.ytmusicPlayerControlsNextButton button, ytmusic-player-bar .next-button',
    previous: '.ytmusicPlayerControlsPreviousButton button, ytmusic-player-bar .previous-button',
    shuffle: '.ytmusicPlayerControlsShuffleButton button, ytmusic-player-bar .shuffle',
    repeat: '.ytmusicPlayerControlsRepeatButton button, ytmusic-player-bar .repeat',
    like: 'ytmusic-miniplayer like-button-view-model button, ytmusic-player-bar #like-button-renderer #button-shape-like button',
    dislike: 'ytmusic-miniplayer dislike-button-view-model button, ytmusic-player-bar #like-button-renderer #button-shape-dislike button',
    volume: '.ytMusicMiniPlayerVolumePopup input#slider, #volume-slider',
  };
  const $ = (key) => document.querySelector(SEL[key]);

  const likeState = () => {
    const old = document.querySelector('ytmusic-player-bar #like-button-renderer');
    if (old) return old.getAttribute('like-status') || 'INDIFFERENT';
    if ($('like')?.getAttribute('aria-pressed') === 'true') return 'LIKE';
    if ($('dislike')?.getAttribute('aria-pressed') === 'true') return 'DISLIKE';
    return 'INDIFFERENT';
  };

  const playerPageOpen = () => document.querySelector('ytmusic-app-layout')?.getAttribute('player-ui-state') === 'PLAYER_PAGE_OPEN';
  // Open (true), close (false) or toggle (undefined) the full player page.
  const playerPage = (open) => {
    if (open !== undefined && open === playerPageOpen()) return;
    const old = document.querySelector('.toggle-player-page-button');
    if (old) { old.click(); return; }
    const want = (open ?? !playerPageOpen()) ? /open player page/i : /close player page|open mini player/i;
    const btn = [...document.querySelectorAll('ytmusic-miniplayer button, ytmusic-player-page button')].find((b) => want.test(b.getAttribute('aria-label') || ''));
    if (btn) btn.click();
    else document.querySelector('ytmusic-miniplayer ytmusic-track-info')?.click();
  };

  // ---- commands from Rust ------------------------------------------------

  const commands = {
    play: () => video()?.play(),
    pause: () => video()?.pause(),
    toggle: () => {
      const btn = $('playPause');
      if (btn) btn.click();
      else { const v = video(); if (v) v.paused ? v.play() : v.pause(); }
    },
    next: () => {
      const api = playerApi();
      if (api && api.nextVideo) api.nextVideo();
      else $('next')?.click();
    },
    previous: () => {
      const api = playerApi();
      if (api && api.previousVideo) api.previousVideo();
      else $('previous')?.click();
    },
    seek: (s) => seekTo(s),
    seekBy: (d) => seekTo(position() + d),
    volume: (f) => playerApi()?.setVolume?.(Math.round(f * 100)),
    volumeBy: (d) => {
      const api = playerApi();
      if (api && api.getVolume) api.setVolume(Math.min(100, Math.max(0, api.getVolume() + d)));
    },
    mute: () => {
      const api = playerApi();
      if (api) api.isMuted() ? api.unMute() : api.mute();
    },
    like: () => $('like')?.click(),
    dislike: () => $('dislike')?.click(),
    shuffle: () => $('shuffle')?.click(),
    repeat: () => $('repeat')?.click(),
    search: () => {
      const box = document.querySelector('ytmusic-search-box');
      box?.click();
      box?.querySelector('input')?.focus();
    },
    playerPage: () => playerPage(),
    back: () => history.back(),
    forward: () => history.forward(),
    home: () => { location.href = 'https://music.youtube.com/'; },
    navigate: (url) => {
      if (typeof url === 'string' && url.startsWith('https://music.youtube.com/')) location.href = url;
    },
  };

  const command = (name, arg) => {
    if (commands[name]) {
      try { commands[name](arg); } catch (e) { console.error('[oxide] command', name, e); }
    } else {
      emit('command:' + name, arg);
    }
  };

  // ---- song info ---------------------------------------------------------

  let song = null;
  let pendingId = null;
  let pendingSince = 0;

  function read() {
    const api = playerApi();
    const v = video();
    if (!api || !api.getVideoData || !v) return null;
    if (api.classList.contains('ad-showing')) return null;
    const data = api.getVideoData();
    if (!data || !data.video_id) return null;

    const md = navigator.mediaSession && navigator.mediaSession.metadata;
    // mediaSession lags a moment behind a track change; only trust it once
    // its title matches the player's, or after 3 seconds.
    const mdFresh = md && md.title && md.title === data.title;
    if (!mdFresh) {
      if (pendingId !== data.video_id) { pendingId = data.video_id; pendingSince = Date.now(); }
      if (Date.now() - pendingSince < 3000) return null;
    }
    const art = mdFresh && md.artwork && md.artwork.length
      ? md.artwork[md.artwork.length - 1].src
      : `https://i.ytimg.com/vi/${data.video_id}/hqdefault.jpg`;

    return {
      videoId: data.video_id,
      title: (mdFresh && md.title) || data.title || '',
      artist: (mdFresh && md.artist) || data.author || '',
      album: (mdFresh && md.album) || null,
      artUrl: art,
      duration: duration(),
      elapsed: position(),
      paused: v.paused,
      url: `https://music.youtube.com/watch?v=${data.video_id}`,
    };
  }

  function tick() {
    const s = read();
    if (!s) return;
    const changed = !song || song.videoId !== s.videoId;
    song = s;
    if (changed) emit('song', s);
    emit('time', s);
    invoke('player_update', { state: s }).catch(() => {});
  }

  setInterval(tick, 1000);
  for (const ev of ['play', 'pause', 'seeked', 'loadedmetadata']) {
    document.addEventListener(ev, (e) => { if (e.target instanceof HTMLMediaElement) setTimeout(tick, 50); }, true);
  }

  // ---- DOM helpers -------------------------------------------------------

  const el = (tag, props = {}, children = []) => {
    const node = document.createElement(tag);
    for (const [k, v] of Object.entries(props)) {
      if (v === undefined || v === null) continue;
      if (k === 'style') Object.assign(node.style, v);
      else if (k.startsWith('on') && typeof v === 'function') node.addEventListener(k.slice(2), v);
      else if (k === 'text') node.textContent = v;
      else node.setAttribute(k, v);
    }
    for (const c of children) if (c !== null && c !== undefined) node.append(c);
    return node;
  };

  const svgIcon = (pathD, size = 24) => {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('fill', 'currentColor');
    const path = document.createElementNS(ns, 'path');
    path.setAttribute('d', pathD);
    svg.append(path);
    return svg;
  };

  const whenReady = (fn) => {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', fn, { once: true });
    else fn();
  };

  // This script can run before <html> exists, so styles may need to wait.
  const addStyle = (css) => {
    const style = document.createElement('style');
    style.textContent = css;
    const root = document.head || document.documentElement;
    if (root) root.append(style);
    else whenReady(() => (document.head || document.documentElement).append(style));
    return style;
  };

  // Calls `fn(node)` for every node matching `selector`, now and as YouTube
  // Music renders new ones. Returns a function that stops watching.
  const watch = (selector, fn) => {
    const seen = new WeakSet();
    let observer = null;
    let stopped = false;
    const check = () => {
      for (const node of document.querySelectorAll(selector)) {
        if (!seen.has(node)) { seen.add(node); try { fn(node); } catch (e) { console.error('[oxide] watch', e); } }
      }
    };
    whenReady(() => {
      if (stopped) return;
      check();
      observer = new MutationObserver(check);
      observer.observe(document.documentElement, { childList: true, subtree: true });
    });
    return () => { stopped = true; observer?.disconnect(); };
  };

  // Resolves with the first node matching `selector`.
  const waitFor = (selector) => new Promise((resolve) => {
    const stop = watch(selector, (node) => { stop(); resolve(node); });
  });

  // Small toast in the bottom-left corner.
  let toastEl = null;
  let toastTimer = 0;
  const toast = (text, ms = 3000) => {
    whenReady(() => {
      if (!toastEl) {
        toastEl = el('div', { class: 'oxide-toast', role: 'status' });
        document.body.append(toastEl);
      }
      toastEl.textContent = text;
      toastEl.classList.add('show');
      clearTimeout(toastTimer);
      if (ms > 0) toastTimer = setTimeout(() => toastEl.classList.remove('show'), ms);
    });
  };

  addStyle(`
    .oxide-toast { position: fixed; left: 16px; bottom: 88px; z-index: 9999; max-width: 360px;
      padding: 10px 14px; border-radius: 8px; background: #212121; color: #fff; font: 14px Roboto, sans-serif;
      box-shadow: 0 4px 16px rgba(0,0,0,.5); opacity: 0; transform: translateY(8px);
      transition: opacity .2s, transform .2s; pointer-events: none; }
    .oxide-toast.show { opacity: 1; transform: none; }
    .oxide-btn { display: inline-flex; align-items: center; justify-content: center; width: 40px; height: 40px;
      margin: 0 2px; padding: 0; border: 0; border-radius: 50%; background: transparent; cursor: pointer;
      color: var(--ytmusic-text-secondary, #aaa); flex: none; }
    .oxide-btn:hover { color: var(--ytmusic-text-primary, #fff); background: rgba(255,255,255,.1); }
    .oxide-btn.active { color: var(--ytmusic-text-primary, #fff); }
    .oxide-btn:focus-visible { outline: 2px solid #fff; outline-offset: 1px; }
    .ytMusicMiniPlayerRightSection { display: flex; align-items: center; }
    .ytMusicMiniPlayerRightSection > .oxide-btn { width: 36px; height: 36px; }
    .oxide-btn svg { display: block; }
  `);

  // ---- plugin runtime ----------------------------------------------------
  //
  // pear.plugin(id, {
  //   start(ctx, config) {},   // required
  //   stop() {},               // optional; ctx resources are cleaned up anyway
  //   onConfig(after, before) {}, // optional; return 'restart' to restart.
  //                            // Without it the plugin always restarts.
  //   reloadOnToggle: false,   // true for plugins that patch the page at load
  // })

  const STORE_KEY = '__oxide_config';
  let config = window.__PEAR_CONFIG__ || { rev: 0, plugins: {} };
  try {
    const saved = JSON.parse(localStorage.getItem(STORE_KEY) || 'null');
    if (saved && saved.rev > config.rev) config = saved;
  } catch (_) {}

  const plugins = new Map();
  const pluginConfig = (id) => (config.plugins && config.plugins[id]) || {};

  function makeContext(id) {
    const cleanups = [];
    const add = (fn) => { cleanups.push(fn); return fn; };
    const ctx = {
      id,
      cleanup: add,
      style: (css) => { const s = addStyle(css); add(() => s.remove()); return s; },
      interval: (fn, ms) => { const h = setInterval(fn, ms); add(() => clearInterval(h)); return h; },
      timeout: (fn, ms) => { const h = setTimeout(fn, ms); add(() => clearTimeout(h)); return h; },
      listen: (target, type, fn, opts) => {
        target.addEventListener(type, fn, opts);
        add(() => target.removeEventListener(type, fn, opts));
      },
      on: (name, fn) => add(on(name, fn)),
      command: (name, fn) => add(on('command:' + name, fn)),
      watch: (selector, fn) => add(watch(selector, fn)),
      observe: (node, options, fn) => {
        const o = new MutationObserver(fn);
        o.observe(node, options);
        add(() => o.disconnect());
        return o;
      },
      // Adds a node and removes it again on stop.
      mount: (node, parent, prepend = false) => {
        // Resolve the parent late: the script can run before <body> exists.
        whenReady(() => { const p = parent || document.body; prepend ? p.prepend(node) : p.append(node); });
        add(() => node.remove());
        return node;
      },
      // Button next to like/dislike in the player bar (always visible).
      playerButton: (title, iconPath, onClick) => {
        const btn = el('button', { class: 'oxide-btn', title, 'aria-label': title, 'data-oxide': id, onclick: onClick }, [svgIcon(iconPath)]);
        add(watch(SEL.buttonHost, (host) => host.prepend(btn)));
        add(() => btn.remove());
        return btn;
      },
      saveOption: (key, value) => invoke('plugin_set_option', { id, key, value }),
    };
    return { ctx, run: () => { while (cleanups.length) { try { cleanups.pop()(); } catch (e) { console.error(e); } } } };
  }

  function startPlugin(id) {
    const p = plugins.get(id);
    if (!p || p.running) return;
    const { ctx, run } = makeContext(id);
    p.cleanup = run;
    p.running = true;
    try { p.def.start(ctx, pluginConfig(id)); } catch (e) { console.error('[oxide] start', id, e); }
  }

  function stopPlugin(id) {
    const p = plugins.get(id);
    if (!p || !p.running) return;
    p.running = false;
    try { p.def.stop?.(); } catch (e) { console.error('[oxide] stop', id, e); }
    p.cleanup?.();
  }

  const plugin = (id, def) => {
    plugins.set(id, { def, running: false });
    if (pluginConfig(id).enabled) startPlugin(id);
  };

  const setConfig = (next) => {
    const prev = config;
    config = next;
    try { localStorage.setItem(STORE_KEY, JSON.stringify(next)); } catch (_) {}
    for (const [id, p] of plugins) {
      const before = (prev.plugins && prev.plugins[id]) || {};
      const after = pluginConfig(id);
      if (JSON.stringify(before) === JSON.stringify(after)) continue;
      if (!!before.enabled !== !!after.enabled && p.def.reloadOnToggle) { location.reload(); return; }
      if (!after.enabled) stopPlugin(id);
      else if (!p.running) startPlugin(id);
      else if (p.def.onConfig) {
        // onConfig(after, before) may return 'restart' when a change needs it.
        let r = null;
        try { r = p.def.onConfig(after, before); } catch (e) { console.error(e); }
        if (r === 'restart') { stopPlugin(id); startPlugin(id); }
      }
      else { stopPlugin(id); startPlugin(id); }
    }
    emit('config', config);
  };

  // ---- shared audio graph ------------------------------------------------
  //
  // A media element can only be routed into Web Audio once, so every audio
  // plugin (equalizer, compressor, crossfade, visualizer, skip silences)
  // shares this graph:  source -> filters (by order) -> master gain -> out
  //                                                        \-> analyser

  const audio = (() => {
    let ctx = null, source = null, master = null, analyser = null, element = null;
    const filters = new Map(); // id -> { order, input, output }

    function rewire() {
      if (!source) return;
      source.disconnect();
      for (const f of filters.values()) { try { f.output.disconnect(); } catch (_) {} }
      let node = source;
      for (const f of [...filters.values()].sort((a, b) => a.order - b.order)) {
        node.connect(f.input);
        node = f.output;
      }
      node.connect(master);
    }

    function ensure() {
      const v = video();
      if (!v) return null;
      if (ctx && element === v) return ctx;
      ctx = ctx || new AudioContext({ latencyHint: 'playback' });
      element = v;
      source = ctx.createMediaElementSource(v);
      master = master || ctx.createGain();
      analyser = analyser || ctx.createAnalyser();
      analyser.fftSize = 4096;
      analyser.smoothingTimeConstant = 0.72;
      master.disconnect();
      master.connect(ctx.destination);
      master.connect(analyser);
      rewire();
      const resume = () => { if (ctx.state === 'suspended') ctx.resume().catch(() => {}); };
      v.addEventListener('play', resume);
      resume();
      return ctx;
    }

    // Resolves once the player's <video> exists and the graph is built.
    const ready = () => new Promise((resolve) => {
      const tryIt = () => { const c = ensure(); if (c) resolve(c); else setTimeout(tryIt, 300); };
      whenReady(tryIt);
    });

    return {
      ready,
      get context() { return ctx; },
      get master() { return master; },
      get analyser() { return analyser; },
      // Insert a processing stage. Lower `order` runs earlier.
      addFilter(id, order, input, output = input) { filters.set(id, { order, input, output }); rewire(); },
      removeFilter(id) {
        const f = filters.get(id);
        if (!f) return;
        filters.delete(id);
        try { f.output.disconnect(); } catch (_) {}
        rewire();
      },
    };
  })();

  // ---- Rust <-> page round trips -----------------------------------------

  // Rust asks the page for something (`pear.answer`) and waits for the reply.
  const queries = {
    'like-state': () => likeState(),
    'volume': () => ({ state: playerApi()?.getVolume?.() ?? 0, isMuted: !!playerApi()?.isMuted?.() }),
    'shuffle': () => ({ state: $('shuffle')?.getAttribute('aria-pressed') === 'true' }),
    'repeat-mode': () => {
      const old = document.querySelector('ytmusic-player-bar')?.getAttribute('repeat-mode');
      if (old) return { mode: old };
      const label = ($('repeat')?.getAttribute('aria-label') || '').toLowerCase();
      return { mode: label.includes('one') ? 'ONE' : label.includes('all') ? 'ALL' : 'NONE' };
    },
    'fullscreen': () => ({ state: playerPageOpen() }),
    'queue': () => {
      const items = [...document.querySelectorAll('ytmusic-player-queue-item')];
      return {
        items: items.map((it, i) => ({
          index: i,
          title: it.querySelector('.song-title')?.textContent?.trim() || '',
          artist: it.querySelector('.byline')?.textContent?.trim() || '',
          duration: it.querySelector('.duration')?.textContent?.trim() || '',
          selected: it.hasAttribute('selected'),
        })),
      };
    },
  };
  const answer = async (id, name, arg) => {
    let value = null, error = null;
    try {
      const fn = queries[name];
      if (!fn) throw new Error('unknown query ' + name);
      value = await fn(arg);
    } catch (e) { error = String(e); }
    invoke('page_reply', { id, value: value ?? null, error }).catch(() => {});
  };

  // Small modal for text/number input (Rust uses it via the `prompt` query).
  const prompt = ({ title, label, value = '', type = 'text', placeholder = '' }) => new Promise((resolve) => {
    whenReady(() => {
      const input = el('input', { class: 'oxide-prompt-input', type, value: String(value ?? ''), placeholder, 'aria-label': label || title });
      const done = (result) => { overlay.remove(); resolve(result); };
      const ok = el('button', { class: 'oxide-prompt-ok', text: 'Save', onclick: () => done(input.value) });
      const cancel = el('button', { class: 'oxide-prompt-cancel', text: 'Cancel', onclick: () => done(null) });
      const box = el('form', { class: 'oxide-prompt', onsubmit: (e) => { e.preventDefault(); done(input.value); } }, [
        el('h2', { text: title }),
        label ? el('p', { text: label }) : null,
        input,
        el('div', { class: 'oxide-prompt-actions' }, [cancel, ok]),
      ]);
      const overlay = el('div', { class: 'oxide-prompt-overlay', onmousedown: (e) => { if (e.target === overlay) done(null); } }, [box]);
      overlay.addEventListener('keydown', (e) => { if (e.key === 'Escape') done(null); });
      document.body.append(overlay);
      input.focus();
      input.select();
    });
  });
  queries.prompt = prompt;

  addStyle(`
    .oxide-prompt-overlay { position: fixed; inset: 0; z-index: 2147483100; display: grid; place-items: center;
      background: rgba(0,0,0,.55); backdrop-filter: blur(2px); }
    .oxide-prompt { width: min(420px, calc(100vw - 32px)); padding: 22px; border-radius: 14px; background: #1c1c1c;
      border: 1px solid rgba(255,255,255,.08); box-shadow: 0 20px 50px rgba(0,0,0,.6); color: #eee; font: 14px Roboto, sans-serif; }
    .oxide-prompt h2 { margin: 0 0 6px; font-size: 17px; font-weight: 500; }
    .oxide-prompt p { margin: 0 0 14px; color: #aaa; line-height: 1.45; }
    .oxide-prompt-input { width: 100%; box-sizing: border-box; padding: 10px 12px; border-radius: 8px; border: 1px solid #3a3a3a;
      background: #111; color: #fff; font: inherit; outline: none; }
    .oxide-prompt-input:focus { border-color: #e8642c; }
    .oxide-prompt-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
    .oxide-prompt-actions button { padding: 8px 16px; border-radius: 18px; border: 0; font: 500 14px Roboto, sans-serif; cursor: pointer; }
    .oxide-prompt-cancel { background: transparent; color: #ddd; }
    .oxide-prompt-cancel:hover { background: rgba(255,255,255,.08); }
    .oxide-prompt-ok { background: #fff; color: #000; }
  `);

  // ---- popover (for player-bar buttons) ----------------------------------

  let openPopover = null;
  const closePopover = () => { openPopover?.remove(); openPopover = null; };
  // Shows `content` in a card above (or below) `anchor`. Clicking outside closes it.
  const popover = (anchor, content, { title } = {}) => {
    const wasOpenHere = openPopover && openPopover.__anchor === anchor;
    closePopover();
    if (wasOpenHere) return null;
    const card = el('div', { class: 'oxide-popover', role: 'dialog', 'aria-label': title || '' }, [
      title ? el('div', { class: 'oxide-popover-title', text: title }) : null,
      content,
    ]);
    card.__anchor = anchor;
    document.body.append(card);
    const r = anchor.getBoundingClientRect();
    const w = card.offsetWidth, h = card.offsetHeight;
    const left = Math.min(Math.max(8, r.left + r.width / 2 - w / 2), innerWidth - w - 8);
    const above = r.top - h - 10;
    card.style.left = left + 'px';
    card.style.top = (above > 40 ? above : r.bottom + 10) + 'px';
    openPopover = card;
    return card;
  };
  document.addEventListener('mousedown', (e) => {
    if (openPopover && !openPopover.contains(e.target) && !openPopover.__anchor.contains(e.target)) closePopover();
  }, true);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closePopover(); });

  addStyle(`
    .oxide-popover { position: fixed; z-index: 2147483050; min-width: 200px; padding: 14px 16px; border-radius: 12px;
      background: #212121; border: 1px solid rgba(255,255,255,.08); box-shadow: 0 14px 36px rgba(0,0,0,.55);
      color: #eee; font: 14px Roboto, sans-serif; animation: oxide-pop .14s cubic-bezier(.2,.8,.3,1); }
    @keyframes oxide-pop { from { opacity: 0; transform: translateY(6px); } }
    .oxide-popover-title { font-size: 12px; letter-spacing: .06em; text-transform: uppercase; color: #9a9a9a; margin-bottom: 10px; }
    .oxide-range { -webkit-appearance: none; appearance: none; width: 100%; height: 4px; border-radius: 2px;
      background: linear-gradient(to right, #fff var(--p, 50%), rgba(255,255,255,.2) var(--p, 50%)); outline: none; margin: 10px 0; }
    .oxide-range::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 14px; border-radius: 50%; background: #fff; cursor: pointer;
      box-shadow: 0 1px 4px rgba(0,0,0,.5); }
    .oxide-chips { display: flex; flex-wrap: wrap; gap: 6px; }
    .oxide-chip { padding: 6px 12px; border-radius: 16px; border: 0; background: rgba(255,255,255,.1); color: #fff;
      font: 500 13px Roboto, sans-serif; cursor: pointer; }
    .oxide-chip:hover { background: rgba(255,255,255,.18); }
    .oxide-chip.on { background: #fff; color: #000; }
    .oxide-list { display: flex; flex-direction: column; gap: 2px; max-height: 50vh; overflow-y: auto; }
    .oxide-list button { text-align: left; padding: 9px 10px; border: 0; border-radius: 8px; background: none; color: #eee;
      font: 14px Roboto, sans-serif; cursor: pointer; display: flex; justify-content: space-between; gap: 16px; }
    .oxide-list button:hover { background: rgba(255,255,255,.08); }
    .oxide-list button.on::after { content: '✓'; color: #e8642c; }
  `);

  // Range input with a filled track.
  const range = ({ min, max, step, value, oninput }) => {
    const r = el('input', { class: 'oxide-range', type: 'range', min, max, step, value });
    const paint = () => r.style.setProperty('--p', ((r.value - min) / (max - min)) * 100 + '%');
    r.addEventListener('input', () => { paint(); oninput(Number(r.value)); });
    paint();
    return r;
  };

  // Dominant colours of an image, most vibrant first. Falls back to Oxide
  // orange if the image can't be read.
  const paletteCache = new Map();
  const palette = (url) => {
    // Placeholder data: images carry no colour; use the current song's art.
    if (!url || url.startsWith('data:')) url = song?.artUrl;
    if (!url) return Promise.resolve([[232, 100, 44], [196, 71, 31], [120, 40, 20]]);
    if (paletteCache.has(url)) return paletteCache.get(url);
    const p = new Promise((resolve) => {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        try {
          const c = document.createElement('canvas');
          c.width = c.height = 32;
          const g = c.getContext('2d', { willReadFrequently: true });
          g.drawImage(img, 0, 0, 32, 32);
          const d = g.getImageData(0, 0, 32, 32).data;
          const buckets = new Map();
          for (let i = 0; i < d.length; i += 4) {
            const key = (d[i] >> 5) << 6 | (d[i + 1] >> 5) << 3 | (d[i + 2] >> 5);
            const b = buckets.get(key) || { r: 0, g: 0, b: 0, n: 0 };
            b.r += d[i]; b.g += d[i + 1]; b.b += d[i + 2]; b.n++;
            buckets.set(key, b);
          }
          const colors = [...buckets.values()].map((b) => {
            const r = b.r / b.n, gg = b.g / b.n, bb = b.b / b.n;
            const max = Math.max(r, gg, bb), min = Math.min(r, gg, bb);
            const sat = max ? (max - min) / max : 0;
            return { rgb: [r | 0, gg | 0, bb | 0], max, score: b.n * (0.3 + sat) * (max > 60 ? 1 : 0.05) };
          }).filter((c) => c.max > 35).sort((a, b) => b.score - a.score).map((c) => c.rgb);
          resolve(colors.length ? colors.slice(0, 6) : [[232, 100, 44]]);
        } catch (_) { resolve([[232, 100, 44], [196, 71, 31], [120, 40, 20]]); }
      };
      img.onerror = () => resolve([[232, 100, 44], [196, 71, 31], [120, 40, 20]]);
      img.src = url;
    });
    paletteCache.set(url, p);
    return p;
  };

  window.__pear = {
    invoke, on, emit, command, toast, el, svgIcon, addStyle, whenReady, watch, waitFor,
    plugin, setConfig, audio, answer, prompt, queries, popover, closePopover, range, palette,
    sel: SEL, $, likeState, playerPage, playerPageOpen,
    ready: () => emit('ready'),
    config: (id) => pluginConfig(id),
    get api() { return playerApi(); },
    get song() { return song; },
    get video() { return video(); },
    position, duration, seekTo,
  };
})();
