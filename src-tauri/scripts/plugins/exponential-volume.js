// Exponential volume curve so low volumes are easier to pick
// (port of Pear Desktop's plugin; exponent 3 matches PulseAudio).
pear.plugin('exponential-volume', {
  reloadOnToggle: true,
  start() {
    const EXPONENT = 3;
    const stored = new WeakMap();
    const desc = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, 'volume');
    Object.defineProperty(HTMLMediaElement.prototype, 'volume', {
      configurable: true,
      get() {
        const low = desc.get.call(this) ?? 0;
        const calculated = low ** (1 / EXPONENT);
        const original = stored.get(this) ?? 0;
        // Return the exact value that was set when it still matches, so code
        // comparing volumes doesn't see rounding drift.
        return Math.abs(original - calculated) < 0.01 ? original : calculated;
      },
      set(value) {
        stored.set(this, value);
        desc.set.call(this, value ** EXPONENT);
      },
    });
    // Re-apply the current volume through the new curve once the player exists.
    pear.waitFor('#movie_player').then((api) => {
      const sync = () => {
        if (!api.getVolume) return setTimeout(sync, 200);
        api.setVolume(api.getVolume());
      };
      sync();
    });
  },
});
