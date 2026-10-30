// Folder browser: lists the folders of the machine (or Docker container) that runs Platno, so a media folder can be
// picked in any web browser on any system. window.pickFolder() resolves with the chosen absolute path, or null.
(() => {
  const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const FOLDER = '<svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M2.5 5.5a1 1 0 0 1 1-1h4l1.6 2h7.4a1 1 0 0 1 1 1v7.5a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1z"/></svg>';
  window.pickFolder = (start = '') => new Promise(resolve => {
    const dlg = document.createElement('dialog');
    dlg.className = 'fb';
    dlg.setAttribute('aria-labelledby', 'fb-title');
    dlg.innerHTML = `<h2 id="fb-title">Choose a folder</h2>
<p class="muted">These are the folders on the computer or container that runs Platno.</p>
<div class="fb-places" id="fb-places"></div>
<div class="fb-bar"><button type="button" class="secondary" id="fb-up" aria-label="Up one folder">Up</button><input id="fb-path" aria-label="Folder path" spellcheck="false" autocomplete="off"></div>
<ul class="fb-list" id="fb-list" aria-label="Folders"></ul>
<p class="fb-msg" id="fb-msg" role="status" aria-live="polite"></p>
<div class="fb-actions"><button type="button" class="secondary" id="fb-cancel">Cancel</button><button type="button" id="fb-use" disabled>Use this folder</button></div>`;
    document.body.appendChild(dlg);
    const $ = id => dlg.querySelector('#' + id);
    let current = '', parent = null, chosen = null, token = 0;
    const msg = (text, bad) => { $('fb-msg').textContent = text || ''; $('fb-msg').classList.toggle('bad', !!bad); };
    async function load(path) {
      const mine = ++token;
      msg('Loading…'); $('fb-use').disabled = true;
      try {
        const r = await api('/api/admin/browse-folders?path=' + encodeURIComponent(path || ''));
        if (mine !== token) return;
        current = r.path; parent = r.parent; $('fb-path').value = current; $('fb-up').disabled = !parent;
        $('fb-places').innerHTML = (r.places || []).map(p => `<button type="button" class="secondary" data-path="${esc(p.path)}" title="${esc(p.path)}">${esc(p.label)}</button>`).join('');
        $('fb-list').innerHTML = r.entries.length
          ? r.entries.map(e => `<li><button type="button" data-path="${esc(e.path)}">${FOLDER}<span>${esc(e.name)}</span></button></li>`).join('')
          : '<li class="fb-empty">No folders inside. You can still use this folder.</li>';
        msg(r.truncated ? 'Only the first 1000 folders are shown. Type a path to go deeper.' : '');
        $('fb-use').disabled = false;
        $('fb-list').scrollTop = 0;
      } catch (e) {
        if (mine !== token) return;
        msg(e.message || 'Could not open that folder.', true);
        $('fb-use').disabled = !current;
        $('fb-path').value = current;
      }
    }
    const finish = path => { chosen = path; dlg.close(); };
    dlg.addEventListener('click', e => { const b = e.target.closest('button[data-path]'); if (b) load(b.dataset.path); });
    $('fb-up').addEventListener('click', () => parent && load(parent));
    // Typing supersedes an in-flight directory response; do not overwrite the draft path.
    $('fb-path').addEventListener('input', () => { token++; $('fb-use').disabled = true; });
    $('fb-path').addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); load($('fb-path').value.trim()); } });
    $('fb-cancel').addEventListener('click', () => dlg.close());
    $('fb-use').addEventListener('click', () => current && finish(current));
    dlg.addEventListener('close', () => { token++; dlg.remove(); resolve(chosen); });
    dlg.showModal();
    load(start);
  });
})();
