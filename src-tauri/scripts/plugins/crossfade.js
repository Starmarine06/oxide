// Smooth transitions between songs: the end of each track fades out and the
// next one fades in, so there's no abrupt cut or silent gap.
pear.plugin('crossfade', {
  start(ctx, config) {
    const fadeOut = () => Math.max(0.5, Number(config.fadeOutSeconds) || 6);
    const fadeIn = () => Math.max(0.2, Number(config.fadeInSeconds) || 2);
    let gain = null;
    let active = true;
    let skipping = false;
    ctx.cleanup(() => { active = false; pear.audio.removeFilter('crossfade'); });

    pear.audio.ready().then((ac) => {
      if (!active) return;
      gain = ac.createGain();
      pear.audio.addFilter('crossfade', 90, gain);
    });

    const set = (value, seconds) => {
      if (!gain) return;
      const t = gain.context.currentTime;
      gain.gain.cancelScheduledValues(t);
      gain.gain.setValueAtTime(gain.gain.value, t);
      gain.gain.linearRampToValueAtTime(value, t + seconds);
    };

    ctx.listen(document, 'timeupdate', (e) => {
      const v = e.target;
      if (!(v instanceof HTMLMediaElement) || v !== pear.video || !gain || v.paused || pear.duration() < 30) return;
      if (document.querySelector('#movie_player')?.classList.contains('ad-showing')) return;
      const remaining = (pear.duration() - pear.position()) / (v.playbackRate || 1);
      if (remaining <= fadeOut() && !skipping) {
        skipping = true;
        set(0, Math.max(0.1, remaining - 0.15));
        // Move on a moment before the hard end so the next song starts on the fade.
        ctx.timeout(() => { if (config.skipEnding) pear.command('next'); }, Math.max(0, (remaining - 0.3) * 1000));
      } else if (remaining > fadeOut() + 1 && skipping) {
        // User seeked back.
        skipping = false;
        set(1, 0.3);
      }
    }, true);

    const fadeInNow = () => {
      if (!gain) return;
      skipping = false;
      gain.gain.cancelScheduledValues(gain.context.currentTime);
      gain.gain.setValueAtTime(0.0001, gain.context.currentTime);
      set(1, fadeIn());
    };
    ctx.listen(document, 'loadedmetadata', (e) => { if (e.target instanceof HTMLMediaElement) fadeInNow(); }, true);
    // With gapless playback the next song can start without a new
    // loadedmetadata, so a song change also fades back in.
    ctx.on('song', () => { if (skipping) fadeInNow(); });

    ctx.listen(document, 'seeked', (e) => {
      const v = e.target;
      if (v instanceof HTMLMediaElement && gain && pear.duration() - pear.position() > fadeOut() + 1) { skipping = false; set(1, 0.2); }
    }, true);
  },
});
