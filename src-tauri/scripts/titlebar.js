// Title bar for the frameless window: app menus, keyboard shortcuts and
// window buttons. Runs on every page the window shows (YouTube Music and the
// Google sign-in pages), so it must not depend on core.js.
(function () {
  if (window.top !== window || window.__oxideTitlebar) return;

  const HEIGHT = 32;
  const isMusic = location.hostname === 'music.youtube.com';
  const invoke = (cmd, args) => {
    const ipc = window.__TAURI_INTERNALS__;
    return ipc ? ipc.invoke(cmd, args) : Promise.reject(new Error('IPC unavailable'));
  };

  let model = { menus: [], hideMenu: false, macos: /Mac/.test(navigator.platform) };
  let altShown = false;
  let openIndex = -1;
  let accelerators = new Map();

  // ---- styles ------------------------------------------------------------

  const css = `
    :root { --menu-bar-height: ${HEIGHT}px; --oxide-accent: #e8642c; }
    #oxide-titlebar { position: fixed; inset: 0 0 auto 0; height: ${HEIGHT}px; z-index: 2147483000;
      display: flex; align-items: center; gap: 2px; padding: 0 0 0 8px; box-sizing: border-box;
      background: #0b0b0b; border-bottom: 1px solid rgba(255,255,255,.06);
      color: #e8e8e8; font: 400 12.5px/1 Roboto, "Segoe UI", system-ui, sans-serif;
      user-select: none; -webkit-user-select: none; transition: transform .25s cubic-bezier(.2,0,.6,1); }
    #oxide-titlebar.macos { padding-left: 78px; }
    #oxide-titlebar * { box-sizing: border-box; }
    #oxide-titlebar .ox-logo { width: 18px; height: 18px; margin: 0 6px 0 2px; flex: none; pointer-events: none; display: block; }
    #oxide-titlebar .ox-menus { display: flex; gap: 1px; }
    #oxide-titlebar.menu-hidden:not(.alt) .ox-menus { display: none; }
    #oxide-titlebar .ox-menu-btn { height: 24px; padding: 0 9px; border: 0; border-radius: 5px; background: none;
      color: inherit; font: inherit; line-height: 24px; cursor: default; display: inline-flex; align-items: center; }
    #oxide-titlebar .ox-menu-btn:hover, #oxide-titlebar .ox-menu-btn.open { background: rgba(255,255,255,.09); }
    #oxide-titlebar .ox-menu-btn:focus-visible { outline: 2px solid var(--oxide-accent); outline-offset: -2px; }
    #oxide-titlebar .ox-spacer { flex: 1; align-self: stretch; }
    /* Centred on the window, not on the space between menus and buttons. */
    #oxide-titlebar .ox-title { position: absolute; left: 50%; top: 0; bottom: 0; transform: translateX(-50%);
      max-width: calc(100% - 2 * var(--ox-side, 300px)); display: flex; align-items: center; justify-content: center;
      color: #9a9a9a; overflow: hidden; white-space: nowrap; pointer-events: none; }
    #oxide-titlebar .ox-title span { overflow: hidden; text-overflow: ellipsis; }
    #oxide-titlebar .ox-title b { color: #e8e8e8; font-weight: 500; }
    #oxide-titlebar .ox-back { height: 24px; padding: 0 10px; border: 0; border-radius: 5px; background: none;
      color: inherit; font: inherit; cursor: pointer; }
    #oxide-titlebar .ox-back:hover { background: rgba(255,255,255,.09); }
    #oxide-titlebar .ox-controls { display: flex; height: 100%; flex: none; }
    #oxide-titlebar.macos .ox-controls { display: none; }
    #oxide-titlebar .ox-ctl { width: 46px; height: 100%; border: 0; background: none; color: #cfcfcf;
      display: grid; place-items: center; cursor: default; }
    #oxide-titlebar .ox-ctl:hover { background: rgba(255,255,255,.09); color: #fff; }
    #oxide-titlebar .ox-ctl.close:hover { background: #e81123; color: #fff; }
    #oxide-titlebar .ox-ctl svg { width: 10px; height: 10px; stroke: currentColor; fill: none; stroke-width: 1; shape-rendering: crispEdges; }

    .ox-panel { position: fixed; z-index: 2147483001; min-width: 230px; max-width: 360px;
      max-height: calc(100vh - ${HEIGHT + 12}px); overflow-y: auto; padding: 5px;
      background: #1c1c1c; border: 1px solid rgba(255,255,255,.08); border-radius: 9px;
      box-shadow: 0 12px 32px rgba(0,0,0,.55), 0 2px 6px rgba(0,0,0,.4);
      color: #e8e8e8; font: 400 13px/1 Roboto, "Segoe UI", system-ui, sans-serif;
      animation: ox-pop .12s cubic-bezier(.2,.8,.3,1); scrollbar-width: thin; }
    @keyframes ox-pop { from { opacity: 0; transform: translateY(-4px) scale(.98); } }
    @media (prefers-reduced-motion: reduce) { .ox-panel { animation: none; } #oxide-titlebar { transition: none; } }
    .ox-item { display: grid; grid-template-columns: 20px 1fr auto 14px; align-items: center; gap: 6px;
      height: 30px; padding: 0 8px 0 6px; border-radius: 6px; white-space: nowrap; cursor: default; outline: none; }
    .ox-item:hover, .ox-item.hover, .ox-item:focus-visible { background: rgba(255,255,255,.08); }
    .ox-item.disabled { opacity: .4; pointer-events: none; }
    .ox-item .ox-check { color: var(--oxide-accent); font-size: 13px; text-align: center; }
    .ox-item .ox-label { overflow: hidden; text-overflow: ellipsis; }
    .ox-item .ox-accel { color: #8a8a8a; font-size: 12px; padding-left: 18px; }
    .ox-item .ox-arrow { color: #8a8a8a; text-align: right; }
    .ox-item .ox-dot { display: block; width: 6px; height: 6px; border-radius: 50%; background: var(--oxide-accent); margin: auto; }
    .ox-sep { height: 1px; margin: 5px 6px; background: rgba(255,255,255,.08); }

    html.oxide-other { margin-top: ${HEIGHT}px !important; }
  `;

  // Make room for the bar in YouTube Music's fixed layout (from Pear
  // Desktop's in-app-menu plugin).
  const musicCss = `
    ytmusic-app-layout { overflow: auto scroll; height: calc(100vh - var(--menu-bar-height)); backface-visibility: hidden; }
    ytmusic-app-layout#layout { --ytmusic-nav-bar-offset: 0px; }
    ytmusic-app-layout > #content { padding-top: var(--menu-bar-height); }
    ytmusic-app-layout::-webkit-scrollbar { width: var(--ytmusic-scrollbar-width); }
    ytmusic-app-layout::-webkit-scrollbar-thumb { background-color: rgb(126,126,126); }
    ytmusic-app-layout::-webkit-scrollbar-track, ytmusic-app-layout::-webkit-scrollbar-corner { background: transparent; }
    ytmusic-app-layout::-webkit-scrollbar-thumb { border-radius: 8px; border: 3px solid transparent; background-clip: content-box; }
    ytmusic-app-layout > [slot='nav-bar'], #nav-bar-background.ytmusic-app-layout { top: var(--menu-bar-height) !important; }
    #nav-bar-divider.ytmusic-app-layout { top: calc(var(--ytmusic-nav-bar-height) + var(--menu-bar-height)) !important; }
    ytmusic-app[is-bauhaus-sidenav-enabled] #guide-spacer.ytmusic-app,
    ytmusic-app[is-bauhaus-sidenav-enabled] #mini-guide-spacer.ytmusic-app {
      margin-top: calc(var(--ytmusic-nav-bar-height) + var(--menu-bar-height)) !important; }
    @media (max-width: 935px) {
      ytmusic-app[is-bauhaus-sidenav-enabled] #guide-spacer.ytmusic-app { margin-top: var(--menu-bar-height) !important; }
    }
    ytmusic-app-layout > [slot='player-page'] { margin-top: var(--menu-bar-height);
      height: calc(100vh - var(--menu-bar-height) - var(--ytmusic-nav-bar-height) - var(--ytmusic-player-bar-height)) !important; }
    ytmusic-guide-renderer { height: calc(100vh - var(--menu-bar-height) - var(--ytmusic-nav-bar-height)) !important; }
    /* Signed-in sidebar is positioned from the window top; push it below the bar. */
    ytmusic-app:not([is-bauhaus-sidenav-enabled]) ytmusic-guide-renderer { margin-top: var(--menu-bar-height) !important; }
    html { scrollbar-color: unset; }
    ytmusic-player[player-ui-state='FULLSCREEN'] { margin-top: calc(var(--menu-bar-height) * -1) !important; }
    html:has(ytmusic-player[player-ui-state='FULLSCREEN']) #oxide-titlebar:not(.alt) { transform: translateY(-100%); }
  `;

  function addStyle(text) {
    const s = document.createElement('style');
    s.textContent = text;
    (document.head || document.documentElement).append(s);
  }

  // ---- DOM helpers (no innerHTML: the page enforces Trusted Types) -------

  function el(tag, cls, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined) n.textContent = text;
    return n;
  }

  function svg(paths, viewBox = '0 0 10 10') {
    const ns = 'http://www.w3.org/2000/svg';
    const s = document.createElementNS(ns, 'svg');
    s.setAttribute('viewBox', viewBox);
    s.setAttribute('aria-hidden', 'true');
    for (const [tag, attrs] of paths) {
      const p = document.createElementNS(ns, tag);
      for (const [k, v] of Object.entries(attrs)) p.setAttribute(k, v);
      s.append(p);
    }
    return s;
  }

  const logo = () => {
    // The bold small-size version of the app icon: thick ring, three bars.
    const s = svg([
      ['rect', { x: '0', y: '0', width: '18', height: '18', rx: '5', fill: '#1c1311' }],
      ['circle', { cx: '9', cy: '9', r: '5.5', fill: 'none', stroke: '#f0642a', 'stroke-width': '2.1' }],
      ['rect', { x: '6.15', y: '7.45', width: '1.4', height: '3.1', rx: '.7', fill: '#fff1e6' }],
      ['rect', { x: '8.3', y: '6.3', width: '1.4', height: '5.4', rx: '.7', fill: '#fff1e6' }],
      ['rect', { x: '10.45', y: '7.45', width: '1.4', height: '3.1', rx: '.7', fill: '#fff1e6' }],
    ], '0 0 18 18');
    s.setAttribute('class', 'ox-logo');
    return s;
  };

  const icons = {
    minimize: () => svg([['path', { d: 'M0 5.5h10' }]]),
    maximize: () => svg([['rect', { x: '0.5', y: '0.5', width: '9', height: '9' }]]),
    restore: () => svg([['rect', { x: '0.5', y: '2.5', width: '7', height: '7' }], ['path', { d: 'M2.5 2.5V.5h7v7h-2' }]]),
    close: () => svg([['path', { d: 'M0 0l10 10M10 0L0 10' }]]),
  };

  // ---- bar ---------------------------------------------------------------

  let bar, menusEl, titleEl, maxBtn;

  function build() {
    addStyle(css);
    if (isMusic) addStyle(musicCss);
    else document.documentElement.classList.add('oxide-other');

    bar = el('div');
    bar.id = 'oxide-titlebar';
    bar.setAttribute('role', 'menubar');
    bar.append(logo());

    menusEl = el('div', 'ox-menus');
    bar.append(menusEl);

    if (!isMusic) {
      const back = el('button', 'ox-back', '← Back to YouTube Music');
      back.onclick = () => { location.href = 'https://music.youtube.com/'; };
      bar.append(back);
    }

    bar.append(el('div', 'ox-spacer'));
    titleEl = el('div', 'ox-title');
    bar.append(titleEl);

    const controls = el('div', 'ox-controls');
    const ctl = (name, action, label) => {
      const b = el('button', 'ox-ctl ' + name);
      b.setAttribute('aria-label', label);
      b.title = label;
      b.append(icons[name]());
      b.onclick = () => invoke('window_control', { action }).then(updateMax).catch(() => {});
      controls.append(b);
      return b;
    };
    ctl('minimize', 'minimize', 'Minimize');
    maxBtn = ctl('maximize', 'maximize', 'Maximize');
    ctl('close', 'close', 'Close');
    bar.append(controls);

    // Drag the window from empty bar space; double-click to maximize.
    bar.addEventListener('mousedown', (e) => {
      if (e.button !== 0 || e.target.closest('button, .ox-menus')) return;
      if (e.detail === 2) invoke('window_control', { action: 'maximize' }).then(updateMax).catch(() => {});
      else invoke('window_control', { action: 'drag' }).catch(() => {});
    });

    document.body.append(bar);
    applyModel();
    updateTitle();
    new MutationObserver(updateTitle).observe(document.querySelector('title') || document.head, { childList: true, subtree: true, characterData: true });
    invoke('window_control', { action: 'state' }).then(updateMax).catch(() => {});
    window.addEventListener('resize', () => {
      updateTitle();
      invoke('window_control', { action: 'state' }).then(updateMax).catch(() => {});
    });
  }

  function updateMax(state) {
    if (!state || !maxBtn) return;
    maxBtn.replaceChildren(state.maximized ? icons.restore() : icons.maximize());
    maxBtn.title = state.maximized ? 'Restore' : 'Maximize';
    maxBtn.setAttribute('aria-label', maxBtn.title);
  }

  function updateTitle() {
    if (!titleEl) return;
    const song = window.__pear && window.__pear.song;
    const span = el('span');
    if (song && song.title) {
      span.append(el('b', '', song.title), document.createTextNode('  ·  ' + song.artist));
    } else {
      span.textContent = isMusic ? 'Oxide' : document.title || 'Oxide';
    }
    titleEl.replaceChildren(span);
    // Keep the centred title clear of whichever side is wider.
    const left = (menusEl?.getBoundingClientRect().right || 0) + 16;
    const right = innerWidth - (bar.querySelector('.ox-controls')?.getBoundingClientRect().left || innerWidth) + 16;
    bar.style.setProperty('--ox-side', Math.max(left, right) + 'px');
  }

  // ---- menus -------------------------------------------------------------

  let panels = [];

  function closeMenus() {
    for (const p of panels) p.remove();
    panels = [];
    openIndex = -1;
    menusEl?.querySelectorAll('.ox-menu-btn.open').forEach((b) => b.classList.remove('open'));
    if (altShown) { altShown = false; bar?.classList.remove('alt'); }
  }

  function closePanelsFrom(level) {
    while (panels.length > level) panels.pop().remove();
  }

  function renderPanel(items, x, y, level) {
    closePanelsFrom(level);
    const panel = el('div', 'ox-panel');
    panel.setAttribute('role', 'menu');
    for (const item of items) {
      if (item.type === 'separator') { panel.append(el('div', 'ox-sep')); continue; }
      const row = el('div', 'ox-item' + (item.disabled ? ' disabled' : ''));
      row.tabIndex = -1;
      row.setAttribute('role', item.type === 'check' ? 'menuitemcheckbox' : item.type === 'radio' ? 'menuitemradio' : 'menuitem');
      if (item.type === 'check' || item.type === 'radio') row.setAttribute('aria-checked', String(!!item.checked));
      if (item.description) row.title = item.description;

      const mark = el('span', 'ox-check');
      if (item.type === 'check' && item.checked) mark.textContent = '✓';
      if (item.type === 'radio' && item.checked) mark.append(el('span', 'ox-dot'));
      if (item.type === 'submenu' && item.checked) mark.append(el('span', 'ox-dot'));
      row.append(mark, el('span', 'ox-label', item.label));
      row.append(el('span', 'ox-accel', item.accelerator ? prettyAccel(item.accelerator) : ''));
      row.append(el('span', 'ox-arrow', item.type === 'submenu' ? '›' : ''));

      if (item.type === 'submenu') {
        const open = () => {
          panel.querySelectorAll('.ox-item.hover').forEach((n) => n.classList.remove('hover'));
          row.classList.add('hover');
          const r = row.getBoundingClientRect();
          renderPanel(item.items, r.right + 2, r.top - 6, level + 1);
        };
        row.addEventListener('mouseenter', open);
        row.addEventListener('click', open);
        row.addEventListener('keydown', (e) => { if (e.key === 'ArrowRight' || e.key === 'Enter') { open(); focusFirst(panels[level + 1]); e.preventDefault(); } });
      } else {
        row.addEventListener('mouseenter', () => {
          closePanelsFrom(level + 1);
          panel.querySelectorAll('.ox-item.hover').forEach((n) => n.classList.remove('hover'));
        });
        const activate = () => { closeMenus(); invoke('menu_click', { id: item.id }).catch(() => {}); };
        row.addEventListener('click', activate);
        row.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { activate(); e.preventDefault(); } });
      }
      panel.append(row);
    }
    panel.addEventListener('keydown', (e) => {
      const rows = [...panel.querySelectorAll('.ox-item:not(.disabled)')];
      const i = rows.indexOf(document.activeElement);
      if (e.key === 'ArrowDown') { rows[(i + 1) % rows.length]?.focus(); e.preventDefault(); }
      else if (e.key === 'ArrowUp') { rows[(i - 1 + rows.length) % rows.length]?.focus(); e.preventDefault(); }
      else if (e.key === 'ArrowLeft' && level > 0) { closePanelsFrom(level); panels[level - 1]?.querySelector('.ox-item.hover')?.focus(); e.preventDefault(); }
      else if (e.key === 'Escape') { closeMenus(); e.preventDefault(); }
    });
    document.body.append(panel);
    // Keep it on screen.
    const r = panel.getBoundingClientRect();
    const left = x + r.width > innerWidth - 4 ? Math.max(4, (level ? x - r.width - 4 - (panels[level - 1]?.offsetWidth || 0) : innerWidth - r.width - 4)) : x;
    const top = Math.max(HEIGHT + 2, Math.min(y, innerHeight - r.height - 4));
    panel.style.left = left + 'px';
    panel.style.top = top + 'px';
    panels.push(panel);
    return panel;
  }

  function focusFirst(panel) {
    panel?.querySelector('.ox-item:not(.disabled)')?.focus();
  }

  function openMenu(index, focus) {
    const menu = model.menus[index];
    if (!menu) return;
    const btn = menusEl.children[index];
    closePanelsFrom(0);
    menusEl.querySelectorAll('.ox-menu-btn.open').forEach((b) => b.classList.remove('open'));
    btn.classList.add('open');
    openIndex = index;
    const r = btn.getBoundingClientRect();
    const panel = renderPanel(menu.items, r.left, HEIGHT + 2, 0);
    if (focus) focusFirst(panel);
  }

  function applyModel() {
    if (!bar) return;
    bar.classList.toggle('macos', !!model.macos);
    bar.classList.toggle('menu-hidden', !!model.hideMenu);
    menusEl.replaceChildren();
    model.menus.forEach((menu, i) => {
      const b = el('button', 'ox-menu-btn', menu.label);
      b.setAttribute('aria-haspopup', 'true');
      b.addEventListener('mousedown', (e) => { e.stopPropagation(); openIndex === i ? closeMenus() : openMenu(i); });
      b.addEventListener('mouseenter', () => { if (openIndex !== -1 && openIndex !== i) openMenu(i); });
      b.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') { openMenu(i, true); e.preventDefault(); }
        else if (e.key === 'ArrowRight') { menusEl.children[(i + 1) % model.menus.length].focus(); e.preventDefault(); }
        else if (e.key === 'ArrowLeft') { menusEl.children[(i - 1 + model.menus.length) % model.menus.length].focus(); e.preventDefault(); }
      });
      menusEl.append(b);
    });
    accelerators = new Map();
    const walk = (items) => {
      for (const it of items || []) {
        if (it.type === 'submenu') walk(it.items);
        else if (it.accelerator && !it.disabled) accelerators.set(normalizeAccel(it.accelerator), it.id);
      }
    };
    model.menus.forEach((m) => walk(m.items));
    updateTitle();
  }

  // ---- keyboard ----------------------------------------------------------

  const isMac = /Mac/.test(navigator.platform);

  function normalizeAccel(a) {
    const parts = a.split('+').map((p) => p.trim().toLowerCase());
    const key = parts.pop();
    const mods = new Set(parts.map((m) => (m === 'cmdorctrl' ? (isMac ? 'meta' : 'ctrl') : m === 'cmd' || m === 'command' ? 'meta' : m === 'option' ? 'alt' : m)));
    return ['ctrl', 'meta', 'alt', 'shift'].filter((m) => mods.has(m)).concat(key).join('+');
  }

  function prettyAccel(a) {
    return a.replace(/CmdOrCtrl/g, isMac ? '⌘' : 'Ctrl').replace(/Left$/, '←').replace(/Right$/, '→').replace(/Up$/, '↑').replace(/Down$/, '↓');
  }

  function eventAccel(e) {
    let key = e.key.length === 1 ? e.key.toLowerCase() : e.key.replace(/^Arrow/, '').toLowerCase();
    const mods = [];
    if (e.ctrlKey) mods.push('ctrl');
    if (e.metaKey) mods.push('meta');
    if (e.altKey) mods.push('alt');
    if (e.shiftKey) mods.push('shift');
    return mods.concat(key).join('+');
  }

  let altAlone = false;
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Alt') { altAlone = true; return; }
    altAlone = false;
    if (e.key === 'Escape' && panels.length) { closeMenus(); return; }
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName));
    const accel = eventAccel(e);
    const id = accelerators.get(accel);
    // Let text fields keep their own shortcuts like Ctrl+Left.
    if (!id || (typing && !/^(ctrl|meta)\+[a-z]$/.test(accel) && !accel.includes('shift'))) return;
    e.preventDefault();
    e.stopPropagation();
    invoke('menu_click', { id }).catch(() => {});
  }, true);

  window.addEventListener('keyup', (e) => {
    if (e.key !== 'Alt' || !altAlone) return;
    altAlone = false;
    e.preventDefault();
    if (!bar || !model.menus.length) return;
    if (model.hideMenu) {
      altShown = !altShown;
      bar.classList.toggle('alt', altShown);
      if (altShown) menusEl.children[0]?.focus();
    } else {
      menusEl.children[0]?.focus();
    }
  }, true);

  document.addEventListener('mousedown', (e) => {
    if (panels.length && !e.target.closest?.('.ox-panel, .ox-menu-btn')) closeMenus();
  }, true);
  window.addEventListener('blur', closeMenus);

  // ---- startup -----------------------------------------------------------

  window.__oxideTitlebar = {
    setMenu(next) { model = next; closeMenus(); applyModel(); },
    updateTitle,
  };

  const start = () => {
    build();
    if (isMusic) {
      invoke('menu_model').then((m) => window.__oxideTitlebar.setMenu(m)).catch(() => {});
      setInterval(updateTitle, 2000);
    }
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, { once: true });
  else start();
})();
