// Synced lyrics panel. Lyrics come from Rust (`lyrics_get`, LRCLIB).
// Toggle with the player-bar button or Plugins > Synced lyrics > Show / hide (Ctrl+L).
pear.plugin('synced-lyrics', {
  start(ctx, config) {
    const offset = () => Number(config.offsetMs) || 0;
    const STORE_KEY = 'oxide-lyrics-open';
    const MIN_SIZE = 18, MAX_SIZE = 40;
    let fontSize = Math.min(MAX_SIZE, Math.max(MIN_SIZE, Number(config.fontSize) || 26));
    const MIN_W = 300;
    const maxW = () => Math.max(MIN_W, Math.min(960, innerWidth - 140));
    let width = Number(config.width) || 0; // 0 = automatic
    let open = false;
    try { open = localStorage.getItem(STORE_KEY) === '1'; } catch (_) {}

    let lines = [];          // [{ time, text, gap, node, words: [{ node, from, to }] }]
    let currentIndex = -1;
    let requestId = 0;
    let userScrollUntil = 0; // auto-scroll pauses while the user scrolls
    let raf = 0;

    ctx.style(`
      .oxl { position: fixed; z-index: 1000; right: 12px; width: var(--oxl-w, min(440px, 42vw));
        top: calc(var(--menu-bar-height, 0px) + var(--ytmusic-nav-bar-height, 64px) + 12px);
        bottom: calc(var(--ytmusic-player-bar-height, 72px) + 12px);
        display: none; flex-direction: column; overflow: hidden; isolation: isolate;
        border-radius: 18px; border: 1px solid rgba(255,255,255,.08);
        box-shadow: 0 24px 60px rgba(0,0,0,.55), 0 2px 8px rgba(0,0,0,.35);
        background: #0d0d0d; color: #fff; font-family: "YouTube Sans", "Roboto", system-ui, sans-serif;
        --oxl-size: 26px; }
      .oxl.open { display: flex; animation: oxl-in .28s cubic-bezier(.2,.8,.2,1); }
      @keyframes oxl-in { from { opacity: 0; transform: translateX(16px) scale(.985); } }

      /* Album art, blurred, as the panel's backdrop. */
      .oxl-bg { position: absolute; inset: -20%; z-index: -2; background-size: cover; background-position: center;
        filter: blur(56px) saturate(1.5) brightness(.75); transform: scale(1.1); transition: background-image .6s; }
      .oxl::before { content: ''; position: absolute; inset: 0; z-index: -1;
        background: linear-gradient(180deg, rgba(8,8,8,.55) 0%, rgba(8,8,8,.72) 55%, rgba(8,8,8,.86) 100%); }

      .oxl-head { display: flex; align-items: center; gap: 12px; padding: 14px 14px 12px 16px; }
      .oxl-art { width: 44px; height: 44px; flex: none; border-radius: 8px; object-fit: cover;
        box-shadow: 0 4px 12px rgba(0,0,0,.4); background: rgba(255,255,255,.08); }
      .oxl-meta { flex: 1; min-width: 0; }
      .oxl-title { font-size: 15px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .oxl-artist { margin-top: 3px; font-size: 13px; color: rgba(255,255,255,.62); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .oxl-tools { display: flex; align-items: center; gap: 2px; flex: none; }
      .oxl-tool { width: 32px; height: 32px; border: 0; border-radius: 50%; background: none; color: rgba(255,255,255,.7);
        display: grid; place-items: center; cursor: pointer; font: 600 13px/1 "YouTube Sans", "Roboto", sans-serif; }
      .oxl-tool:hover { background: rgba(255,255,255,.1); color: #fff; }
      .oxl-tool:focus-visible { outline: 2px solid #fff; outline-offset: -2px; }
      .oxl-tool svg { width: 18px; height: 18px; }

      .oxl-body { flex: 1; overflow-y: auto; padding: 38% 26px; scrollbar-width: none;
        -webkit-mask-image: linear-gradient(transparent 0, #000 16%, #000 80%, transparent 100%);
                mask-image: linear-gradient(transparent 0, #000 16%, #000 80%, transparent 100%); }
      .oxl-body::-webkit-scrollbar { display: none; }

      .oxl-line { display: block; width: fit-content; max-width: 100%; margin: 0 0 .5em -10px; padding: .12em 10px;
        border-radius: 10px; font-size: var(--oxl-size); font-weight: 700; line-height: 1.28; letter-spacing: -.01em;
        color: rgba(255,255,255,.34); cursor: pointer; transform-origin: left center;
        transition: color .35s ease, transform .45s cubic-bezier(.2,.8,.2,1), filter .45s ease, opacity .45s ease; }
      .oxl-line:hover { background: rgba(255,255,255,.07); color: rgba(255,255,255,.7); }
      .oxl-line.past { color: rgba(255,255,255,.5); }
      .oxl-line.far { filter: blur(1.1px); opacity: .8; }
      /* Current line: bright, and lit word by word in reading order (so a
         line that wraps onto two rows fills the top row first). */
      .oxl-line.current { transform: scale(1.035); opacity: 1; color: rgba(255,255,255,.55);
        filter: drop-shadow(0 0 14px rgba(255,255,255,.22)); }
      .oxl-line.current:hover { background: none; }
      /* Every word on the current line is painted the same way; only --wp
         moves (-12% = not sung, 112% = fully sung), so nothing switches
         rendering mode when a word finishes and there's no flicker. */
      .oxl-line.current .oxl-w { color: transparent;
        background: linear-gradient(90deg, #fff var(--wp, -12%), rgba(255,255,255,.55) calc(var(--wp, -12%) + 12%));
        -webkit-background-clip: text; background-clip: text; }

      /* Drag the left edge to resize. */
      .oxl-grip { position: absolute; left: 0; top: 0; bottom: 0; width: 12px; z-index: 3; cursor: ew-resize;
        display: flex; align-items: center; justify-content: center; touch-action: none; outline: none; }
      .oxl-grip::after { content: ''; width: 4px; height: 44px; border-radius: 2px; background: rgba(255,255,255,.22);
        opacity: 0; transition: opacity .2s, background .2s; }
      .oxl:hover .oxl-grip::after { opacity: 1; }
      .oxl-grip:hover::after, .oxl-grip.dragging::after, .oxl-grip:focus-visible::after { opacity: 1; background: rgba(255,255,255,.7); }
      .oxl.resizing, .oxl.resizing * { user-select: none; transition: none !important; }

      .oxl-gap { display: flex; gap: 8px; height: calc(var(--oxl-size) * 1.28); align-items: center; margin: 0 0 .5em; }
      .oxl-gap i { width: 9px; height: 9px; border-radius: 50%; background: rgba(255,255,255,.3); }
      .oxl-gap.current i { background: #fff; animation: oxl-dot 1.2s ease-in-out infinite; }
      .oxl-gap.current i:nth-child(2) { animation-delay: .15s; }
      .oxl-gap.current i:nth-child(3) { animation-delay: .3s; }
      @keyframes oxl-dot { 0%, 100% { transform: scale(.7); opacity: .5; } 50% { transform: scale(1.1); opacity: 1; } }

      .oxl-plain { white-space: pre-wrap; font-size: calc(var(--oxl-size) * .76); font-weight: 500; line-height: 1.65;
        color: rgba(255,255,255,.88); }
      .oxl-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
        text-align: center; color: rgba(255,255,255,.62); font-size: 15px; line-height: 1.5; margin-top: -18%; }
      .oxl-empty svg { width: 40px; height: 40px; opacity: .5; }
      .oxl-empty b { color: #fff; font-size: 17px; font-weight: 600; }
      .oxl-skel { height: calc(var(--oxl-size) * .9); border-radius: 8px; margin-bottom: .7em;
        background: linear-gradient(90deg, rgba(255,255,255,.06) 0%, rgba(255,255,255,.14) 50%, rgba(255,255,255,.06) 100%);
        background-size: 200% 100%; animation: oxl-shimmer 1.4s linear infinite; }
      @keyframes oxl-shimmer { to { background-position: -200% 0; } }

      .oxl-foot { display: flex; align-items: center; justify-content: space-between; padding: 8px 16px 12px;
        font-size: 11.5px; color: rgba(255,255,255,.45); }
      .oxl-resync { border: 0; border-radius: 14px; padding: 5px 12px; background: rgba(255,255,255,.14); color: #fff;
        font: 600 12px "YouTube Sans", "Roboto", sans-serif; cursor: pointer; visibility: hidden; }
      .oxl-resync.show { visibility: visible; }
      .oxl-resync:hover { background: rgba(255,255,255,.22); }
      html.oxide-pip .oxl { display: none !important; }

      @media (prefers-reduced-motion: reduce) {
        .oxl.open { animation: none; }
        .oxl-line { transition: color .2s; }
        .oxl-line.current { transform: none; }
        .oxl-line.far { filter: none; }
        .oxl-gap.current i, .oxl-skel { animation: none; }
      }
    `);

    // ---- DOM -----------------------------------------------------------
    const svg = (d) => pear.svgIcon(d, 18);
    const bg = pear.el('div', { class: 'oxl-bg', 'aria-hidden': 'true' });
    const art = pear.el('img', { class: 'oxl-art', alt: '' });
    const title = pear.el('div', { class: 'oxl-title', text: 'Lyrics' });
    const artist = pear.el('div', { class: 'oxl-artist' });
    const smaller = pear.el('button', { class: 'oxl-tool', title: 'Smaller text', 'aria-label': 'Smaller text', text: 'A−', onclick: () => setSize(fontSize - 2) });
    const bigger = pear.el('button', { class: 'oxl-tool', title: 'Larger text', 'aria-label': 'Larger text', text: 'A+', onclick: () => setSize(fontSize + 2) });
    const close = pear.el('button', { class: 'oxl-tool', title: 'Close lyrics', 'aria-label': 'Close lyrics', onclick: () => setOpen(false) },
      [svg('M19 6.4 17.6 5 12 10.6 6.4 5 5 6.4 10.6 12 5 17.6 6.4 19 12 13.4 17.6 19 19 17.6 13.4 12z')]);
    const body = pear.el('div', { class: 'oxl-body', role: 'list', 'aria-live': 'off' });
    const source = pear.el('span');
    const resync = pear.el('button', { class: 'oxl-resync', text: 'Back to current line', onclick: () => { userScrollUntil = 0; resync.classList.remove('show'); scrollToCurrent(true); } });
    const grip = pear.el('div', { class: 'oxl-grip', role: 'separator', 'aria-orientation': 'vertical',
      'aria-label': 'Resize lyrics panel', title: 'Drag to resize · double-click to reset', tabindex: '0' });
    const panel = ctx.mount(pear.el('section', { class: 'oxl' + (open ? ' open' : ''), 'aria-label': 'Lyrics' }, [
      bg,
      grip,
      pear.el('header', { class: 'oxl-head' }, [art, pear.el('div', { class: 'oxl-meta' }, [title, artist]), pear.el('div', { class: 'oxl-tools' }, [smaller, bigger, close])]),
      body,
      pear.el('footer', { class: 'oxl-foot' }, [source, resync]),
    ]));
    panel.style.setProperty('--oxl-size', fontSize + 'px');

    // ---- resizing --------------------------------------------------------
    const applyWidth = () => {
      if (width) panel.style.setProperty('--oxl-w', Math.min(maxW(), Math.max(MIN_W, width)) + 'px');
      else panel.style.removeProperty('--oxl-w');
      grip.setAttribute('aria-valuenow', String(Math.round(panel.offsetWidth || width)));
    };
    let widthSaveTimer = 0;
    const saveWidth = () => {
      clearTimeout(widthSaveTimer);
      widthSaveTimer = setTimeout(() => ctx.saveOption('width', width || null), 400);
    };
    const setWidth = (w) => {
      width = w ? Math.round(Math.min(maxW(), Math.max(MIN_W, w))) : 0;
      applyWidth();
      saveWidth();
      requestAnimationFrame(() => scrollToCurrent(true));
    };
    applyWidth();
    ctx.listen(grip, 'pointerdown', (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      try { grip.setPointerCapture(e.pointerId); } catch (_) {}
      grip.classList.add('dragging');
      panel.classList.add('resizing');
      const startX = e.clientX, startW = panel.offsetWidth;
      const move = (ev) => {
        // The panel is pinned to the right, so dragging left widens it.
        width = Math.round(Math.min(maxW(), Math.max(MIN_W, startW + (startX - ev.clientX))));
        applyWidth();
      };
      const up = () => {
        grip.removeEventListener('pointermove', move);
        grip.classList.remove('dragging');
        panel.classList.remove('resizing');
        saveWidth();
        scrollToCurrent(true);
      };
      grip.addEventListener('pointermove', move);
      grip.addEventListener('pointerup', up, { once: true });
      grip.addEventListener('pointercancel', up, { once: true });
    });
    ctx.listen(grip, 'dblclick', () => setWidth(0));
    ctx.listen(grip, 'keydown', (e) => {
      if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
        setWidth(panel.offsetWidth + (e.key === 'ArrowLeft' ? 24 : -24));
      }
    });
    ctx.listen(window, 'resize', applyWidth);

    // ---- parsing -------------------------------------------------------
    function parseLrc(lrc) {
      const out = [];
      for (const raw of lrc.split(/\r?\n/)) {
        const stamps = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
        if (!stamps.length) continue;
        const text = raw.replace(/\[[^\]]*\]/g, '').trim();
        for (const m of stamps) out.push({ time: Number(m[1]) * 60 + Number(m[2]), text });
      }
      out.sort((a, b) => a.time - b.time);
      // Empty lines and long silences become a "• • •" breather.
      const result = [];
      out.forEach((line, i) => {
        const empty = !line.text || line.text === '♪';
        if (i === 0 && line.time > 6) result.push({ time: 0, text: '', gap: true });
        if (empty) { if (!result.length || !result[result.length - 1].gap) result.push({ time: line.time, text: '', gap: true }); return; }
        result.push(line);
      });
      return result;
    }

    // ---- rendering -----------------------------------------------------
    const empty = (heading, sub, icon) => {
      lines = [];
      currentIndex = -1;
      body.replaceChildren(pear.el('div', { class: 'oxl-empty' }, [icon ? svg(icon) : null, pear.el('b', { text: heading }), sub ? pear.el('span', { text: sub }) : null]));
    };
    const loading = () => {
      lines = [];
      currentIndex = -1;
      const widths = [78, 92, 64, 86, 55, 72];
      body.replaceChildren(...widths.map((w) => pear.el('div', { class: 'oxl-skel', style: { width: w + '%' } })));
    };
    const NOTE = 'M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z';

    function render(result) {
      currentIndex = -1;
      if (result && result.synced) {
        lines = parseLrc(result.synced);
        body.replaceChildren(...lines.map((line) => {
          if (line.gap) {
            line.node = pear.el('div', { class: 'oxl-gap', role: 'listitem', 'aria-label': 'Instrumental' }, [pear.el('i'), pear.el('i'), pear.el('i')]);
          } else {
            line.node = pear.el('p', { class: 'oxl-line', role: 'listitem', title: 'Jump here' });
            // One span per word. Each word owns a slice of the line's sung
            // time, sized by its length, so the fill moves in reading order.
            const parts = line.text.split(/(\s+)/);
            const total = line.text.replace(/\s+/g, '').length || 1;
            let acc = 0;
            line.words = [];
            for (const part of parts) {
              if (!part) continue;
              if (/^\s+$/.test(part)) { line.node.append(document.createTextNode(part)); continue; }
              const node = pear.el('span', { class: 'oxl-w', text: part });
              const from = acc / total;
              acc += part.length;
              line.words.push({ node, from, to: acc / total });
              line.node.append(node);
            }
          }
          line.node.addEventListener('click', () => {
            pear.command('seek', Math.max(0, line.time - offset() / 1000));
            userScrollUntil = 0;
          });
          return line.node;
        }));
        source.textContent = 'Synced lyrics · LRCLIB';
        tick();
      } else if (result && result.plain) {
        lines = [];
        body.replaceChildren(pear.el('div', { class: 'oxl-plain', text: result.plain }));
        source.textContent = 'Lyrics · LRCLIB (not time-synced)';
      } else if (result && result.instrumental) {
        empty('Instrumental', 'Just listen.', NOTE);
        source.textContent = '';
      } else {
        empty('No lyrics for this song', 'LRCLIB doesn’t have lyrics for it yet.', NOTE);
        source.textContent = '';
      }
    }

    async function load(song) {
      title.textContent = song.title;
      artist.textContent = song.artist;
      if (song.artUrl) {
        art.src = song.artUrl;
        bg.style.backgroundImage = `url("${song.artUrl.replace(/"/g, '%22')}")`;
      }
      const id = ++requestId;
      loading();
      source.textContent = 'Searching…';
      try {
        const result = await pear.invoke('lyrics_get', { title: song.title, artist: song.artist, album: song.album, duration: song.duration });
        if (id === requestId) render(result);
      } catch (e) {
        if (id === requestId) { empty('Couldn’t load lyrics', String(e)); source.textContent = ''; }
      }
    }

    // ---- syncing -------------------------------------------------------
    function scrollToCurrent(force) {
      if (currentIndex < 0 || !open) return;
      if (!force && Date.now() < userScrollUntil) return;
      const node = lines[currentIndex].node;
      const target = node.offsetTop - body.clientHeight / 2 + node.offsetHeight / 2;
      const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
      body.scrollTo({ top: target, behavior: reduce ? 'auto' : 'smooth' });
    }

    function tick() {
      const v = pear.video;
      if (!v || !lines.length) return;
      const t = v.currentTime + offset() / 1000;
      let idx = -1;
      for (let i = 0; i < lines.length && lines[i].time <= t; i++) idx = i;
      if (idx !== currentIndex) {
        currentIndex = idx;
        lines.forEach((line, i) => {
          line.node.classList.toggle('current', i === idx);
          line.node.classList.toggle('past', i < idx);
          line.node.classList.toggle('far', Math.abs(i - idx) >= 3);
          if (i !== idx && line.words) for (const w of line.words) w.node.style.removeProperty('--wp');
        });
        scrollToCurrent(false);
      }
      // Light the current line word by word in step with the music.
      const line = lines[idx];
      if (line && line.words) {
        const start = line.time;
        const next = lines[idx + 1] ? lines[idx + 1].time : start + 6;
        // Singing usually ends before the next line starts; estimate from length.
        const chars = line.text.replace(/\s+/g, '').length;
        const sungFor = Math.max(0.6, Math.min(next - start - 0.15, 0.6 + chars * 0.085));
        const p = Math.min(1, Math.max(0, (t - start) / sungFor));
        for (const w of line.words) {
          // -12..112 so the soft edge fully enters and leaves each word.
          const k = Math.min(1, Math.max(0, (p - w.from) / (w.to - w.from)));
          const wp = (-12 + k * 124).toFixed(1) + '%';
          if (w.wp !== wp) { w.wp = wp; w.node.style.setProperty('--wp', wp); }
        }
      }
    }

    const loop = () => {
      tick();
      const v = pear.video;
      raf = open && v && !v.paused && !document.hidden ? requestAnimationFrame(loop) : 0;
    };
    const kick = () => { if (!raf && open) raf = requestAnimationFrame(loop); };
    ctx.cleanup(() => cancelAnimationFrame(raf));

    // Manual scrolling pauses follow mode for a few seconds.
    const pauseFollow = () => {
      userScrollUntil = Date.now() + 5000;
      if (lines.length) resync.classList.add('show');
      clearTimeout(pauseFollow.t);
      pauseFollow.t = setTimeout(() => { resync.classList.remove('show'); scrollToCurrent(true); }, 5000);
    };
    ctx.listen(body, 'wheel', pauseFollow, { passive: true });
    ctx.listen(body, 'touchmove', pauseFollow, { passive: true });

    // ---- controls ------------------------------------------------------
    const button = ctx.playerButton('Lyrics', 'M14 3v10.55A4 4 0 1 0 16 17V7h4V3h-6zM4 6h8v2H4zm0 4h8v2H4zm0 4h5v2H4z', () => setOpen(!open));
    button.classList.toggle('active', open);

    function setOpen(value) {
      open = value;
      try { localStorage.setItem(STORE_KEY, open ? '1' : '0'); } catch (_) {}
      panel.classList.toggle('open', open);
      button.classList.toggle('active', open);
      if (open) { currentIndex = -1; tick(); kick(); requestAnimationFrame(() => scrollToCurrent(true)); }
    }
    let saveTimer = 0;
    function setSize(px) {
      fontSize = Math.min(MAX_SIZE, Math.max(MIN_SIZE, px));
      panel.style.setProperty('--oxl-size', fontSize + 'px');
      smaller.disabled = fontSize <= MIN_SIZE;
      bigger.disabled = fontSize >= MAX_SIZE;
      requestAnimationFrame(() => scrollToCurrent(true));
      clearTimeout(saveTimer);
      saveTimer = setTimeout(() => ctx.saveOption('fontSize', fontSize), 500);
    }

    ctx.on('song', (song) => pear.whenReady(() => load(song)));
    ctx.command('synced-lyrics:toggle', () => setOpen(!open));
    ctx.listen(document, 'play', kick, true);
    ctx.listen(document, 'seeked', () => { userScrollUntil = 0; currentIndex = -1; tick(); }, true);
    ctx.listen(document, 'timeupdate', (e) => { if (e.target instanceof HTMLMediaElement && !raf) tick(); }, true);
    if (pear.song) pear.whenReady(() => load(pear.song));
  },
  // Font size and width are saved by the panel itself; only a timing change needs a restart.
  onConfig: (a, b) => (a.offsetMs !== b.offsetMs ? 'restart' : null),
});
