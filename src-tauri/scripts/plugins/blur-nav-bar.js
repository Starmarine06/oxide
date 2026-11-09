// Transparent, blurred top bar (port of Pear Desktop's blur-nav-bar).
pear.plugin('blur-nav-bar', {
  start(ctx) {
    ctx.style(`
      #nav-bar-background, #header.ytmusic-item-section-renderer {
        background: rgba(0, 0, 0, 0.3) !important; backdrop-filter: blur(8px) !important; }
      ytmusic-tabs { backdrop-filter: blur(8px) !important; }
      ytmusic-tabs.stuck { background: rgba(0, 0, 0, 0.3) !important; }
      #nav-bar-divider { display: none !important; }
    `);
  },
});
