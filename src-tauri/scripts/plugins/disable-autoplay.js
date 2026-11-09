// Start songs paused (port of Pear Desktop's disable-autoplay).
pear.plugin('disable-autoplay', {
  start(ctx, config) {
    let done = false;
    ctx.listen(document, 'loadedmetadata', (e) => {
      const v = e.target;
      if (!(v instanceof HTMLVideoElement) || done) return;
      if (document.querySelector('#movie_player')?.classList.contains('ad-showing')) return;
      if (config.applyOnce) done = true;
      v.pause();
      // YouTube Music sometimes calls play() right after loading.
      v.addEventListener('timeupdate', () => v.pause(), { once: true });
    }, true);
  },
});
