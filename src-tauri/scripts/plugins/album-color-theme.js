// Tints the app with colours from the current album art.
pear.plugin('album-color-theme', {
  start(ctx, config) {
    const strength = Math.min(1, Math.max(0.1, Number(config.ratio) || 0.5));
    const root = document.documentElement;
    ctx.style(`
      :root.oxide-act {
        --ytmusic-general-background-A: var(--oxide-act-bg) !important;
        --ytmusic-general-background-B: var(--oxide-act-bg) !important;
        --ytmusic-general-background-C: var(--oxide-act-bg) !important;
        --ytmusic-brand-background-solid: var(--oxide-act-bg2) !important;
        --ytmusic-player-bar-background: var(--oxide-act-bar) !important;
      }
      :root.oxide-act body, :root.oxide-act ytmusic-app-layout, :root.oxide-act #nav-bar-background,
      :root.oxide-act #mini-guide-background, :root.oxide-act ytmusic-player-bar, :root.oxide-act #player-page,
      :root.oxide-act ytmusic-browse-response, :root.oxide-act #guide-wrapper {
        transition: background-color .8s ease;
      }
      :root.oxide-act body, :root.oxide-act #layout { background: var(--oxide-act-bg) !important; }
      :root.oxide-act #nav-bar-background.ytmusic-app-layout[opened], :root.oxide-act ytmusic-app-layout.content-scrolled #nav-bar-background { background: var(--oxide-act-bg2) !important; }
      :root.oxide-act ytmusic-player-bar { background: var(--oxide-act-bar) !important; }
      :root.oxide-act #player-page { background: radial-gradient(ellipse at 30% 20%, var(--oxide-act-glow), transparent 60%), var(--oxide-act-bg) !important; }
      :root.oxide-act #guide-wrapper, :root.oxide-act #mini-guide-background { background: var(--oxide-act-bg) !important; }
    `);
    const mix = (c, k) => `rgb(${(c[0] * k) | 0}, ${(c[1] * k) | 0}, ${(c[2] * k) | 0})`;
    const apply = (song) => {
      const art = document.querySelector('#song-image img')?.src || song?.artUrl;
      pear.palette(art).then((cols) => {
        const c = cols[0];
        root.style.setProperty('--oxide-act-bg', mix(c, 0.12 * strength * 2));
        root.style.setProperty('--oxide-act-bg2', mix(c, 0.18 * strength * 2));
        root.style.setProperty('--oxide-act-bar', mix(c, 0.16 * strength * 2));
        root.style.setProperty('--oxide-act-glow', `rgba(${c[0]},${c[1]},${c[2]},${0.35 * strength})`);
        root.classList.add('oxide-act');
      });
    };
    ctx.on('song', apply);
    if (pear.song) apply(pear.song);
    ctx.cleanup(() => root.classList.remove('oxide-act'));
  },
});
