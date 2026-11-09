// Ambient light: a soft, blurred glow of the video/art behind the player.
pear.plugin('ambient-mode', {
  start(ctx, config) {
    const blur = Number(config.blur) || 100;
    const opacity = Math.min(1, Number(config.opacity) || 0.8);
    const size = Number(config.size) || 100;
    ctx.style(`
      .oxide-ambient { position: absolute; z-index: -1; pointer-events: none; filter: blur(${blur}px) saturate(1.3);
        opacity: ${opacity}; transform: scale(${1 + size / 400}); transition: opacity .6s; border-radius: 12px; }
      ytmusic-player#player { overflow: visible !important; }
    `);
    const canvas = pear.el('canvas', { class: 'oxide-ambient', width: 64, height: 36, 'aria-hidden': 'true' });
    const g = canvas.getContext('2d');
    let player = null;
    ctx.watch('ytmusic-player#player', (p) => { player = p; p.prepend(canvas); });
    ctx.cleanup(() => canvas.remove());

    const artImg = new Image();
    artImg.crossOrigin = 'anonymous';
    ctx.on('song', (s) => { artImg.src = s.artUrl; });
    if (pear.song) artImg.src = pear.song.artUrl;

    const draw = () => {
      if (!player || document.hidden) return;
      const state = document.querySelector('ytmusic-app-layout')?.getAttribute('player-ui-state');
      if (state !== 'PLAYER_PAGE_OPEN' && state !== 'FULLSCREEN') return;
      // Match the player's box.
      const box = player.querySelector('#song-video:not([style*="none"]) , #song-image') || player;
      canvas.style.left = box.offsetLeft + 'px';
      canvas.style.top = box.offsetTop + 'px';
      canvas.style.width = box.offsetWidth + 'px';
      canvas.style.height = box.offsetHeight + 'px';
      const v = pear.video;
      const showingVideo = v && player.getAttribute('playback-mode') !== 'ATV_PREFERRED' && v.videoWidth && !player.classList.contains('oxide-song-only');
      try {
        if (showingVideo) g.drawImage(v, 0, 0, canvas.width, canvas.height);
        else if (artImg.complete && artImg.naturalWidth) g.drawImage(artImg, 0, 0, canvas.width, canvas.height);
      } catch (_) {}
    };
    ctx.interval(draw, Math.max(33, Number(config.interval) || 80));
  },
});
