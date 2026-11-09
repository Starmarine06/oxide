// Audio visualizer drawn over the album art (or behind the whole player
// page). Three styles: Bars, Radial and Aurora, coloured from the album art.
pear.plugin('visualizer', {
  start(ctx, config) {
    const mode = () => config.mode || 'bars';
    const placement = config.placement === 'background' ? 'background' : 'art';
    const BANDS = mode() === 'radial' ? 120 : mode() === 'aurora' ? 48 : 72;
    const MIN_HZ = 32, MAX_HZ = 16000;

    ctx.style(`
      .oxide-viz { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; z-index: 5; }
      .oxide-viz.bg { position: fixed; inset: var(--menu-bar-height, 0px) 0 0 0; height: auto; z-index: 0; opacity: .9; }
      ytmusic-player-page.oxide-viz-host #main-panel, ytmusic-player-page.oxide-viz-host .side-panel { position: relative; z-index: 1; }
    `);

    const canvas = pear.el('canvas', { class: 'oxide-viz' + (placement === 'background' ? ' bg' : ''), 'aria-hidden': 'true' });
    const g = canvas.getContext('2d');
    if (placement === 'background') {
      ctx.watch('ytmusic-player-page', (page) => { page.classList.add('oxide-viz-host'); page.prepend(canvas); });
      ctx.cleanup(() => document.querySelector('ytmusic-player-page')?.classList.remove('oxide-viz-host'));
    } else {
      ctx.watch('ytmusic-player#player', (player) => { if (!player.style.position) player.style.position = 'relative'; player.append(canvas); });
    }
    ctx.cleanup(() => canvas.remove());

    // ---- colours ---------------------------------------------------------
    let colors = [[232, 100, 44], [255, 170, 90], [196, 71, 31]];
    const loadColors = (song) => {
      if (!config.albumColors) return;
      pear.palette(song?.artUrl || document.querySelector('#song-image img')?.src).then((c) => {
        if (c.length) colors = [c[0], c[1] || c[0], c[2] || c[1] || c[0]].map(lift);
      });
    };
    // Keep colours bright enough to read on black.
    const lift = ([r, gg, b]) => {
      const m = Math.max(r, gg, b);
      // Near-black or grey art: use the Oxide accent instead.
      if (m < 30 || m - Math.min(r, gg, b) < 12) return [232, 100, 44];
      const k = m < 150 ? 150 / Math.max(m, 1) : 1;
      return [Math.min(255, r * k) | 0, Math.min(255, gg * k) | 0, Math.min(255, b * k) | 0];
    };
    const rgba = (c, a) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;
    ctx.on('song', loadColors);
    loadColors(pear.song);

    // ---- analysis --------------------------------------------------------
    let freq = null;
    const levels = new Float32Array(BANDS);
    const peaks = new Float32Array(BANDS);
    const peakHold = new Float32Array(BANDS);
    let bass = 0;
    let edges = null;

    function bandEdges(an) {
      const nyquist = an.context.sampleRate / 2;
      const bins = an.frequencyBinCount;
      const out = [];
      for (let i = 0; i <= BANDS; i++) {
        const hz = MIN_HZ * Math.pow(MAX_HZ / MIN_HZ, i / BANDS);
        out.push(Math.min(bins - 1, Math.max(1, Math.round((hz / nyquist) * bins))));
      }
      return out;
    }

    function analyse(dt) {
      const an = pear.audio.analyser;
      const v = pear.video;
      const playing = an && v && !v.paused;
      if (playing) {
        if (!freq || freq.length !== an.frequencyBinCount) { freq = new Uint8Array(an.frequencyBinCount); edges = bandEdges(an); }
        an.getByteFrequencyData(freq);
      }
      let bassSum = 0;
      for (let i = 0; i < BANDS; i++) {
        let target = 0;
        if (playing) {
          const a = edges[i], b = Math.max(edges[i + 1], a + 1);
          let m = 0;
          for (let k = a; k < b; k++) if (freq[k] > m) m = freq[k];
          // Tilt up the highs, which are naturally quieter.
          target = Math.min(1, Math.pow(m / 255, 1.6) * (1 + (i / BANDS) * 0.6));
        }
        // Fast attack, slow release.
        const rate = target > levels[i] ? 0.55 : 0.12;
        levels[i] += (target - levels[i]) * Math.min(1, rate * dt * 60);
        if (levels[i] >= peaks[i]) { peaks[i] = levels[i]; peakHold[i] = 0.35; }
        else if ((peakHold[i] -= dt) <= 0) peaks[i] = Math.max(levels[i], peaks[i] - dt * 0.6);
        if (i < BANDS * 0.12) bassSum += levels[i];
      }
      const b = bassSum / Math.max(1, Math.round(BANDS * 0.12));
      bass += (b - bass) * (b > bass ? 0.5 : 0.08);
      return playing || levels.some((x) => x > 0.002);
    }

    // ---- drawing ---------------------------------------------------------
    function drawBars(w, h) {
      const gap = Math.max(2, w / BANDS * 0.28);
      const bw = (w - gap * (BANDS - 1)) / BANDS;
      const base = h * 0.78;
      const maxH = h * 0.62;
      const grad = g.createLinearGradient(0, base - maxH, 0, base);
      grad.addColorStop(0, rgba(colors[1], 0.95));
      grad.addColorStop(1, rgba(colors[0], 0.95));
      // Fade the art slightly at the bottom so bars read clearly.
      const shade = g.createLinearGradient(0, h * 0.45, 0, h);
      shade.addColorStop(0, 'rgba(0,0,0,0)');
      shade.addColorStop(1, 'rgba(0,0,0,0.55)');
      g.fillStyle = shade;
      g.fillRect(0, h * 0.45, w, h * 0.55);
      for (let i = 0; i < BANDS; i++) {
        // Mirror: lows in the middle, highs at the edges.
        const idx = Math.abs(i - (BANDS - 1) / 2) * 2 | 0;
        const lv = levels[Math.min(BANDS - 1, idx)];
        const pk = peaks[Math.min(BANDS - 1, idx)];
        const x = i * (bw + gap);
        const bh = Math.max(2, lv * maxH);
        const r = Math.min(bw / 2, 4);
        g.fillStyle = grad;
        g.beginPath();
        g.roundRect(x, base - bh, bw, bh, [r, r, 0, 0]);
        g.fill();
        // Reflection.
        g.fillStyle = rgba(colors[0], 0.18);
        g.fillRect(x, base + 2, bw, bh * 0.35);
        // Peak cap.
        g.fillStyle = rgba([255, 255, 255], 0.85);
        g.fillRect(x, base - pk * maxH - 5, bw, 2);
      }
    }

    let rot = 0;
    function drawRadial(w, h, dt) {
      const cx = w / 2, cy = h / 2;
      const R = Math.min(w, h) * (0.24 + bass * 0.035);
      const maxLen = Math.min(w, h) * 0.2;
      rot += dt * (0.08 + bass * 0.5);
      // Soft halo pulsing with the bass.
      const halo = g.createRadialGradient(cx, cy, R * 0.6, cx, cy, R * 1.9);
      halo.addColorStop(0, rgba(colors[0], 0.22 + bass * 0.35));
      halo.addColorStop(1, rgba(colors[0], 0));
      g.fillStyle = halo;
      g.fillRect(0, 0, w, h);
      g.save();
      g.translate(cx, cy);
      g.rotate(rot);
      g.lineCap = 'round';
      const lw = Math.max(2, (Math.PI * 2 * R) / BANDS * 0.5);
      g.lineWidth = lw;
      for (let i = 0; i < BANDS; i++) {
        // Mirror around the circle so it's symmetric.
        const idx = i < BANDS / 2 ? i * 2 : (BANDS - 1 - i) * 2;
        const lv = levels[Math.min(BANDS - 1, idx)];
        const a = (i / BANDS) * Math.PI * 2;
        const len = 3 + lv * maxLen;
        const c = colors[i % 2 ? 1 : 0];
        g.strokeStyle = rgba(c, 0.55 + lv * 0.45);
        g.beginPath();
        g.moveTo(Math.cos(a) * R, Math.sin(a) * R);
        g.lineTo(Math.cos(a) * (R + len), Math.sin(a) * (R + len));
        g.stroke();
      }
      g.restore();
      // Thin inner ring.
      g.strokeStyle = rgba([255, 255, 255], 0.35);
      g.lineWidth = 1.5;
      g.beginPath();
      g.arc(cx, cy, R - 4, 0, Math.PI * 2);
      g.stroke();
    }

    let t = 0;
    function drawAurora(w, h, dt) {
      t += dt;
      g.globalCompositeOperation = 'lighter';
      const layers = [
        { c: colors[0], amp: 0.55, y: 0.72, speed: 0.35, alpha: 0.45, from: 0 },
        { c: colors[1], amp: 0.45, y: 0.66, speed: -0.25, alpha: 0.35, from: BANDS / 3 },
        { c: colors[2], amp: 0.38, y: 0.6, speed: 0.18, alpha: 0.3, from: (BANDS / 3) * 2 },
      ];
      for (const L of layers) {
        const pts = 18;
        g.beginPath();
        g.moveTo(0, h);
        let prevX = 0, prevY = h * L.y;
        for (let p = 0; p <= pts; p++) {
          const x = (p / pts) * w;
          const band = Math.floor(L.from + (p / pts) * (BANDS / 3)) % BANDS;
          const wobble = Math.sin(p * 0.7 + t * L.speed * 4) * 0.04;
          const y = h * (L.y - levels[band] * L.amp * 0.6 - wobble - bass * 0.08);
          if (p === 0) g.lineTo(x, y);
          else g.quadraticCurveTo(prevX, prevY, (prevX + x) / 2, (prevY + y) / 2);
          prevX = x; prevY = y;
        }
        g.lineTo(w, prevY);
        g.lineTo(w, h);
        g.closePath();
        const grad = g.createLinearGradient(0, h * 0.2, 0, h);
        grad.addColorStop(0, rgba(L.c, L.alpha));
        grad.addColorStop(1, rgba(L.c, 0));
        g.fillStyle = grad;
        g.fill();
      }
      g.globalCompositeOperation = 'source-over';
    }

    // ---- loop ------------------------------------------------------------
    let raf = 0, last = performance.now(), running = false;
    const visible = () => {
      if (document.hidden) return false;
      const state = document.querySelector('ytmusic-app-layout')?.getAttribute('player-ui-state');
      return state === 'PLAYER_PAGE_OPEN' || state === 'FULLSCREEN';
    };
    function frame(now) {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const w = Math.max(1, Math.round(rect.width * dpr)), h = Math.max(1, Math.round(rect.height * dpr));
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
      const active = analyse(dt);
      g.clearRect(0, 0, w, h);
      if (active) {
        if (mode() === 'radial') drawRadial(w, h, dt);
        else if (mode() === 'aurora') drawAurora(w, h, dt);
        else drawBars(w, h);
      }
      if (visible() && (active || !pear.video?.paused)) raf = requestAnimationFrame(frame);
      else { running = false; g.clearRect(0, 0, w, h); }
    }
    const kick = () => {
      if (running || !visible()) return;
      running = true;
      last = performance.now();
      raf = requestAnimationFrame(frame);
    };
    pear.audio.ready().then(kick);
    ctx.listen(document, 'play', kick, true);
    ctx.listen(document, 'visibilitychange', kick);
    ctx.watch('ytmusic-app-layout', (layout) => ctx.observe(layout, { attributes: true, attributeFilter: ['player-ui-state'] }, kick));
    ctx.interval(kick, 1000);
    ctx.cleanup(() => cancelAnimationFrame(raf));
  },
});
