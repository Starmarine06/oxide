// Dynamic range compression: quiet parts louder, loud parts quieter.
pear.plugin('audio-compressor', {
  start(ctx, config) {
    const PRESETS = {
      light: { threshold: -18, knee: 30, ratio: 2.5, attack: 0.01, release: 0.25, makeup: 2 },
      medium: { threshold: -28, knee: 30, ratio: 4, attack: 0.005, release: 0.25, makeup: 5 },
      strong: { threshold: -40, knee: 24, ratio: 10, attack: 0.003, release: 0.2, makeup: 9 },
    };
    const p = PRESETS[config.strength] || PRESETS.medium;
    let active = true;
    ctx.cleanup(() => { active = false; pear.audio.removeFilter('audio-compressor'); });
    pear.audio.ready().then((ac) => {
      if (!active) return;
      const comp = ac.createDynamicsCompressor();
      comp.threshold.value = p.threshold;
      comp.knee.value = p.knee;
      comp.ratio.value = p.ratio;
      comp.attack.value = p.attack;
      comp.release.value = p.release;
      const makeup = ac.createGain();
      makeup.gain.value = Math.pow(10, p.makeup / 20);
      comp.connect(makeup);
      pear.audio.addFilter('audio-compressor', 40, comp, makeup);
    });
  },
});
