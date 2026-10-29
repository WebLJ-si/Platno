/* Owner-only "Updates" panel in Settings. Nothing is checked until the owner presses a button. */
(() => {
  'use strict';
  const when = ts => (ts ? new Date(ts * 1000).toLocaleString() : 'not yet');
  const say = text => { const el = $('#update-status'); if (el) el.textContent = text; };
  let before = null;
  function render(s) {
    const body = $('#update-body');
    if (!body) return;
    before = s.running_since;
    const latest = s.latest, newer = s.update_available;
    const state = !s.enabled ? '' : !latest ? '<p class="muted">Nobody has been asked yet. Press the button to look for a newer version on GitHub.</p>'
      : newer ? `<p><b>Version ${esc(latest.version)} is available.</b> Published ${esc(latest.published_at ? new Date(latest.published_at).toLocaleDateString() : '')}${latest.page_url ? ` · <a href="${esc(latest.page_url)}" target="_blank" rel="noopener noreferrer">Release page</a>` : ''}</p>${latest.notes ? `<pre class="update-notes">${esc(latest.notes)}</pre>` : ''}`
      : `<p>You are up to date (last checked ${esc(when(s.checked_at))}).</p>`;
    let action = '';
    if (newer && s.can_install) action = '<button id="update-install">Install and restart</button><p class="muted">Platno downloads the official release from GitHub, checks it against the published checksum, keeps the old program next to it as <code>.previous</code> and restarts. The library is unavailable for a few seconds.</p>';
    else if (newer && s.instructions) action = `<p class="update-instructions">${esc(s.instructions)}</p>`;
    else if (newer) action = '<p class="muted">This release has no download for this computer. Open the release page to see what is available.</p>';
    else if (!s.enabled && s.instructions) action = `<p class="muted">${esc(s.instructions)}</p>`;
    body.innerHTML = `<p>Installed: <b>Platno ${esc(s.current)}</b> <span class="muted">(${esc(s.os)} ${esc(s.arch)})</span></p>${state}${s.enabled ? '<button class="secondary" id="update-check">Check for updates</button>' : ''}${action}<p id="update-status" role="status" class="muted"></p>`;
    $('#update-check')?.addEventListener('click', check);
    $('#update-install')?.addEventListener('click', () => install(latest.version));
  }
  async function mount() {
    try { render(await api('/api/owner/update')); } catch (e) { const body = $('#update-body'); if (body) body.textContent = e.message; }
  }
  async function check() {
    const button = $('#update-check');
    if (button) button.disabled = true;
    say('Asking GitHub…');
    try { render(await post('/api/owner/update/check', {})); } catch (e) { say(e.message); if (button) button.disabled = false; }
  }
  async function install(version) {
    const button = $('#update-install');
    if (button) button.disabled = true;
    say('Downloading and checking the update…');
    try {
      await post('/api/owner/update/apply', { version });
      say('Installed. Restarting the server…');
      for (let i = 0; i < 60; i++) {
        await new Promise(r => setTimeout(r, 1500));
        try { const s = await api('/api/owner/update'); if (s.running_since !== before) { say('Updated to ' + s.current + '. Reloading…'); location.reload(); return; } } catch { /* the server is restarting */ }
      }
      say('The server did not come back within 90 seconds. Check it on the computer where it runs.');
    } catch (e) { say(e.message); if (button) button.disabled = false; }
  }
  window.PlatnoUpdate = { mount, check, install };
})();
