// Keeps the full-screen player page from popping open when you start a song
// from a list. Click the player bar to open it yourself.
pear.plugin('unobtrusive-player', {
  start(ctx) {
    let startedFromList = false;
    ctx.listen(document, 'click', (e) => {
      const t = e.target;
      if (!(t instanceof Element)) return;
      if (t.closest(pear.sel.bar)) { startedFromList = false; return; }
      if (t.closest('ytmusic-play-button-renderer, ytmusic-responsive-list-item-renderer, ytmusic-two-row-item-renderer, ytmusic-menu-navigation-item-renderer')
          && !t.closest('ytmusic-player-page')) {
        startedFromList = true;
      }
    }, true);
    const layout = () => document.querySelector('ytmusic-app-layout');
    ctx.watch('ytmusic-app-layout', (el) => {
      ctx.observe(el, { attributes: true, attributeFilter: ['player-ui-state'] }, () => {
        if (startedFromList && el.getAttribute('player-ui-state') === 'PLAYER_PAGE_OPEN') {
          startedFromList = false;
          pear.playerPage(false);
        }
      });
    });
    void layout;
  },
});
