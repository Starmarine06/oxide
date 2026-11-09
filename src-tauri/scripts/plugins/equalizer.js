// 10-band equalizer with presets. Open it from the player-bar button.
pear.plugin('equalizer', {
  start(ctx, config) {
    const BANDS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
    const PRESETS = {
      Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      'Bass boost': [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
      'Bass reducer': [-6, -5, -4, -2, 0, 0, 0, 0, 0, 0],
      'Treble boost': [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
      Vocal: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1],
      Rock: [5, 4, 2, 0, -1, -1, 1, 3, 4, 5],
      Pop: [-1, 1, 3, 4, 3, 0, -1, -1, 0, 1],
      Electronic: [5, 4, 1, 0, -2, 1, 0, 2, 4, 5],
      Acoustic: [4, 4, 3, 1, 2, 2, 3, 3, 2, 1],
      Loudness: [6, 4, 0, 0, -2, 0, -1, -4, 4, 2],
      'Late night': [3, 2, 1, 0, -1, -1, 0, 1, 1, 0],
    };
    let gains = Array.isArray(config.gains) && config.gains.length === 10 ? config.gains.slice() : PRESETS[config.preset] || PRESETS.Flat;
    let preset = config.preset || 'Flat';
    let filters = [];
    let preamp = null;

    pear.audio.ready().then((ac) => {
      if (!ctx.active) return;
      preamp = ac.createGain();
      filters = BANDS.map((f, i) => {
        const b = ac.createBiquadFilter();
        b.type = i === 0 ? 'lowshelf' : i === BANDS.length - 1 ? 'highshelf' : 'peaking';
        b.frequency.value = f;
        b.Q.value = 1.1;
        return b;
      });
      preamp.connect(filters[0]);
      for (let i = 0; i < filters.length - 1; i++) filters[i].connect(filters[i + 1]);
      pear.audio.addFilter('equalizer', 20, preamp, filters[filters.length - 1]);
      apply();
    });
    ctx.active = true;
    ctx.cleanup(() => { ctx.active = false; pear.audio.removeFilter('equalizer'); });

    function apply() {
      filters.forEach((f, i) => f.gain.setTargetAtTime(gains[i], f.context.currentTime, 0.03));
      // Pull the level down by part of the biggest boost to avoid clipping.
      if (preamp) preamp.gain.value = Math.pow(10, -Math.max(0, ...gains) * 0.6 / 20);
      btn.classList.toggle('active', gains.some((g) => g !== 0));
    }
    let saveTimer = 0;
    const save = () => {
      clearTimeout(saveTimer);
      saveTimer = setTimeout(() => { ctx.saveOption('gains', gains); ctx.saveOption('preset', preset); }, 500);
    };

    ctx.style(`
      .oxide-eq { display: grid; grid-template-columns: repeat(10, 30px); gap: 6px; justify-content: center; margin: 6px 0 14px; }
      .oxide-eq-band { display: flex; flex-direction: column; align-items: center; gap: 6px; font: 11px Roboto, sans-serif; color: #9a9a9a; }
      .oxide-eq-band output { color: #ddd; font-variant-numeric: tabular-nums; min-height: 13px; }
      .oxide-eq-band input { writing-mode: vertical-lr; direction: rtl; width: 4px; height: 130px; margin: 0;
        -webkit-appearance: none; appearance: none; border-radius: 2px;
        background: linear-gradient(to top, #e8642c var(--p, 50%), rgba(255,255,255,.18) var(--p, 50%)); }
      .oxide-eq-band input::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 14px; border-radius: 50%; background: #fff; cursor: pointer; }
    `);

    const icon = 'M10 20h4V4h-4v16zm-6 0h4v-8H4v8zM16 9v11h4V9h-4z';
    const btn = ctx.playerButton('Equalizer', icon, (e) => {
      const bands = BANDS.map((f, i) => {
        const out = pear.el('output', { text: (gains[i] > 0 ? '+' : '') + gains[i] });
        const input = pear.el('input', { type: 'range', min: -12, max: 12, step: 0.5, value: gains[i], 'aria-label': f + ' Hz' });
        const paint = () => input.style.setProperty('--p', ((input.value - -12) / 24) * 100 + '%');
        paint();
        input.addEventListener('input', () => {
          gains[i] = Number(input.value);
          out.textContent = (gains[i] > 0 ? '+' : '') + gains[i];
          paint();
          preset = 'Custom';
          chips.querySelectorAll('.oxide-chip').forEach((c) => c.classList.toggle('on', c.textContent === preset));
          apply();
          save();
        });
        return pear.el('div', { class: 'oxide-eq-band' }, [out, input, pear.el('span', { text: f >= 1000 ? f / 1000 + 'k' : String(f) })]);
      });
      const chips = pear.el('div', { class: 'oxide-chips' }, Object.keys(PRESETS).map((name) => pear.el('button', {
        class: 'oxide-chip' + (name === preset ? ' on' : ''),
        text: name,
        onclick: () => {
          preset = name;
          gains = PRESETS[name].slice();
          apply();
          save();
          pear.closePopover();
          btn.click();
        },
      })));
      pear.popover(e.currentTarget, pear.el('div', { style: { width: '380px' } }, [pear.el('div', { class: 'oxide-eq' }, bands), chips]), { title: 'Equalizer · ' + preset });
    });
  },
  onConfig() {},
});
