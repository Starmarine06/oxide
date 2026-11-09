// Precise volume: scroll on the player bar or the player to change the
// volume in small steps, with an on-screen readout. Optional Shift+Up/Down
// shortcuts when not typing.
pear.plugin('precise-volume', {
  start(ctx, config) {
    const step = () => Number(config.steps) || 1;
    ctx.style(`
      .oxide-vol-hud { position: fixed; left: 50%; top: 18%; transform: translateX(-50%); z-index: 2147483040;
        padding: 10px 18px; border-radius: 22px; background: rgba(20,20,20,.9); color: #fff;
        font: 500 16px Roboto, sans-serif; font-variant-numeric: tabular-nums; opacity: 0; transition: opacity .25s;
        pointer-events: none; display: flex; align-items: center; gap: 12px; }
      .oxide-vol-hud.show { opacity: 1; }
      .oxide-vol-hud span { min-width: 42px; text-align: right; }
      .oxide-vol-hud i { display: block; width: 140px; height: 4px; border-radius: 2px; background: rgba(255,255,255,.2); overflow: hidden; }
      .oxide-vol-hud i b { display: block; height: 100%; background: #fff; }
    `);
    const bar = pear.el('b');
    const text = pear.el('span');
    const hud = ctx.mount(pear.el('div', { class: 'oxide-vol-hud', role: 'status' }, [text, pear.el('i', {}, [bar])]));
    let hideTimer = 0;
    const show = (v) => {
      text.textContent = v + '%';
      bar.style.width = v + '%';
      hud.classList.add('show');
      clearTimeout(hideTimer);
      hideTimer = setTimeout(() => hud.classList.remove('show'), 900);
    };
    const change = (delta) => {
      const api = pear.api;
      if (!api || !api.getVolume) return;
      const v = Math.min(100, Math.max(0, Math.round(api.getVolume() + delta)));
      api.setVolume(v);
      if (v > 0 && api.isMuted()) api.unMute();
      const slider = pear.$('volume');
      if (slider) { slider.value = v; slider.dispatchEvent(new Event('input', { bubbles: true })); }
      show(v);
    };
    ctx.listen(window, 'wheel', (e) => {
      if (!(e.target instanceof Element)) return;
      if (!e.target.closest(pear.sel.bar + ', ytmusic-player')) return;
      if (e.target.closest('.oxide-popover')) return;
      e.preventDefault();
      change(e.deltaY < 0 ? step() : -step());
    }, { passive: false, capture: true });
    if (config.arrowKeys) {
      ctx.listen(window, 'keydown', (e) => {
        const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || /^(INPUT|TEXTAREA)$/.test(e.target.tagName));
        if (typing || !e.shiftKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
        e.preventDefault();
        change(e.key === 'ArrowUp' ? step() : -step());
      }, true);
    }
  },
});
