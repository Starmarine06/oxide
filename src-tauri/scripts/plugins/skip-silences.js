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
      if (!ready || !v || !an || v.paused || v.seeking || !Number.isFinite(v.duration)) { silentSince = null; return; }
      if (document.querySelector('#movie_player')?.classList.contains('ad-showing')) return;
      if (config.onlySkipBeginning && v.currentTime > 20) return;
      if (!buf || buf.length !== an.fftSize) buf = new Float32Array(an.fftSize);
      an.getFloatTimeDomainData(buf);
      let sum = 0;
      for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i];
      const rms = Math.sqrt(sum / buf.length);
      if (rms > THRESHOLD) { silentSince = null; return; }
      silentSince ??= v.currentTime;
      // Leave the last couple of seconds alone so songs still end naturally.
      if (v.currentTime - silentSince >= MIN_SILENCE && v.duration - v.currentTime > 3) {
        v.currentTime = Math.min(v.duration - 1, v.currentTime + 0.75);
      }
    }, 150);
  },
});
