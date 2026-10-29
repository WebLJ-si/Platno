/* Hand editing of a title: name, year, genres, description, cover and per-file season/episode.
   Bundled classic script; uses api/post/esc/$, grouped(), poster() and refreshFiles() from index.html. */
(() => {
  'use strict';
  const LIMIT = 8 * 1024 * 1024, TYPES = ['image/jpeg', 'image/png', 'image/webp'];
  let state = null;
  const canEdit = () => typeof authUser !== 'undefined' && authUser && ['owner', 'admin'].includes(authUser.role);
  const say = (id, text, bad) => { const el = $('#' + id); if (el) { el.textContent = text; el.dataset.bad = bad ? '1' : ''; } };
  const base = path => path.split(/[\\/]/).pop();

  function episodeRows(r) {
    const files = [...r.files].sort((a, b) => (a.result.season ?? 1e6) - (b.result.season ?? 1e6) || (a.result.episodes?.[0] ?? 1e6) - (b.result.episodes?.[0] ?? 1e6) || a.path.localeCompare(b.path));
    return `<h2>Episodes</h2><p class="muted">Fix the season, the episode number (use 3, 4 for a double episode) and the title of each file. Each row saves on its own.</p><div class="edit-episodes">${files.map((f, n) => `<div class="edit-episode" data-path="${esc(f.path)}"><div class="path">${esc(base(f.path))}</div><div class="edit-episode-fields"><label>Season<input type="number" min="0" max="1000" inputmode="numeric" class="ee-season" value="${esc(f.result.season ?? '')}"></label><label>Episode<input class="ee-numbers" inputmode="numeric" value="${esc((f.result.episodes || []).join(', '))}"></label><label>Title<input class="ee-title" maxlength="200" value="${esc(f.result.episode_title || '')}"></label><button class="secondary" data-row="${n}">Save episode</button></div><p class="edit-status" role="status"></p></div>`).join('')}</div>`;
  }
  function render() {
    const { r, kind } = state, current = state.poster === undefined ? r : { ...r, poster: state.poster || null };
    return `<div class="edit-title"><div class="row" style="justify-content:space-between;align-items:start"><h1>Edit details</h1><button class="secondary" id="edit-back">Back</button></div>
<p class="muted">Changes apply to all ${r.files.length} file${r.files.length === 1 ? '' : 's'} of this title and are protected from rescans. To link the title to TMDB or TVmaze instead, use Edit identification in the file details.</p>
<div class="edit-grid"><div class="edit-cover"><div id="edit-cover-preview">${poster(current)}</div><label class="edit-file">Cover picture<input type="file" id="edit-cover-file" accept="image/jpeg,image/png,image/webp"></label><p class="muted">JPEG, PNG or WebP, up to 8 MB. A poster about 2:3 looks best.</p><button class="secondary" id="edit-cover-remove">Remove custom cover</button><p class="edit-status" id="edit-cover-msg" role="status"></p></div>
<div class="edit-fields"><label>Title<input id="edit-name" maxlength="200" value="${esc(r.title || '')}"></label><label>Year<input id="edit-year" maxlength="4" inputmode="numeric" value="${esc(String(r.year || '').slice(0, 4))}"></label><label>Genres (separate with commas)<input id="edit-genres" value="${esc((Array.isArray(r.genres) ? r.genres : []).join(', '))}"></label><label>Description<textarea id="edit-description" rows="6" maxlength="4000">${esc(String(r.description || '').replace(/<[^>]*>/g, ''))}</textarea></label></div></div>
<div class="toolbar"><button id="edit-save">Save changes</button></div><p class="edit-status" id="edit-msg" role="status"></p>${kind === 'tv' ? episodeRows(r) : ''}</div>`;
  }
  function open(kind, index) {
    const r = grouped(kind)[index];
    if (!r || !canEdit()) return;
    state = { kind, r, poster: undefined, key: r.provider + ':' + r.provider_id };
    const d = $('#dialog');
    if (d.open) d.close();
    d.innerHTML = render();
    d.showModal();
    $('#edit-back').onclick = reopen;
    $('#edit-save').onclick = save;
    $('#edit-cover-file').onchange = upload;
    $('#edit-cover-remove').onclick = () => { state.poster = ''; $('#edit-cover-preview').innerHTML = poster({ ...state.r, poster: null }); say('edit-cover-msg', 'The custom cover is removed when you save. A cover from the provider, if any, comes back.'); };
    d.querySelectorAll('.edit-episode button').forEach(b => { b.onclick = () => saveEpisode(b.closest('.edit-episode')); });
    $('#edit-name').focus();
  }
  function reopen() {
    if (!state) return;
    const { kind, key } = state, i = grouped(kind).findIndex(g => g.provider + ':' + g.provider_id === key);
    if (i >= 0) detail(kind, i); else $('#dialog').close();
  }
  async function upload(event) {
    const file = event.target.files[0];
    if (!file) return;
    if (!TYPES.includes(file.type)) { say('edit-cover-msg', 'Use a JPEG, PNG or WebP picture.', true); return; }
    if (file.size > LIMIT) { say('edit-cover-msg', 'That picture is larger than 8 MB.', true); return; }
    say('edit-cover-msg', 'Uploading…');
    try {
      const saved = await api('/api/admin/covers', { method: 'POST', headers: { 'Content-Type': file.type }, body: file });
      state.poster = saved.url;
      $('#edit-cover-preview').innerHTML = poster({ ...state.r, poster: saved.url });
      say('edit-cover-msg', 'Uploaded. Press Save changes to use it.');
    } catch (e) { say('edit-cover-msg', e.message, true); }
  }
  async function save() {
    const button = $('#edit-save');
    button.disabled = true; say('edit-msg', 'Saving…');
    const body = {
      paths: state.r.files.map(f => f.path), title: $('#edit-name').value, year: $('#edit-year').value.trim(),
      description: $('#edit-description').value, genres: $('#edit-genres').value.split(',').map(x => x.trim()).filter(Boolean)
    };
    if (state.poster !== undefined) body.poster = state.poster;
    try {
      await post('/api/admin/title', body);
      await refreshFiles();
      reopen();
    } catch (e) { say('edit-msg', e.message, true); button.disabled = false; }
  }
  async function saveEpisode(row) {
    const numbers = row.querySelector('.ee-numbers').value.split(/[ ,;]+/).filter(Boolean).map(Number), seasonText = row.querySelector('.ee-season').value.trim();
    const status = row.querySelector('.edit-status');
    if (numbers.some(n => !Number.isInteger(n) || n < 1)) { status.textContent = 'Use whole episode numbers such as 4 or 3, 4.'; status.dataset.bad = '1'; return; }
    if (seasonText && !/^\d{1,4}$/.test(seasonText)) { status.textContent = 'Use a whole season number, or leave it empty.'; status.dataset.bad = '1'; return; }
    status.textContent = 'Saving…'; status.dataset.bad = '';
    try {
      await post('/api/admin/episode', { path: row.dataset.path, season: seasonText === '' ? null : Number(seasonText), episodes: numbers, title: row.querySelector('.ee-title').value });
      status.textContent = 'Saved.';
      refreshFiles();
    } catch (e) { status.textContent = e.message; status.dataset.bad = '1'; }
  }
  window.PlatnoEdit = { open, canEdit };
})();
