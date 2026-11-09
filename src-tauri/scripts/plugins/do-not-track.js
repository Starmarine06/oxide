// Blocks third-party ad and analytics requests (DoubleClick, Google Ads,
// Analytics, Tag Manager, Play logging). YouTube's own endpoints are left
// alone on purpose: the player waits on several of them (stats, logging,
// attestation) and stalls if they fail, which also breaks history.
pear.plugin('do-not-track', {
  reloadOnToggle: true,
  start() {
    const BLOCK = [
      /^https?:\/\/([a-z0-9-]+\.)*doubleclick\.net\//,
      /^https?:\/\/([a-z0-9-]+\.)*googleadservices\.com\//,
      /^https?:\/\/([a-z0-9-]+\.)*googlesyndication\.com\//,
      /^https?:\/\/([a-z0-9-]+\.)*google-analytics\.com\//,
      /^https?:\/\/([a-z0-9-]+\.)*googletagmanager\.com\//,
      /^https?:\/\/([a-z0-9-]+\.)*adservice\.google\.com\//,
      /^https?:\/\/play\.google\.com\/log/,
    ];
    const blocked = (url) => {
      try {
        const s = new URL(String(url instanceof Request ? url.url : url), location.href).href;
        return BLOCK.some((r) => r.test(s));
      } catch (_) { return false; }
    };

    const fetch0 = window.fetch;
    window.fetch = function (input, init) {
      if (blocked(input)) return Promise.resolve(new Response(null, { status: 204 }));
      return fetch0.call(this, input, init);
    };
    if (navigator.sendBeacon) {
      const beacon0 = navigator.sendBeacon.bind(navigator);
      navigator.sendBeacon = (url, data) => (blocked(url) ? true : beacon0(url, data));
    }
    const open0 = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (method, url, ...rest) {
      // Point blocked requests at an empty data URL so callers still get a
      // normal load event.
      return open0.call(this, method, blocked(url) ? 'data:text/plain,' : url, ...rest);
    };
    const src = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
    Object.defineProperty(HTMLImageElement.prototype, 'src', {
      configurable: true,
      get() { return src.get.call(this); },
      set(v) { src.set.call(this, blocked(v) ? 'data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==' : v); },
    });
  },
});
