// Skips a song as soon as it is disliked (port of Pear Desktop's plugin).
pear.plugin('skip-disliked-songs', {
  start(ctx) {
    let skippedFor = null;
    const check = () => {
      const id = pear.song?.videoId;
      if (id && id !== skippedFor && pear.likeState() === 'DISLIKE') {
        skippedFor = id;
        pear.command('next');
      }
    };
    // Catch the click right away, and poll in case the state changes elsewhere.
    ctx.listen(document, 'click', () => ctx.timeout(check, 400), true);
    ctx.interval(check, 1500);
  },
});
