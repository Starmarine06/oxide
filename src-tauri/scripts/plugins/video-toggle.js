// Song/Video switch on the player page, or hide the video area entirely.
pear.plugin('video-toggle', {
  start(ctx, config) {
    if (config.forceHide) {
      ctx.style(`
        ytmusic-player-page #main-panel { display: none !important; }
        ytmusic-player-page .side-panel.ytmusic-player-page { max-width: 100% !important; width: 100% !important; margin: 0 !important; }
      `);
      return;
    }
    if (config.mode === 'native') {
      ctx.watch('ytmusic-player-page', (p) => p.setAttribute('has-av-switcher', ''));
      ctx.watch('ytmusic-player', (p) => p.setAttribute('has-av-switcher', ''));
      ctx.watch('ytmusic-av-toggle', (t) => t.removeAttribute('toggle-disabled'));
      return;
    }
    if (config.mode === 'disabled') {
      ctx.watch('ytmusic-player-page', (p) => p.removeAttribute('has-av-switcher'));
      ctx.watch('ytmusic-player', (p) => p.removeAttribute('has-av-switcher'));
      ctx.watch('ytmusic-av-toggle', (t) => t.setAttribute('toggle-disabled', ''));
      return;
    }

    const justify = { left: 'flex-start', middle: 'center', right: 'flex-end' }[config.align] || 'flex-start';
    ctx.style(`
      ytmusic-player-page #av-id { display: none !important; }
      .oxide-av { position: absolute; top: 16px; left: 0; right: 0; z-index: 30; display: flex; justify-content: ${justify};
        padding: 0 12px; pointer-events: none; }
      .oxide-av-switch { pointer-events: auto; display: inline-flex; padding: 3px; border-radius: 20px;
        background: rgba(0,0,0,.55); backdrop-filter: blur(8px); }
      .oxide-av-switch button { border: 0; padding: 7px 18px; border-radius: 17px; background: none; color: #ddd;
        font: 500 13px Roboto, sans-serif; cursor: pointer; }
      .oxide-av-switch button.on { background: #fff; color: #000; }
      ytmusic-player.oxide-song-only #song-video { display: none !important; }
      ytmusic-player.oxide-song-only #song-image { display: block !important; }
    `);
    let showVideo = !config.hideVideo;
    const songBtn = pear.el('button', { text: 'Song', onclick: () => set(false) });
    const videoBtn = pear.el('button', { text: 'Video', onclick: () => set(true) });
    const wrap = pear.el('div', { class: 'oxide-av' }, [
      pear.el('div', { class: 'oxide-av-switch', role: 'group', 'aria-label': 'Song or video' }, [songBtn, videoBtn]),
    ]);
    const audioOnlyTrack = () => pear.api?.getPlayerResponse?.()?.videoDetails?.musicVideoType === 'MUSIC_VIDEO_TYPE_ATV';
    const apply = () => {
      const player = document.querySelector('ytmusic-player');
      const audioOnly = audioOnlyTrack();
      wrap.style.display = audioOnly ? 'none' : '';
      const video = showVideo && !audioOnly;
      songBtn.classList.toggle('on', !video);
      videoBtn.classList.toggle('on', video);
      if (player) {
        player.classList.toggle('oxide-song-only', !video);
        player.setAttribute('playback-mode', video ? 'OMV_PREFERRED' : 'ATV_PREFERRED');
      }
      if (!video) fixArtwork();
    };
    // In song mode on a music video YouTube leaves the artwork as a 1px
    // placeholder; load the video's thumbnail instead (as Pear Desktop does).
    const fixArtwork = () => {
      const img = document.querySelector('#song-image img');
      if (!img || !img.src.startsWith('data:')) return;
      const thumbs = pear.api?.getPlayerResponse?.()?.videoDetails?.thumbnail?.thumbnails || [];
      const url = thumbs.length ? thumbs[thumbs.length - 1].url.split('?')[0] : pear.song?.artUrl;
      if (url) img.src = url;
    };
    ctx.watch('#song-image img', (img) => ctx.observe(img, { attributes: true, attributeFilter: ['src'] }, fixArtwork));
    const set = (v) => { showVideo = v; apply(); ctx.saveOption('hideVideo', !v); };
    ctx.watch('ytmusic-player#player', (player) => {
      if (!player.style.position) player.style.position = 'relative';
      player.prepend(wrap);
      apply();
    });
    ctx.on('song', () => ctx.timeout(apply, 300));
    ctx.cleanup(() => { wrap.remove(); document.querySelector('ytmusic-player')?.classList.remove('oxide-song-only'); });
  },
  onConfig: (a, b) => (a.mode !== b.mode || a.align !== b.align || a.forceHide !== b.forceHide ? 'restart' : null),
});
