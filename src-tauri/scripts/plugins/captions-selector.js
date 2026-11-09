// Pick a captions track for the current song.
pear.plugin('captions-selector', {
  start(ctx, config) {
    const tracks = () => {
      const api = pear.api;
      if (!api || !api.getOption) return [];
      try { api.loadModule?.('captions'); } catch (_) {}
      return api.getOption('captions', 'tracklist', { includeAsr: true }) || [];
    };
    const choose = (track) => {
      const api = pear.api;
      if (!api) return;
      if (!track) { api.setOption?.('captions', 'track', {}); api.unloadModule?.('captions'); return; }
      api.loadModule?.('captions');
      api.setOption('captions', 'track', track);
      ctx.saveOption('lastLanguage', track.languageCode);
    };
    const icon = 'M19 4H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2zm-8 7H9.5v-.5h-2v3h2V13H11v1a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1zm7 0h-1.5v-.5h-2v3h2V13H18v1a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1z';
    ctx.playerButton('Captions', icon, (e) => {
      const list = tracks();
      if (!list.length) { pear.toast('No captions for this song.'); return; }
      const current = pear.api.getOption('captions', 'track') || {};
      const items = [pear.el('button', { class: current.languageCode ? '' : 'on', text: 'Off', onclick: () => { choose(null); pear.closePopover(); } })]
        .concat(list.map((t) => pear.el('button', {
          class: current.languageCode === t.languageCode ? 'on' : '',
          text: t.displayName || t.languageName || t.languageCode,
          onclick: () => { choose(t); pear.closePopover(); },
        })));
      pear.popover(e.currentTarget, pear.el('div', { class: 'oxide-list' }, items), { title: 'Captions' });
    });
    ctx.on('song', () => {
      if (!config.autoload) return;
      ctx.timeout(() => {
        const list = tracks();
        const pick = list.find((t) => t.languageCode === config.lastLanguage) || list[0];
        if (pick) choose(pick);
      }, 1500);
    });
  },
  onConfig: (a, b) => (a.autoload !== b.autoload ? 'restart' : null),
});
