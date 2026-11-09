// Playback speed control: player-bar button with a slider and presets.
// Scroll over the button to nudge the speed.
pear.plugin('playback-speed', {
  start(ctx, config) {
    let speed = Number(config.speed) || 1;
    const MIN = 0.25, MAX = 3;
    const label = pear.el('span', { class: 'oxide-speed-label' });
    const fmt = (s) => s.toFixed(2).replace(/\.?0+$/, '') + '×';

    const apply = () => {
      const v = pear.video;
      if (v && Math.abs(v.playbackRate - speed) > 0.001) v.playbackRate = speed;
      label.textContent = fmt(speed);
      btn.classList.toggle('active', speed !== 1);
    };
    let saveTimer = 0;
    const set = (s) => {
      speed = Math.round(Math.min(MAX, Math.max(MIN, s)) * 100) / 100;
      apply();
      clearTimeout(saveTimer);
      saveTimer = setTimeout(() => ctx.saveOption('speed', speed), 600);
    };

    ctx.style(`
      .oxide-speed-label { font: 500 12px Roboto, sans-serif; min-width: 30px; text-align: center; }
      .oxide-speed-value { font: 500 26px Roboto, sans-serif; text-align: center; margin: 4px 0; font-variant-numeric: tabular-nums; }
    `);
    const btn = ctx.playerButton('Playback speed', '', (e) => {
      const value = pear.el('div', { class: 'oxide-speed-value', text: fmt(speed) });
      const slider = pear.range({ min: MIN, max: MAX, step: 0.05, value: speed, oninput: (v) => { set(v); value.textContent = fmt(speed); } });
      const presets = pear.el('div', { class: 'oxide-chips' }, [0.5, 0.75, 1, 1.25, 1.5, 2].map((p) =>
        pear.el('button', { class: 'oxide-chip' + (p === speed ? ' on' : ''), text: fmt(p), onclick: () => { set(p); pear.closePopover(); } })));
      pear.popover(e.currentTarget, pear.el('div', { style: { width: '240px' } }, [value, slider, presets]), { title: 'Playback speed' });
    });
    btn.replaceChildren(label);
    ctx.listen(btn, 'wheel', (e) => { e.preventDefault(); e.stopPropagation(); set(speed + (e.deltaY < 0 ? 0.05 : -0.05)); }, { passive: false });
    // YouTube Music resets the rate on every new track.
    ctx.listen(document, 'ratechange', (e) => { if (e.target instanceof HTMLMediaElement) apply(); }, true);
    ctx.listen(document, 'loadedmetadata', (e) => { if (e.target instanceof HTMLMediaElement) apply(); }, true);
    ctx.cleanup(() => { const v = pear.video; if (v) v.playbackRate = 1; });
    apply();
  },
  // Speed changes come from this plugin itself; no restart needed.
  onConfig() {},
});
