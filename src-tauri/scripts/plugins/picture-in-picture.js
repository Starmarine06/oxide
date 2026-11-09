// Picture-in-picture: Rust shrinks the window and keeps it on top; this
// strips the page down to the artwork/video and the player controls.
pear.plugin('picture-in-picture', {
  start(ctx) {
    ctx.style(`
      html.oxide-pip ytmusic-nav-bar, html.oxide-pip #nav-bar-background, html.oxide-pip #nav-bar-divider,
      html.oxide-pip ytmusic-guide-renderer, html.oxide-pip #guide-wrapper, html.oxide-pip #mini-guide,
      html.oxide-pip ytmusic-player-page .side-panel, html.oxide-pip #oxide-titlebar .ox-menus,
      html.oxide-pip #oxide-titlebar .ox-logo, html.oxide-pip ytmusic-player-bar .right-controls,
      html.oxide-pip ytmusic-player-bar .middle-controls-buttons, html.oxide-pip .oxide-lyrics,
      html.oxide-pip ytmusic-player-bar .time-info, html.oxide-pip .ytMusicMiniPlayerRightSection,
      html.oxide-pip .ytMusicMiniPlayerLeftSection, html.oxide-pip .ytmusicPlayerControlsShuffleButton,
      html.oxide-pip .ytmusicPlayerControlsRepeatButton, html.oxide-pip .ytmusicPlayerControlsPlaybackRateButton,
      html.oxide-pip .ytmusicPlayerControlsSeekBackwardButton, html.oxide-pip .ytmusicPlayerControlsSeekForwardButton { display: none !important; }
      html.oxide-pip .ytMusicMiniPlayerMiddleSection { flex: 1; justify-content: center; }
      html.oxide-pip ytmusic-app-layout > [slot='player-page'] { margin-top: var(--menu-bar-height) !important;
        height: calc(100vh - var(--menu-bar-height) - var(--ytmusic-player-bar-height)) !important; }
      html.oxide-pip ytmusic-player-page { padding: 0 !important; }
      html.oxide-pip ytmusic-player-page #main-panel { padding: 8px !important; width: 100% !important; max-width: none !important; }
      html.oxide-pip ytmusic-player-page .content { padding: 0 !important; }
      html.oxide-pip ytmusic-player-bar .middle-controls { justify-content: center; }
      html.oxide-pip ytmusic-player-bar .left-controls { width: auto !important; }
      html.oxide-pip #oxide-titlebar .ox-title { font-size: 11.5px; }
    `);
    ctx.playerButton('Picture-in-picture', 'M19 7h-8v6h8V7zm2-4H3a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h18a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zm0 16H3V5h18v14z', () => pear.invoke('pip_toggle'));
    ctx.command('pip:enter', () => {
      pear.playerPage(true);
    });
    // Double-click the art to leave PiP.
    ctx.listen(document, 'dblclick', (e) => {
      if (document.documentElement.classList.contains('oxide-pip') && e.target instanceof Element && e.target.closest('ytmusic-player')) pear.invoke('pip_toggle');
    });
    ctx.cleanup(() => document.documentElement.classList.remove('oxide-pip'));
  },
});
