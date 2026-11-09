// Skips non-music parts of music videos (intros, outros, talking) using
// SponsorBlock's community database.
pear.plugin('sponsorblock', {
  start(ctx, config) {
    const categories = Array.isArray(config.categories) && config.categories.length
      ? config.categories
      : ['sponsor', 'intro', 'outro', 'interaction', 'selfpromo', 'music_offtopic'];
    let segments = [];
    let forId = null;

    const load = async (videoId) => {
      forId = videoId;
      segments = [];
      const url = `https://sponsor.ajay.app/api/skipSegments?videoID=${encodeURIComponent(videoId)}&categories=${encodeURIComponent(JSON.stringify(categories))}`;
      try {
        const res = await pear.invoke('http_get', { url });
        if (forId !== videoId || res.status !== 200 || !Array.isArray(res.json)) return;
        segments = res.json.map((s) => ({ start: s.segment[0], end: s.segment[1], category: s.category })).filter((s) => s.end - s.start > 0.5);
      } catch (_) {}
    };
    ctx.on('song', (s) => load(s.videoId));
    if (pear.song) load(pear.song.videoId);

    ctx.listen(document, 'timeupdate', (e) => {
      const v = e.target;
      if (!(v instanceof HTMLMediaElement) || v !== pear.video || !segments.length) return;
      const t = pear.position();
      const seg = segments.find((s) => t >= s.start && t < s.end - 0.3);
      if (!seg) return;
      pear.seekTo(Math.min(seg.end, pear.duration() - 0.1));
      pear.toast(`Skipped ${seg.category.replace('_', ' ')}`, 2000);
    }, true);
  },
});
