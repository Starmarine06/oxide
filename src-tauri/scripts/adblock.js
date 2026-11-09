// Ad blocker. Starts before the page's scripts so the hooks are in place
// before any player response is parsed.
// Layer 1: strip ad fields from player responses (JSON.parse, Response.json
//          and the inline ytInitialPlayerResponse).
// Layer 2: if an ad still plays, mute it, jump to its end and press skip.
// Layer 3: hide Premium upsell banners.
pear.plugin('ad-blocker', {
  reloadOnToggle: true,
  start(ctx) {
    const AD_KEYS = ['adPlacements', 'playerAds', 'adSlots', 'adBreakHeartbeatParams', 'adBreakParams'];
    const hasAds = (o) => o && typeof o === 'object' && AD_KEYS.some((k) => k in o);

    function prune(obj) {
      if (!obj || typeof obj !== 'object') return obj;
      if (hasAds(obj)) for (const k of AD_KEYS) delete obj[k];
      if (obj.playerResponse && typeof obj.playerResponse === 'object') prune(obj.playerResponse);
      if (Array.isArray(obj)) for (const item of obj) if (item && item.playerResponse) prune(item.playerResponse);
      return obj;
    }

    const originalParse = JSON.parse;
    JSON.parse = function (...args) {
      const out = originalParse.apply(this, args);
      try {
        if (out && typeof out === 'object' && (hasAds(out) || hasAds(out.playerResponse))) prune(out);
      } catch (_) {}
      return out;
    };

    const originalJson = Response.prototype.json;
    Response.prototype.json = function () {
      return originalJson.call(this).then((out) => {
        try { prune(out); } catch (_) {}
        return out;
      });
    };

    let initialResponse;
    try {
      Object.defineProperty(window, 'ytInitialPlayerResponse', {
        configurable: true,
        get: () => initialResponse,
        set: (v) => { initialResponse = prune(v); },
      });
    } catch (_) {}

    ctx.interval(() => {
      const player = document.querySelector('#movie_player');
      if (!player || !player.classList.contains('ad-showing')) return;
      const v = player.querySelector('video');
      if (v) {
        if (!v.muted) { v.muted = true; v.dataset.oxideMuted = '1'; }
        if (Number.isFinite(v.duration) && v.duration > 0) v.currentTime = v.duration;
      }
      const skip = document.querySelector('.ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-skip-ad-button');
      if (skip) skip.click();
    }, 300);

    // Unmute once the ad is gone, but only if we were the ones who muted.
    ctx.listen(document, 'loadedmetadata', (e) => {
      const v = e.target;
      const player = document.querySelector('#movie_player');
      if (v instanceof HTMLMediaElement && v.dataset.oxideMuted && player && !player.classList.contains('ad-showing')) {
        v.muted = false;
        delete v.dataset.oxideMuted;
      }
    }, true);

    ctx.style(`
      ytmusic-mealbar-promo-renderer,
      ytmusic-statement-banner-renderer,
      ytmusic-popup-container tp-yt-paper-dialog:has(ytmusic-mealbar-promo-renderer),
      .ytp-ad-module, .ytp-ad-overlay-container, #player-ads,
      ytmusic-premium-upsell-renderer { display: none !important; }
    `);
  },
});
