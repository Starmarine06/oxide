// Back/forward arrows in the top bar, like a browser.
pear.plugin('navigation', {
  start(ctx) {
    ctx.style(`
      .oxide-nav { display: flex; align-items: center; gap: 2px; margin-right: 8px; }
      .oxide-nav .oxide-btn { color: var(--ytmusic-text-primary, #fff); }
      .oxide-nav .oxide-btn[disabled] { opacity: .35; pointer-events: none; }
    `);
    const back = pear.el('button', { class: 'oxide-btn', title: 'Back', 'aria-label': 'Back', onclick: () => history.back() },
      [pear.svgIcon('M15.4 7.4 14 6l-6 6 6 6 1.4-1.4L10.8 12z')]);
    const fwd = pear.el('button', { class: 'oxide-btn', title: 'Forward', 'aria-label': 'Forward', onclick: () => history.forward() },
      [pear.svgIcon('M8.6 16.6 10 18l6-6-6-6-1.4 1.4 4.6 4.6z')]);
    const box = pear.el('div', { class: 'oxide-nav' }, [back, fwd]);
    ctx.watch('ytmusic-nav-bar #right-content', (right) => right.prepend(box));
    ctx.cleanup(() => box.remove());
  },
});
