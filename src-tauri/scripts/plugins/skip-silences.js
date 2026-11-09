// Skips stretches of silence (long intros, gaps before hidden tracks).
pear.plugin('skip-silences', {
  start(ctx, config) {
    const THRESHOLD = 0.004;     // RMS level treated as silence
    const MIN_SILENCE = 1.2;     // seconds of silence before skipping
    let silentSince = null;
    let buf = null;
    let ready = false;
    pear.audio.ready().then(() => { ready = true; });

    ctx.interval(() => {
      const v = pear.video;
      const an = pear.audio.analyser;
      if (!ready || !v || !an || v.paused || v.seeking || !(pear.duration() > 0)) { silentSince = null; return; }
      if (document.querySelector('#movie_player')?.classList.contains('ad-showing')) return;
      const now = pear.position();
      if (config.onlySkipBeginning && now > 20) return;
      if (!buf || buf.length !== an.fftSize) buf = new Float32Array(an.fftSize);
      an.getFloatTimeDomainData(buf);
      let sum = 0;
      for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i];
      const rms = Math.sqrt(sum / buf.length);
      if (rms > THRESHOLD) { silentSince = null; return; }
      silentSince ??= now;
      // Leave the last couple of seconds alone so songs still end naturally.
      if (now - silentSince >= MIN_SILENCE && pear.duration() - now > 3) {
        pear.seekTo(Math.min(pear.duration() - 1, now + 0.75));
      }
    }, 150);
  },
});
