// Lowers CPU use by throttling background timers and trimming YouTube
// Music's DOM work (vendored scripts by CY Fung, MIT, via Pear Desktop).
pear.plugin('performance-improvement', {
  reloadOnToggle: true,
  start() {
    try { injectRm3(); } catch (e) { console.warn('[oxide] rm3', e); }
    try {
      const gl = (() => { try { const c = document.createElement('canvas'); return !!(c.getContext('webgl') || c.getContext('experimental-webgl')); } catch (_) { return false; } })();
      gl ? injectCpuTamerByAnimationFrame(null) : injectCpuTamerByDomMutation(null);
    } catch (e) { console.warn('[oxide] cpu tamer', e); }
  },
});
