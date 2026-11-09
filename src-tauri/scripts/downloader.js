// Download button for the current song. The work happens in Rust (yt-dlp);
// this script only starts it and shows progress.
pear.plugin('downloader', {
  start(ctx) {
    function startDownload() {
      const song = pear.song;
      if (!song) { pear.toast('Play a song first.'); return; }
      pear.invoke('download', { videoId: song.videoId, label: `${song.artist} - ${song.title}` })
        .catch((e) => pear.toast('Download failed: ' + e, 5000));
    }

    ctx.on('download', (e) => {
      const label = e.label ? ` ${e.label}` : '';
      switch (e.status) {
        case 'start': pear.toast(`Downloading${label}…`, 0); break;
        case 'setup': pear.toast(e.message + '…', 0); break;
        case 'progress': pear.toast(`Downloading… ${e.percent}%`, 0); break;
        case 'done': pear.toast('Download finished.', 4000); break;
        case 'error': pear.toast('Download failed: ' + e.message, 8000); break;
      }
    });

    ctx.playerButton('Download', 'M5 20h14v-2H5v2zM19 9h-4V3H9v6H5l7 7 7-7z', startDownload);
  },
});
