// Clock in the middle of the top bar.
pear.plugin('clock', {
  start(ctx, config) {
    ctx.style(`
      .oxide-clock { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);
        margin: 0; font: 500 20px/1 Roboto, sans-serif; color: var(--ytmusic-text-primary, #fff);
        font-variant-numeric: tabular-nums; letter-spacing: .02em; pointer-events: none; }
      @media (max-width: 1100px) { .oxide-clock { display: none; } }
    `);
    const clock = pear.el('div', { class: 'oxide-clock', 'aria-live': 'off' });
    const update = () => {
      clock.textContent = new Date().toLocaleTimeString(undefined, {
        hour: 'numeric', minute: '2-digit',
        second: config.displaySeconds ? '2-digit' : undefined,
        hour12: !!config.hour12,
      });
    };
    update();
    ctx.interval(update, 1000);
    ctx.watch('ytmusic-nav-bar', (nav) => nav.append(clock));
    ctx.cleanup(() => clock.remove());
  },
});
