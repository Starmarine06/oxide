// Play through a chosen audio output instead of the system default.
// Plugins > Custom output device > Choose output device.
pear.plugin('custom-output-device', {
  start(ctx, config) {
    const apply = async (deviceId) => {
      const id = deviceId || '';
      try {
        const ac = pear.audio.context;
        // Once the audio graph exists, sound leaves through the AudioContext.
        if (ac && ac.setSinkId) await ac.setSinkId(id);
        const v = pear.video;
        if (v && v.setSinkId) await v.setSinkId(id);
      } catch (e) {
        pear.toast('Could not switch output: ' + e.message, 5000);
      }
    };
    if (config.deviceId) {
      pear.whenReady(() => apply(config.deviceId));
      ctx.listen(document, 'loadedmetadata', (e) => { if (e.target instanceof HTMLMediaElement) apply(config.deviceId); }, true);
    }

    ctx.command('custom-output-device:choose', async () => {
      // Device names are only exposed after microphone permission is granted
      // once; without it the list shows generic names.
      try {
        const s = await navigator.mediaDevices.getUserMedia({ audio: true });
        s.getTracks().forEach((t) => t.stop());
      } catch (_) {}
      const devices = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audiooutput');
      const items = devices.map((d, i) => pear.el('button', {
        class: (config.deviceId || 'default') === d.deviceId ? 'on' : '',
        text: d.label || `Output ${i + 1}`,
        onclick: () => { apply(d.deviceId); ctx.saveOption('deviceId', d.deviceId); pear.closePopover(); pear.toast('Output: ' + (d.label || `Output ${i + 1}`)); },
      }));
      const anchor = document.querySelector(pear.sel.buttonHost) || document.body;
      pear.popover(anchor, pear.el('div', { class: 'oxide-list', style: { minWidth: '300px' } }, items), { title: 'Audio output' });
    });
  },
  onConfig() {},
});
