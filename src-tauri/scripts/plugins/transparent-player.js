// Makes the page see-through so the window's Acrylic/Mica material shows.
pear.plugin('transparent-player', {
  start(ctx, config) {
    const a = Math.min(0.95, Math.max(0, Number(config.opacity) || 0.5));
    ctx.style(`
      html, body, ytmusic-app, ytmusic-app-layout, #layout, ytmusic-browse-response, #player-page,
      ytmusic-player-page, ytmusic-section-list-renderer, ytmusic-tabbed-search-results-renderer {
        background: transparent !important; }
      ytmusic-app-layout { background: rgba(0,0,0,${a}) !important; }
      #nav-bar-background, #mini-guide-background, #guide-wrapper, ytmusic-player-bar {
        background: rgba(0,0,0,${Math.min(1, a + 0.15)}) !important; }
      #oxide-titlebar { background: rgba(0,0,0,${Math.min(1, a + 0.2)}) !important; }
      :root { --ytmusic-general-background-A: transparent !important; --ytmusic-general-background-B: transparent !important;
        --ytmusic-general-background-C: transparent !important; }
    `);
  },
});
