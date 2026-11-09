// Video quality picker (for music videos).
pear.plugin('quality-changer', {
  start(ctx) {
    const gear = 'M19.4 13a7.5 7.5 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.4 7.4 0 0 0-1.7-1L15 3h-4l-.4 2.7a7.4 7.4 0 0 0-1.7 1l-2.5-1-2 3.5L6.5 11a7.5 7.5 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1a7.4 7.4 0 0 0 1.7 1L11 21h4l.4-2.7a7.4 7.4 0 0 0 1.7-1l2.5 1 2-3.5zM13 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7z';
    ctx.playerButton('Video quality', gear, (e) => {
      const api = pear.api;
      if (!api || !api.getAvailableQualityLevels) { pear.toast('Nothing is playing.'); return; }
      const levels = api.getAvailableQualityLevels();
      const labels = api.getAvailableQualityLabels ? api.getAvailableQualityLabels() : levels;
      const current = api.getPlaybackQuality();
      if (!levels.length) { pear.toast('This track has no quality options.'); return; }
      const list = pear.el('div', { class: 'oxide-list' }, levels.map((q, i) => pear.el('button', {
        class: q === current ? 'on' : '',
        text: labels[i] || q,
        onclick: () => {
          api.setPlaybackQualityRange?.(q);
          api.setPlaybackQuality?.(q);
          pear.closePopover();
          pear.toast('Quality: ' + (labels[i] || q));
        },
      })));
      pear.popover(e.currentTarget, list, { title: 'Video quality' });
    });
  },
});
