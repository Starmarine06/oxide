// Like / dislike / clear every song on an album or playlist page at once.
pear.plugin('album-actions', {
  start(ctx) {
    ctx.style(`
      .oxide-album-actions { display: flex; gap: 8px; margin: 12px 0 0; flex-wrap: wrap; }
      .oxide-album-actions button { display: inline-flex; align-items: center; gap: 6px; padding: 8px 14px; border-radius: 18px;
        border: 1px solid rgba(255,255,255,.2); background: transparent; color: #fff; font: 500 13px Roboto, sans-serif; cursor: pointer; }
      .oxide-album-actions button:hover { background: rgba(255,255,255,.1); }
      .oxide-album-actions button svg { width: 18px; height: 18px; }
    `);
    const rows = () => [...document.querySelectorAll('ytmusic-shelf-renderer ytmusic-responsive-list-item-renderer, ytmusic-playlist-shelf-renderer ytmusic-responsive-list-item-renderer')];
    const run = async (target) => {
      const list = rows();
      if (!list.length) { pear.toast('No songs found on this page.'); return; }
      let changed = 0;
      for (const row of list) {
        // Old rows: ytmusic-like-button-renderer[like-status]. New rows: view-model buttons with aria-pressed.
        const old = row.querySelector('ytmusic-like-button-renderer');
        const likeBtn = old ? old.querySelector('#button-shape-like button') : row.querySelector('like-button-view-model button');
        const dislikeBtn = old ? old.querySelector('#button-shape-dislike button') : row.querySelector('dislike-button-view-model button');
        if (!likeBtn && !dislikeBtn) continue;
        const status = old ? old.getAttribute('like-status')
          : likeBtn?.getAttribute('aria-pressed') === 'true' ? 'LIKE'
          : dislikeBtn?.getAttribute('aria-pressed') === 'true' ? 'DISLIKE' : 'INDIFFERENT';
        let btn = null;
        if (target === 'LIKE' && status !== 'LIKE') btn = likeBtn;
        if (target === 'DISLIKE' && status !== 'DISLIKE') btn = dislikeBtn;
        if (target === 'INDIFFERENT' && status === 'LIKE') btn = likeBtn;
        if (target === 'INDIFFERENT' && status === 'DISLIKE') btn = dislikeBtn;
        if (!btn) continue;
        btn.click();
        changed++;
        await new Promise((r) => setTimeout(r, 120));
      }
      pear.toast(changed ? `Updated ${changed} song${changed === 1 ? '' : 's'}.` : 'Nothing to change.');
    };
    const mk = (label, icon, target) => pear.el('button', { title: label, onclick: () => run(target) }, [pear.svgIcon(icon, 18), pear.el('span', { text: label })]);
    const bar = pear.el('div', { class: 'oxide-album-actions' }, [
      mk('Like all', 'M2 20h2c.6 0 1-.5 1-1v-9c0-.5-.4-1-1-1H2zm19.8-7.1c.1-.3.2-.6.2-.9v-1c0-1.1-.9-2-2-2h-5.5l.9-4.7v-.3c0-.4-.2-.8-.4-1.1L14 2 7.6 8.4C7.2 8.8 7 9.3 7 9.8V18c0 1.1.9 2 2 2h8c.8 0 1.5-.5 1.8-1.2z', 'LIKE'),
      mk('Dislike all', 'M22 4h-2c-.6 0-1 .5-1 1v9c0 .5.4 1 1 1h2zM2.2 11.1c-.1.3-.2.6-.2.9v1c0 1.1.9 2 2 2h5.5l-.9 4.7v.3c0 .4.2.8.4 1.1L10 22l6.4-6.4c.4-.4.6-.9.6-1.4V6c0-1.1-.9-2-2-2H7c-.8 0-1.5.5-1.8 1.2z', 'DISLIKE'),
      mk('Clear ratings', 'M19 6.4 17.6 5 12 10.6 6.4 5 5 6.4 10.6 12 5 17.6 6.4 19 12 13.4 17.6 19 19 17.6 13.4 12z', 'INDIFFERENT'),
    ]);
    const place = () => {
      const isList = /^\/(playlist|browse\/MPRE)/.test(location.pathname) || location.search.includes('list=');
      const header = document.querySelector('ytmusic-responsive-header-renderer .action-buttons, ytmusic-detail-header-renderer .action-buttons, ytmusic-editable-playlist-detail-header-renderer .action-buttons');
      if (isList && header) { if (bar.parentElement !== header.parentElement) header.after(bar); }
      else bar.remove();
    };
    ctx.watch('ytmusic-responsive-header-renderer, ytmusic-detail-header-renderer, ytmusic-editable-playlist-detail-header-renderer', place);
    ctx.listen(document, 'yt-navigate-finish', () => ctx.timeout(place, 300));
    ctx.cleanup(() => bar.remove());
  },
});
