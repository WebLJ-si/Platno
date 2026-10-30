/* Platno first-run guide and API-key help. Bundled classic script: shares api/post/esc/$ and authUser with index.html. */
(() => {
  'use strict';
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches || document.body.classList.contains('reduce-motion');
  const CHECK = '<svg class="sg-check" viewBox="0 0 20 20" aria-hidden="true"><path d="M4.5 10.5l3.7 3.7 7.3-8.4"/></svg>';
  const DASH = '<svg class="sg-check" viewBox="0 0 20 20" aria-hidden="true"><path d="M5.5 10h9"/></svg>';
  const WARN = '<svg class="sg-warn-icon" viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3.2l7.6 13.3H2.4z"/><path d="M10 8.4v3.6M10 14.2v.4"/></svg>';
  const KIND = { mixed: 'Automatic · mixed media', movie: 'Movies', tv: 'TV shows', video: 'Other videos', other: 'Other' };
  const KIND_HELP = {
    mixed: 'One shared folder is enough. Movies and series are recognized automatically; other videos stay browsable by folder.',
    movie: 'Feature films. One folder per movie or a flat folder both work.',
    tv: 'Series, usually Show / Season 01 / episode files.',
    video: 'Home videos, clips and anything that is not a movie or an episode.',
    other: 'Anything else you want kept apart.'
  };
  const CHECK_TEXT = { ok: 'Found', missing: 'Not found', unreadable: 'No permission', timeout: 'Not responding', not_folder: 'Not a folder', invalid: 'Not a valid path', checking: 'Checking' };
  const ENCODER = {
    h264_videotoolbox: 'Apple VideoToolbox (the Mac graphics chip)',
    h264_nvenc: 'NVIDIA NVENC',
    h264_qsv: 'Intel Quick Sync',
    libx264: 'Software (processor), works everywhere'
  };
  const KEYS = {
    tmdb: {
      name: 'TMDB', title: 'Movie information (TMDB)', where: 'Settings, then Metadata and subtitle providers, then TMDB Read Access Token',
      env: 'TMDB_TOKEN', provider: 'tmdb_configured', override: 'tmdb_environment_override', save: '/api/owner/providers/tmdb',
      test: '/api/metadata/search?kind=movie&q=The%20Matrix', rejected: 'tmdb_key_rejected',
      gives: ['Movie posters, ratings and full descriptions', 'Searching for the right movie when a file is matched wrongly', 'Movie collections such as trilogies'],
      loses: ['No posters, ratings or full descriptions for movies', 'No movie collections such as trilogies', 'No online search to correct a wrong movie match'],
      still: 'TV shows keep working: they use TVmaze, which needs no key. Movies are still named from the free Wikidata database or from their file names, and you can edit names, covers and episodes by hand.',
      steps: [
        { t: 'Create a free TMDB account.', href: 'https://www.themoviedb.org/signup', link: 'Open themoviedb.org/signup' },
        { t: 'Open your API settings and request an API key. Choose “Developer”, personal use, and describe it as a private home media server.', href: 'https://www.themoviedb.org/settings/api', link: 'Open API settings' },
        { t: 'On that page copy the long “API Read Access Token”. It is a long text starting with “eyJ”. Do not copy the short “API Key”.' },
        { t: 'Paste it below and press Save and test.' }
      ]
    },
    opensubtitles: {
      name: 'OpenSubtitles', title: 'Subtitle search (OpenSubtitles)', where: 'Settings, then Metadata and subtitle providers, then OpenSubtitles API key',
      env: 'OPENSUBTITLES_API_KEY', provider: 'opensubtitles_configured', override: 'opensubtitles_environment_override', save: '/api/owner/providers/opensubtitles',
      test: '/api/subtitles/search?q=The%20Matrix', rejected: 'opensubtitles_key_rejected',
      gives: ['Searching online subtitles by title in the Subtitles page', 'Seeing language, rating and download counts before you choose'],
      loses: ['The online search in the Subtitles page stays switched off'],
      still: 'Subtitles next to your videos, subtitles inside the video files and local speech-based subtitle sync keep working.',
      steps: [
        { t: 'Create a free account on opensubtitles.com (the .com site, not .org).', href: 'https://www.opensubtitles.com/', link: 'Open opensubtitles.com' },
        { t: 'Open the API consumers page and create a new consumer. Any name works, for example “Platno”.', href: 'https://www.opensubtitles.com/consumers', link: 'Open API consumers' },
        { t: 'Copy the API key shown for that consumer.' },
        { t: 'Paste it below and press Save and test.' }
      ]
    }
  };
  const STEPS = [
    { id: 'welcome', name: 'Welcome', blurb: '' },
    { id: 'libraries', name: 'Media folders', blurb: 'Where your movies, shows and videos live', skip: true,
      loses: ['Your library stays empty: there is nothing for the scanner to read.', 'Add folders later in Settings, under Scan & settings.'] },
    { id: 'places', name: 'Other locations', blurb: 'Data, conversion files and FFmpeg', skip: true,
      loses: ['Temporary conversion files stay in the system temporary folder. On a small system disk a long stream can fill it.', 'Nothing else changes. You can pick a folder later in Settings, under Conversion folder.'] },
    { id: 'performance', name: 'Memory & graphics', blurb: 'How many conversions and which encoder', skip: true,
      loses: ['Defaults stay: software encoding on the processor, balanced quality, up to 4 conversions at once.', 'Videos that need converting may use more processor time than necessary. Change this later in Settings.'] },
    { id: 'scan', name: 'First scan', blurb: 'Read your folders and match titles', skip: true,
      loses: ['Your library stays empty until you press Scan in Settings.'] },
    { id: 'accounts', name: 'Accounts', blurb: 'One login per person', skip: true,
      loses: ['Everyone has to share the owner login. That login also controls every server setting.', 'Add accounts later in Settings, under Accounts.'] },
    { id: 'tmdb', name: 'Movie data', blurb: 'Free TMDB key for posters and details', skip: true, key: 'tmdb' },
    { id: 'opensubtitles', name: 'Subtitles', blurb: 'Free OpenSubtitles key for online search', skip: true, key: 'opensubtitles' },
    { id: 'remote', name: 'Remote access', blurb: 'Watch away from home over HTTPS', skip: true,
      loses: ['Platno is reachable only from the network it runs on, not from the internet.', 'You can turn it on later in Settings, under Remote access.'] },
    { id: 'finish', name: 'Finish', blurb: '' }
  ];
  const S = {
    open: false, i: 0, max: 0, dir: 1, leaving: false, warn: false, dirty: false, state: {}, sys: null, sysPending: false,
    server: {}, libs: [], providers: {}, accounts: [], remote: null, scan: null, checks: {}, autoSort: true, busyKey: false
  };
  let root = null, restore = [], lastHelp = {};

  const field = o => String(o ?? '');
  const step = () => STEPS[S.i];
  const owner = () => typeof authUser !== 'undefined' && authUser && authUser.role === 'owner';

  /* ---------- shell ---------- */
  function trap(e) {
    if (!root) return;
    if (e.key === 'Escape' && root.querySelector('select:open')) return;
    if (e.key === 'Escape') { e.preventDefault(); if (root.classList.contains('sg-modal')) closeHelp('close'); else toggleExit(true); return; }
    if (e.key === 'Enter' && e.target.dataset && e.target.dataset.enter) { e.preventDefault(); ACT[e.target.dataset.enter]?.(e.target); return; }
    if (e.key !== 'Tab') return;
    const items = [...root.querySelectorAll('button,[href],input,select,textarea,[tabindex]:not([tabindex="-1"])')].filter(el => el.tabIndex >= 0 && !el.disabled && el.offsetParent !== null && !el.closest('[inert]'));
    if (!items.length) return;
    const first = items[0], last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  }
  function mount(html, cls) {
    // A modal <dialog> lives in the browser's top layer, so it also appears above the app's own dialogs
    // (such as Edit identification) and makes everything underneath inert.
    root = document.createElement('dialog');
    root.className = cls; root.innerHTML = html;
    document.body.append(root); root.showModal();
    root.addEventListener('cancel', e => e.preventDefault());
    root.addEventListener('click', e => { const b = e.target.closest('[data-act]'); if (b && !b.disabled) ACT[b.dataset.act]?.(b, e); });
    root.addEventListener('change', e => { if (e.target.dataset.libKind !== undefined) { const n=Number(e.target.dataset.libKind); S.libs[n].kind=e.target.value; e.target.closest('.sg-card').querySelector('p.muted').textContent=KIND_HELP[e.target.value] || ''; } });
    document.addEventListener('keydown', trap);
    requestAnimationFrame(() => root.classList.add('on'));
  }
  async function unmount() {
    if (!root) return;
    root.classList.remove('on');
    await sleep(reduced() ? 0 : 220);
    document.removeEventListener('keydown', trap); root.close(); root.remove(); root = null;
  }
  function shell() {
    return `<div class="sg-rail" role="navigation" aria-label="Setup progress"><div class="sg-brand"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M6 3.5l10 6.5-10 6.5z"/></svg><b>Platno</b></div><ol class="sg-steps" id="sg-steps"></ol></div>
<div class="sg-main"><div class="sg-top"><span class="sg-count" id="sg-count" aria-live="polite"></span><div class="sg-meter" aria-hidden="true"><i id="sg-meter"></i></div><button class="secondary sg-close" data-act="exit">Close guide</button></div>
<div class="sg-scroll"><div class="sg-body" id="sg-body" tabindex="-1"></div></div>
<div class="sg-foot" id="sg-foot"></div></div>
<div class="sg-sheet" id="sg-sheet" hidden><div class="sg-sheet-card" role="alertdialog" aria-labelledby="sg-sheet-title"><h2 id="sg-sheet-title">Leave the guide?</h2><p>Your changes so far are saved. You can run the guide again any time from Settings.</p><div class="sg-actions"><button data-act="exit-keep">Keep going</button><button class="secondary" data-act="exit-later">Remind me next time</button><button class="secondary" data-act="exit-never">Do not show again</button></div></div></div>`;
  }
  function renderRail() {
    if (!$('#sg-steps')) return;
    $('#sg-steps').innerHTML = STEPS.map((s, n) => {
      const st = n === S.i ? 'current' : S.state[s.id] || (n < S.max ? 'seen' : 'todo');
      const mark = st === 'done' ? CHECK : st === 'skipped' ? DASH : `<span>${n + 1}</span>`;
      return `<li data-state="${st}"><button type="button" class="sg-step" ${n > S.max || S.leaving ? 'disabled' : ''} data-act="jump" data-n="${n}" ${n === S.i ? 'aria-current="step"' : ''}><span class="sg-mark">${mark}</span><span class="sg-step-name">${esc(s.name)}</span></button></li>`;
    }).join('');
    $('#sg-count').textContent = `Step ${S.i + 1} of ${STEPS.length}`;
    $('#sg-meter').style.transform = `scaleX(${(S.i + 1) / STEPS.length})`;
  }
  function renderFoot(primary) {
    const s = step();
    $('#sg-foot').innerHTML = `${S.i > 0 ? '<button class="secondary" data-act="back">Back</button>' : '<span></span>'}<div class="sg-foot-right">${s.skip ? `<button class="secondary sg-skip" data-act="skip" aria-expanded="${S.warn}" aria-controls="sg-warn">Skip this step</button>` : ''}${primary ? `<button data-act="${primary.act}" id="sg-primary" ${primary.disabled ? 'disabled' : ''}>${esc(primary.label)}</button>` : ''}</div>`;
  }
  function warnPanel() {
    const s = step();
    if (!s.skip) return '';
    const lines = s.key ? [...KEYS[s.key].loses, KEYS[s.key].still, `Add it later: ${KEYS[s.key].where}. Platno will also explain this whenever you try a feature that needs it.`] : s.loses;
    return `<div class="sg-warn" id="sg-warn" ${S.warn ? '' : 'inert'}><div><div class="sg-warn-in">${WARN}<div><h3>Skip ${esc(s.name.toLowerCase())}?</h3><ul>${lines.map(l => `<li>${esc(l)}</li>`).join('')}</ul><div class="sg-actions"><button class="secondary" data-act="skip-confirm">Skip anyway</button><button data-act="skip-cancel">Stay on this step</button></div></div></div></div></div>`;
  }
  async function paint(content) {
    const body = $('#sg-body');
    body.className = 'sg-body';
    body.innerHTML = `<h1 id="sg-title" tabindex="-1" data-stagger style="--i:0">${esc(content.title)}</h1>${content.lead ? `<p class="sg-lead" data-stagger style="--i:1">${content.lead}</p>` : ''}<div class="sg-content" data-stagger style="--i:2">${content.html}</div>${warnPanel()}<p class="sg-msg" id="sg-foot-msg" role="alert"></p>`;
    root.setAttribute('aria-labelledby', 'sg-title');
    body.dataset.dir = S.dir; body.classList.add('enter');
    renderRail(); renderFoot(content.primary);
    $('#sg-body').parentElement.scrollTop = 0;
    $('#sg-title').focus({ preventScroll: true });
  }
  async function go(n, state) {
    if (S.leaving || n < 0 || n >= STEPS.length) return;
    S.leaving = true;
    if (state) S.state[step().id] = state;
    S.dir = n >= S.i ? 1 : -1;
    $('#sg-body').dataset.dir = S.dir; $('#sg-body').classList.add('leave'); renderRail();
    await sleep(reduced() ? 0 : 140);
    S.i = n; S.max = Math.max(S.max, n); S.warn = false; S.leaving = false;
    await show();
  }
  async function show() {
    const s = step();
    try { await LOAD[s.id]?.(); } catch (e) { console.error('Setup guide could not load', s.id, e); }
    await paint(RENDER[s.id]());
    AFTER[s.id]?.();
  }
  const msg = (id, text, bad) => { const el = $('#' + id); if (!el) return; el.textContent = text; el.dataset.bad = bad ? '1' : ''; };

  /* ---------- data ---------- */
  async function loadSystem() {
    if (S.sys || S.sysPending) return;
    S.sysPending = true;
    try { S.sys = await api('/api/owner/system'); } catch (e) { S.sys = { error: e.message, encoders: ['libx264'], max_slots: 8, conversion_slots: 4, recommended_slots: 2 }; }
    S.sysPending = false;
    if (S.open && step().id === 'performance' && !S.leaving) { await paint(RENDER.performance()); AFTER.performance?.(); }
    if (S.open && step().id === 'places' && !S.leaving) { $('#sg-places-sys') && ($('#sg-places-sys').outerHTML = sysCard()); }
  }
  async function refreshServer() { S.server = await api('/api/owner/settings'); }
  async function refreshProviders() { S.providers = await api('/api/owner/providers'); }
  async function refreshLibs() {
    const data = await api('/api/libraries');
    S.libs = (data.libraries || []).map(l => ({ id: l.id, name: l.name, kind: l.kind, roots: [...(l.roots || [])] }));
    if (!S.libs.length) S.libs = [{ name: 'My media', kind: 'mixed', roots: [] }];
    S.autoSort = data.auto_sort !== false;
    S.libs.flatMap(l => l.roots).forEach(checkRoot);
  }
  async function checkRoot(path) {
    if (S.checks[path] && S.checks[path] !== 'checking') return;
    S.checks[path] = 'checking';
    try { S.checks[path] = (await post('/api/admin/check-folder', { path })).reason; } catch { S.checks[path] = 'invalid'; }
    S.libs.forEach((_, n) => refreshRoots(n));
  }

  /* ---------- step views ---------- */
  const pill = k => `<span class="sg-pill" data-k="${k}">${CHECK_TEXT[k] || k}</span>`;
  const bullets = l => `<ul class="sg-list">${l.map(x => `<li>${esc(x)}</li>`).join('')}</ul>`;
  const RENDER = {
    welcome: () => ({
      title: 'Let’s set up your media server',
      lead: 'About five minutes. Every step can be skipped, and every choice can be changed later in Settings. Where something needs an outside service, we tell you plainly what you would miss.',
      html: `<div class="sg-welcome"><ol class="sg-overview">${STEPS.slice(1, -1).map((s, n) => `<li data-stagger style="--i:${n + 3}"><b>${esc(s.name)}</b><span>${esc(s.blurb)}</span></li>`).join('')}</ol>
<svg class="sg-hero" viewBox="0 0 120 120" aria-hidden="true"><path class="sg-hero-line" d="M40 24l52 36-52 36z"/></svg></div>`,
      primary: { label: 'Start', act: 'next' }
    }),
    libraries: () => ({
      title: 'Where is your media?',
      lead: 'Start with one shared folder and Automatic to recognize movies, series and other videos. Or add your own libraries and choose a folder for each. Files stay in place.',
      html: `${S.libs.map(libCard).join('')}<button class="secondary" data-act="add-library">+ Add another library</button><p class="muted">Only libraries you save are created. You can add, rename or delete them later in Settings.</p>
<p class="muted" data-stagger style="--i:${S.libs.length + 4}">${hintPaths()}</p>`,
      primary: { label: 'Save folders and continue', act: 'save-libs' }
    }),
    places: () => ({
      title: 'Other locations',
      lead: 'Where Platno keeps its own data, and where video conversions write temporary files.',
      html: `<section class="sg-card" data-stagger style="--i:3"><h3>Data folder</h3><p class="muted">Accounts, library, posters and settings live here. It must stay outside your media folders.</p><code class="sg-code">${esc(S.sys?.data_dir || 'Detecting…')}</code><p class="muted">To move it, set <code>SMART_MEDIA_DATA</code> to a new folder and restart. Copy the old folder first to keep your accounts.</p></section>
<section class="sg-card" data-stagger style="--i:4"><h3>Conversion folder</h3><p class="muted">Temporary files while a video is converted for your browser, and while speech sync runs. Use a fast disk with free space. They are removed when playback ends.</p>${conversionForm()}</section>
<div id="sg-places-sys">${sysCard()}</div>`,
      primary: { label: 'Continue', act: 'next-done' }
    }),
    performance: () => {
      const s = S.sys;
      if (!s) return { title: 'Memory and graphics', lead: 'Detecting your hardware…', html: '<div class="sg-skeleton" aria-busy="true"><i></i><i></i><i></i></div>', primary: { label: 'Save and continue', act: 'save-perf', disabled: true } };
      const hw = (s.usable_encoders || s.encoders).filter(e => e !== 'libx264'), cur = S.server.hardware_encoder || 'auto';
      const pick = cur !== 'auto' ? cur : (hw[0] || 'auto');
      const q = S.server.transcode_quality || 'balanced', slots = Number(S.server.effective_conversion_slots || s.conversion_slots);
      return {
        title: 'Memory and graphics',
        lead: 'Videos your browser cannot play directly are converted on the fly. These choices decide how hard your computer works doing that.',
        html: `${hardwareFacts(s)}
<section class="sg-card" data-stagger style="--i:5"><h3>Encoder</h3><p class="muted">${hw.length ? `Your graphics hardware can do this work, which leaves the processor free. Platno tests the encoder on every stream and falls back to software if it fails.` : 'No hardware encoder was found, so conversions use the processor. That works everywhere.'}</p>
<label for="sg-enc">Encoder</label><select id="sg-enc"><option value="auto" ${pick === 'auto' ? 'selected' : ''}>${ENCODER.libx264}</option>${hw.map(e => `<option value="${esc(e)}" ${pick === e ? 'selected' : ''}>${esc(ENCODER[e] || e)}${e === hw[0] ? ' (recommended)' : ''}</option>`).join('')}</select></section>
<section class="sg-card" data-stagger style="--i:6"><h3>Picture quality</h3><div class="sg-radios" role="radiogroup" aria-label="Picture quality">${[['speed', 'Faster start', 'Lower bitrate. Starts quicker and suits slow networks.'], ['balanced', 'Balanced', 'The best choice for most homes.'], ['quality', 'Best picture', 'Higher bitrate. More processor and network use.']].map(([v, n, d]) => `<label class="sg-radio"><input type="radio" name="sg-q" value="${v}" ${q === v ? 'checked' : ''}><span><b>${n}</b>${d}</span></label>`).join('')}</div></section>
<section class="sg-card" data-stagger style="--i:7"><h3>Simultaneous conversions</h3><p class="muted">Each conversion is its own FFmpeg process, and adaptive quality runs three at once. More at the same time means more memory and processor use. ${s.memory_gb ? `With ${s.memory_gb} GB of memory and ${s.cpu_threads} processor threads we suggest ${s.recommended_slots}.` : `With ${s.cpu_threads} processor threads we suggest ${s.recommended_slots}.`}</p>
<div class="sg-slider"><input type="range" id="sg-slots" min="1" max="${s.max_slots}" value="${slots}" aria-label="Simultaneous conversions" aria-describedby="sg-slots-out"><output id="sg-slots-out" for="sg-slots">${slots}</output></div><p class="sg-msg" id="sg-slots-note" role="status"></p></section>`,
        primary: { label: 'Save and continue', act: 'save-perf' }
      };
    },
    scan: () => scanView(),
    accounts: () => ({
      title: 'Who will use this?',
      lead: 'Give each person their own login. Everyone also gets profiles with their own watch history inside their account.',
      html: `<ul class="sg-accounts" id="sg-accounts">${S.accounts.map(accountRow).join('')}</ul>
<form class="sg-card sg-form" id="sg-account-form" data-stagger style="--i:4" autocomplete="off" onsubmit="return false"><h3>Add a person</h3>
<label for="sg-a-email">Email (used to sign in)</label><input id="sg-a-email" type="email" autocomplete="off" maxlength="254">
<label for="sg-a-name">Display name (optional)</label><input id="sg-a-name" maxlength="64" autocomplete="off">
<div class="sg-two"><div><label for="sg-a-pass">Password (12 characters or more)</label><input id="sg-a-pass" type="password" autocomplete="new-password" minlength="12"></div><div><label for="sg-a-pass2">Confirm password</label><input id="sg-a-pass2" type="password" autocomplete="new-password" minlength="12" data-enter="add-account"></div></div>
<label for="sg-a-role">Can this person manage the server?</label><select id="sg-a-role"><option value="user">No: browse and watch only</option><option value="admin">Yes: also manage libraries and scans</option></select>
<div class="sg-actions"><button type="button" class="secondary" data-act="add-account">Add person</button></div><p class="sg-msg" id="sg-a-msg" role="status"></p></form>`,
      primary: { label: 'Continue', act: 'next-done' }
    }),
    tmdb: () => keyView('tmdb'),
    opensubtitles: () => keyView('opensubtitles'),
    remote: () => remoteView(),
    finish: () => finishView()
  };
  const AFTER = {
    welcome: () => { $('.sg-hero')?.classList.add('draw'); },
    performance: () => { const r = $('#sg-slots'); if (r) r.oninput = () => { $('#sg-slots-out').textContent = r.value; }; },
    scan: () => {}
  };
  const LOAD = {
    libraries: async () => { await refreshLibs(); },
    places: async () => { await refreshServer(); loadSystem(); },
    performance: async () => { await refreshServer(); loadSystem(); if (!S.sys) await Promise.race([sleep(1500), (async () => { while (!S.sys) await sleep(50); })()]); },
    scan: async () => { await refreshLibs(); await refreshProviders().catch(() => {}); },
    accounts: async () => { S.accounts = (await api('/api/admin/accounts')).accounts || []; },
    tmdb: async () => { await refreshProviders(); },
    opensubtitles: async () => { await refreshProviders(); },
    remote: async () => { S.remote = await api('/api/owner/remote-access'); loadSystem(); if (!S.sys) await Promise.race([sleep(1200), (async () => { while (!S.sys) await sleep(50); })()]); },
    finish: async () => { await Promise.all([refreshProviders(), refreshLibs(), refreshServer(), api('/api/admin/accounts').then(a => { S.accounts = a.accounts || []; }), api('/api/owner/remote-access').then(r => { S.remote = r; })]); }
  };

  function examplePath(name) {
    const os = S.sys?.os || (navigator.platform || '').toLowerCase();
    if (S.sys?.docker) return '/media/' + name;
    if (/mac|darwin/.test(os)) return '/Volumes/Media/' + name;
    if (/win/.test(os)) return 'D:\\Media\\' + name;
    return '/mnt/media/' + name;
  }
  function hintPaths() {
    const os = S.sys?.os || (navigator.platform || '').toLowerCase();
    if (/mac|darwin/.test(os)) return 'On a Mac, an external drive looks like /Volumes/DriveName/Movies. Use Choose to pick it from a list.';
    if (/win/.test(os)) return 'For example D:\\Media\\Movies. Use Choose to pick it from a list.';
    return S.sys?.docker ? 'In Docker, your media folders appear under /media. Use Choose to pick one from a list.' : 'Enter the full path, for example /mnt/media/movies, or use Choose to pick it from a list.';
  }
  function rememberLibraries() {
    S.libs.forEach((l,n) => { l.name = $('#sg-name-'+n)?.value.trim() ?? l.name; l.kind = $('#sg-kind-'+n)?.value ?? l.kind; });
  }
  function libCard(l, n) {
    return `<section class="sg-card" data-stagger style="--i:${n + 3}"><div class="sg-library-fields"><label for="sg-name-${n}">Library name<input id="sg-name-${n}" value="${esc(l.name)}" maxlength="48" data-lib-name="${n}"></label><label for="sg-kind-${n}">Content<select id="sg-kind-${n}" data-lib-kind="${n}">${Object.entries(KIND).map(([key,name])=>`<option value="${key}" ${l.kind===key?'selected':''}>${esc(name)}</option>`).join('')}</select></label></div><p class="muted">${esc(KIND_HELP[l.kind] || '')}</p><ul class="sg-roots" id="sg-roots-${n}">${rootRows(n)}</ul>
<div class="sg-add"><input id="sg-add-${n}" placeholder="${esc(examplePath(l.name))}" aria-label="Folder path for ${esc(l.name)}" autocomplete="off" spellcheck="false" data-enter="add-root" data-n="${n}"><button class="secondary" data-act="add-root" data-n="${n}">Add</button><button class="secondary" data-act="choose-root" data-n="${n}">Choose</button></div><p class="sg-msg" id="sg-msg-${n}" role="status"></p></section>`;
  }
  function rootRows(n) {
    const l = S.libs[n];
    return l.roots.map((r, k) => `<li class="sg-root" data-state="${S.checks[r] || 'checking'}"><span class="path">${esc(r)}</span>${pill(S.checks[r] || 'checking')}<button class="secondary sg-x" data-act="remove-root" data-n="${n}" data-k="${k}" aria-label="Remove ${esc(r)}">Remove</button></li>`).join('') || '<li class="sg-empty">No folder yet</li>';
  }
  function refreshRoots(n) { const el = $('#sg-roots-' + n); if (el && S.libs[n]) el.innerHTML = rootRows(n); }
  function conversionForm() {
    const locked = S.server.transcode_dir_from_environment;
    return `<label for="sg-work">Folder (absolute path)</label><div class="sg-add"><input id="sg-work" value="${esc(S.server.transcode_dir || '')}" placeholder="${esc(S.server.effective_transcode_dir || '')}" ${locked ? 'disabled' : ''} autocomplete="off" spellcheck="false" data-enter="save-work"><button class="secondary" data-act="save-work" ${locked ? 'disabled' : ''}>Use this folder</button><button class="secondary" data-act="reset-work" ${locked ? 'disabled' : ''}>Reset</button></div>
<p class="muted">Currently using: <code>${esc(S.server.effective_transcode_dir || '')}</code>${locked ? ' (fixed by the SMART_MEDIA_TRANSCODE_DIR environment variable)' : ''}</p><p class="sg-msg" id="sg-work-msg" role="status"></p>`;
  }
  function sysCard() {
    const s = S.sys;
    if (!s) return '<section class="sg-card sg-pulse" id="sg-places-sys"><h3>FFmpeg</h3><p class="muted">Checking…</p></section>';
    return `<section class="sg-card" id="sg-places-sys" data-stagger style="--i:5"><h3>FFmpeg</h3>${s.ffmpeg ? `<p class="sg-ok">${CHECK}<span>FFmpeg works. Videos that browsers cannot play will be converted.</span></p>` : `<p class="sg-bad">${WARN}<span>FFmpeg was not found. Without it, only files your browser can play directly will work (many MP4 files). Install it, then restart Platno: on a Mac run <code>brew install ffmpeg</code>, on Debian or Ubuntu <code>sudo apt install ffmpeg</code>, or set <code>SMART_MEDIA_FFMPEG</code> and <code>SMART_MEDIA_FFPROBE</code> to their paths.</span></p>`}</section>`;
  }
  function hardwareFacts(s) {
    const gpu = s.gpu || (s.hardware_encoder_hint ? s.hardware_encoder_hint : 'Not detected');
    const fact = (k, v, n) => `<div class="sg-fact" data-stagger style="--i:${n}"><span>${k}</span><b>${esc(v)}</b></div>`;
    return `<div class="sg-facts">${fact('Memory', s.memory_gb ? `${s.memory_gb} GB` : 'Not detected', 3)}${fact('Processor', `${s.cpu_threads} threads`, 4)}${fact('Graphics', gpu, 5)}</div>`;
  }
  const accountRow = a => `<li class="sg-account"><span><b>${esc(a.username)}</b><small>${esc(a.email || '')}</small></span><span class="badge">${esc(a.role)}</span></li>`;

  function scanView() {
    const withRoots = S.libs.filter(l => l.roots.length), sc = S.scan;
    let body;
    if (!withRoots.length) body = `<div class="sg-card sg-center"><p>No folders were added, so there is nothing to scan yet.</p><button class="secondary" data-act="goto" data-id="libraries">Add folders</button></div>`;
    else body = `<section class="sg-card"><ul class="sg-plan">${withRoots.map(l => `<li><b>${esc(l.name)}</b><span>${l.roots.length} folder${l.roots.length === 1 ? '' : 's'}</span></li>`).join('')}</ul>
<div class="sg-bar" role="progressbar" aria-label="Scan progress" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${sc ? sc.pct : 0}"><i id="sg-bar" style="transform:scaleX(${sc ? sc.pct / 100 : 0})"></i></div>
<p id="sg-scan-line" role="status">${esc(scanLine())}</p><p class="path sg-file" id="sg-scan-file">${esc(sc?.file || '')}</p>
<div class="sg-actions"><button data-act="scan-start" id="sg-scan-btn" ${sc?.running ? 'hidden' : ''}>${sc?.done ? 'Scan again' : 'Scan now'}</button><button class="secondary" data-act="scan-cancel" id="sg-scan-stop" ${sc?.running ? '' : 'hidden'}>Cancel scan</button></div></section>
<div id="sg-scan-result">${scanResult()}</div>
${providerNote()}`;
    return { title: 'Read your library', lead: 'The scanner walks through your folders, reads file names and matches them to titles. Large libraries on network drives can take a while; you can keep going and watch progress later.', html: body, primary: { label: sc?.running ? 'Scanning…' : 'Continue', act: 'next-done', disabled: !!sc?.running } };
  }
  function providerNote() {
    if (S.providers.tmdb_configured) return '';
    return `<p class="sg-note" data-stagger style="--i:6">${WARN}<span>No TMDB key yet, so movies are named from the free Wikidata database or from their file names, without posters or ratings. You can edit them by hand, or add the key in a later step and the next scan upgrades them.</span></p>`;
  }
  const scanLine = () => { const sc = S.scan; if (!sc) return 'Ready when you are.'; if (sc.error) return sc.error; if (sc.running) return `Reading ${sc.lib}: ${sc.processed} of ${sc.total || '…'} videos`; if (sc.cancelled) return `Cancelled after ${sc.processed} videos.`; return `Done: ${sc.processed} videos processed.`; };
  function scanResult() {
    const sc = S.scan;
    if (!sc || sc.running || !sc.done || sc.cancelled) return '';
    const r = sc.summary || {};
    return `<div class="sg-card sg-result"><p class="sg-ok">${CHECK}<span><b>${sc.processed} videos</b>: ${r.movies || 0} movies, ${r.episodes || 0} episodes, ${r.other || 0} other${r.review ? `. ${r.review} need a look under the review filter.` : '.'}</span></p>${sc.errors ? `<p class="muted">${sc.errors} problem${sc.errors === 1 ? '' : 's'} while reading, usually a missing folder or no permission:</p><ul class="sg-list">${(sc.errorList || []).map(e => `<li>${esc(e)}</li>`).join('')}</ul>` : ''}</div>`;
  }

  function keyView(kind) {
    const k = KEYS[kind], configured = S.providers[k.provider], fromEnv = S.providers[k.override];
    const status = configured ? `<p class="sg-ok" data-stagger style="--i:3">${CHECK}<span>${k.name} is configured${fromEnv ? ` by the ${k.env} environment variable` : ''}. You can replace it below.</span></p>` : '';
    return {
      title: k.title,
      lead: `${k.name} offers a free key. It is optional, but these features need it.`,
      html: `${status}<div class="sg-split"><section class="sg-card" data-stagger style="--i:4"><h3>With a key</h3>${bullets(k.gives)}</section><section class="sg-card sg-lack" data-stagger style="--i:5"><h3>Without a key</h3>${bullets(k.loses)}<p class="muted">${esc(k.still)}</p></section></div>
${howTo(kind)}`,
      primary: { label: configured ? 'Continue' : 'Continue without a key', act: 'next-done' }
    };
  }
  function howTo(kind, compact) {
    const k = KEYS[kind];
    const steps = `<ol class="sg-steps-how">${k.steps.map(s => `<li><span>${esc(s.t)}</span>${s.href ? `<a href="${esc(s.href)}" target="_blank" rel="noopener noreferrer">${esc(s.link)}</a>` : ''}</li>`).join('')}</ol>`;
    const form = owner() ? `<div class="sg-add"><input id="sg-key-${kind}" type="password" autocomplete="new-password" spellcheck="false" placeholder="Paste your ${esc(k.name)} ${kind === 'tmdb' ? 'Read Access Token' : 'API key'}" aria-label="${esc(k.name)} key" data-enter="save-key" data-kind="${kind}"><button data-act="save-key" data-kind="${kind}" id="sg-key-btn-${kind}">Save and test</button></div><p class="sg-msg" id="sg-key-msg-${kind}" role="status"></p>`
      : '<p class="sg-note">' + WARN + '<span>Only the server owner can add this. Show these steps to them.</span></p>';
    return `<section class="sg-card ${compact ? '' : ''}" ${compact ? '' : 'data-stagger style="--i:6"'}><h3>How to get it</h3>${steps}${form}<p class="muted">Prefer the command line? Set <code>${k.env}</code> before starting Platno; it overrides the saved value. Where to find it later: ${esc(k.where)}.</p></section>`;
  }

  function remoteView() {
    const r = S.remote || {}, s = S.sys || {};
    const lan = s.listen_host && !['127.0.0.1', 'localhost', '::1'].includes(s.listen_host);
    const net = `<section class="sg-card" data-stagger style="--i:3"><h3>Home network</h3><p class="${lan ? 'sg-ok' : 'sg-bad'}">${lan ? CHECK : WARN}<span>${lan ? `Platno listens on ${esc(s.listen_host)}:${s.port}, so phones and TVs on your network can open it.` : `Platno currently listens only on this computer (127.0.0.1:${s.port || 8787}). To use it from other devices at home, set <code>SMART_MEDIA_HOST</code> to your computer’s network address (or 0.0.0.0) and restart.`}</span></p></section>`;
    const setup = r.configured ? `<section class="sg-card" data-stagger style="--i:4"><h3>Internet access over HTTPS</h3><p class="muted">Only the owner can turn this on, every visitor must sign in, and plain HTTP is never exposed. A trusted certificate and a DNS name pointing to your home are required.</p>
<label for="sg-remote-mode">Router setup</label><select id="sg-remote-mode"><option value="automatic" ${r.automatic !== false ? 'selected' : ''}>Automatic (UPnP opens one HTTPS port for ten minutes at a time)</option><option value="manual" ${r.automatic === false ? 'selected' : ''}>Manual (I forward a TCP port myself)</option></select>
<div id="sg-remote-state">${remoteState(r)}</div><div class="sg-actions"><button data-act="remote-on" id="sg-remote-on">${r.enabled ? 'Apply and reconnect' : 'Turn on remote access'}</button><button class="secondary" data-act="remote-off" ${r.enabled ? '' : 'disabled'} id="sg-remote-off">Turn off</button></div><p class="sg-msg" id="sg-remote-msg" role="status"></p></section>`
      : `<section class="sg-card" data-stagger style="--i:4"><h3>Not configured on this server yet</h3><p class="muted">${esc(r.configuration_error || 'Remote access needs a hostname, certificate and your computer’s address.')} All five settings are needed. Set them before starting Platno, restart, then turn it on here or in Settings, under Remote access:</p>
<pre class="sg-pre">SMART_MEDIA_REMOTE_HOSTNAME=media.example.com
SMART_MEDIA_REMOTE_IP=192.168.1.50
SMART_MEDIA_REMOTE_PORT=8943
SMART_MEDIA_TLS_CERT=/private/fullchain.pem
SMART_MEDIA_TLS_KEY=/private/privkey.pem</pre><p class="muted">The hostname must be covered by your certificate, and the IP is this computer’s address on your home network. Full walkthrough: <a href="/help#remote-access" target="_blank" rel="noopener noreferrer">Setup guide</a>.</p></section>`;
    return { title: 'Watch away from home', lead: 'Optional. Without it Platno works only on your own network.', html: net + setup, primary: { label: 'Continue', act: 'next-done' } };
  }
  const remoteState = r => `<p class="${r.enabled ? 'sg-ok' : 'muted'}">${r.enabled ? CHECK : ''}<span>${r.enabled ? `On${r.url ? `: <code>${esc(r.url)}</code>` : ''}. ${esc(r.message || '')}` : 'Currently off.'}</span></p>`;

  function finishView() {
    const roots = S.libs.reduce((n, l) => n + l.roots.length, 0), sc = S.scan, p = S.providers;
    const rows = [
      { id: 'libraries', name: 'Media folders', ok: roots > 0, good: `${roots} folder${roots === 1 ? '' : 's'} added`, bad: 'No folders: the library is empty' },
      { id: 'scan', name: 'First scan', ok: !!(sc && sc.done && !sc.cancelled), good: `${sc?.processed || 0} videos read`, bad: 'Not run yet: press Scan in Settings' },
      { id: 'performance', name: 'Conversion', ok: true, good: `${S.server.hardware_encoder && S.server.hardware_encoder !== 'auto' ? S.server.hardware_encoder : 'software encoder'}, ${S.server.effective_conversion_slots || 4} at a time` },
      { id: 'accounts', name: 'Accounts', ok: S.accounts.length > 1, good: `${S.accounts.length} accounts`, bad: 'Only the owner login: everyone shares it' },
      { id: 'tmdb', name: 'Movie data (TMDB)', ok: !!p.tmdb_configured, good: 'Key saved', bad: 'No key: movies have no posters or ratings' },
      { id: 'opensubtitles', name: 'Subtitle search', ok: !!p.opensubtitles_configured, good: 'Key saved', bad: 'No key: online subtitle search is off' },
      { id: 'remote', name: 'Remote access', ok: !!S.remote?.enabled, good: 'On', bad: 'Off: reachable only on your own network' }
    ];
    const open = rows.filter(r => !r.ok).length;
    const later = sc && sc.done && p.tmdb_configured && sc.tmdbAfter ? `<p class="sg-note">${WARN}<span>You added the TMDB key after scanning. Run one more scan so your movies are identified.</span><button class="secondary" data-act="goto" data-id="scan">Back to scan</button></p>` : '';
    return {
      title: open ? 'Almost there' : 'Everything is ready',
      lead: open ? `${open} thing${open === 1 ? ' is' : 's are'} still open. None of it blocks you, and each can be done later from Settings.` : 'Your server is set up. Enjoy.',
      html: `<ul class="sg-summary">${rows.map((r, n) => `<li data-state="${r.ok ? 'ok' : 'open'}" data-stagger style="--i:${n + 3}"><span class="sg-mark">${r.ok ? CHECK : DASH}</span><div><b>${esc(r.name)}</b><span>${esc(r.ok ? r.good : r.bad)}</span></div>${r.ok ? '' : `<button class="secondary" data-act="goto" data-id="${r.id}">Set up</button>`}</li>`).join('')}</ul>${later}
<p class="muted">Run this guide again any time from Settings. If an action later needs a missing key, Platno explains it again at that moment.</p>`,
      primary: { label: 'Open my library', act: 'finish' }
    };
  }

  /* ---------- actions ---------- */
  // A step counts as done only when it actually changed something useful, so the rail never overstates.
  function outcome() {
    const id = step().id;
    if (id === 'accounts') return S.accounts.length > 1 ? 'done' : 'skipped';
    if (id === 'tmdb' || id === 'opensubtitles') return S.providers[KEYS[id].provider] ? 'done' : 'skipped';
    if (id === 'remote') return S.remote?.enabled ? 'done' : 'skipped';
    if (id === 'scan') return S.scan?.done && !S.scan.cancelled ? 'done' : 'skipped';
    return 'done';
  }
  const ACT = {
    next: () => go(S.i + 1),
    'next-done': () => go(S.i + 1, outcome()),
    back: () => go(S.i - 1),
    jump: b => go(Number(b.dataset.n)),
    goto: b => go(STEPS.findIndex(s => s.id === b.dataset.id)),
    skip: () => { S.warn = !S.warn; const w = $('#sg-warn'); if (w) { w.toggleAttribute('inert', !S.warn); } $('.sg-skip')?.setAttribute('aria-expanded', S.warn); if (S.warn) setTimeout(() => $('#sg-warn')?.scrollIntoView({ block: 'nearest', behavior: reduced() ? 'auto' : 'smooth' }), 60); },
    'skip-cancel': () => ACT.skip(),
    'skip-confirm': () => go(S.i + 1, 'skipped'),
    exit: () => toggleExit(true),
    'exit-keep': () => toggleExit(false),
    'exit-later': async () => { await unmount(); S.open = false; if (S.dirty) location.reload(); },
    'exit-never': async () => { await post('/api/owner/settings', { key: 'setup_guide', value: 'skipped' }).catch(() => {}); await unmount(); S.open = false; if (S.dirty) location.reload(); },
    finish: async () => {
      try { await post('/api/owner/settings', { key: 'setup_guide', value: 'done' }); } catch (e) { msg('sg-foot-msg', e.message, true); return; }
      await unmount(); S.open = false;
      if (S.dirty) location.reload();
    },
    'add-library': async () => { rememberLibraries(); S.libs.push({ name: 'New library', kind: 'mixed', roots: [] }); await paint(RENDER.libraries()); $('#sg-name-' + (S.libs.length-1))?.focus(); },
    'add-root': async b => {
      const n = Number(b.dataset.n), input = $('#sg-add-' + n), path = field(input.value).trim();
      if (!path) { msg('sg-msg-' + n, 'Type or choose a folder first.', true); return; }
      if (S.libs.some(l => l.roots.includes(path))) { msg('sg-msg-' + n, 'That folder is already listed.', true); return; }
      S.libs[n].roots.push(path); input.value = ''; msg('sg-msg-' + n, ''); S.checks[path] = ''; refreshRoots(n); checkRoot(path);
    },
    'choose-root': async b => {
      const n = Number(b.dataset.n);
      msg('sg-msg-' + n, '');
      const path = await window.pickFolder();
      if (!path) return;
      $('#sg-add-' + n).value = path; ACT['add-root']({ dataset: { n } });
    },
    'remove-root': b => { S.libs[Number(b.dataset.n)].roots.splice(Number(b.dataset.k), 1); refreshRoots(Number(b.dataset.n)); },
    'save-libs': async () => {
      const btn = $('#sg-primary'); btn.disabled = true;
      try {
        rememberLibraries();
        for (let n=0; n<S.libs.length; n++) if ($('#sg-add-'+n)?.value.trim()) await ACT['add-root']({dataset:{n}});
        for (const l of S.libs.filter(l => l.id || l.roots.length)) { const saved = await post('/api/admin/libraries', { id: l.id, name: l.name, kind: l.kind, roots: l.roots }); l.id = saved.id; }
        S.libs = S.libs.filter(l => l.id);
        const auto = S.autoSort;
        await post('/api/admin/library-settings', { auto_sort: auto, scan_roots: [...new Set(S.libs.flatMap(l => l.roots))] });
        S.dirty = true; await go(S.i + 1, S.libs.some(l => l.roots.length) ? 'done' : 'skipped');
      } catch (e) { btn.disabled = false; msg('sg-foot-msg', e.message, true); }
    },
    'save-work': async () => {
      const value = $('#sg-work').value.trim();
      msg('sg-work-msg', 'Checking the folder…');
      try { const r = await post('/api/owner/settings', { key: 'transcode_dir', value }); await refreshServer(); msg('sg-work-msg', `Saved. Using ${r.effective_transcode_dir}.`); S.dirty = true; }
      catch (e) { msg('sg-work-msg', e.message, true); }
    },
    'reset-work': async () => { $('#sg-work').value = ''; await ACT['save-work'](); },
    'save-perf': async () => {
      const btn = $('#sg-primary'); btn.disabled = true;
      try {
        await post('/api/owner/settings', { key: 'hardware_encoder', value: $('#sg-enc').value });
        await post('/api/owner/settings', { key: 'transcode_quality', value: document.querySelector('input[name="sg-q"]:checked').value });
        await post('/api/owner/settings', { key: 'conversion_slots', value: $('#sg-slots').value });
        await go(S.i + 1, 'done');
      } catch (e) { btn.disabled = false; msg('sg-slots-note', e.message, true); }
    },
    'scan-start': () => runScan(),
    'scan-cancel': async () => { if (S.scan) S.scan.cancelled = true; await post('/api/cancel', {}).catch(() => {}); },
    'add-account': async () => {
      const g = id => $('#' + id).value, button = $('[data-act="add-account"]'); button.disabled = true;
      try {
        await post('/api/admin/accounts', { email: g('sg-a-email'), display_name: g('sg-a-name') || null, password: g('sg-a-pass'), confirm_password: g('sg-a-pass2'), role: g('sg-a-role') });
        S.accounts = (await api('/api/admin/accounts')).accounts || [];
        const list = $('#sg-accounts'); list.innerHTML = S.accounts.map(accountRow).join(''); list.lastElementChild?.classList.add('sg-pop');
        ['sg-a-email', 'sg-a-name', 'sg-a-pass', 'sg-a-pass2'].forEach(id => { $('#' + id).value = ''; }); msg('sg-a-msg', 'Added. They can sign in right away.');
      } catch (e) { msg('sg-a-msg', e.message, true); } finally { button.disabled = false; }
    },
    'save-key': async b => {
      const kind = b.dataset.kind, out = await saveKey(kind, $('#sg-key-' + kind).value, id => msg('sg-key-msg-' + kind, id.text, id.bad), $('#sg-key-btn-' + kind));
      if (!out.ok) return;
      await refreshProviders(); S.dirty = true;
      if (root?.classList.contains('sg-modal')) { lastHelp.added = true; const close = $('#sg-help-close'); if (close) close.textContent = lastHelp.scan ? 'Continue with the scan' : 'Done'; return; }
      if (kind === 'tmdb' && S.scan?.done) S.scan.tmdbAfter = true;
      S.state[kind] = 'done'; renderRail();
    },
    'remote-on': async () => {
      msg('sg-remote-msg', 'Preparing HTTPS and the router…'); $('#sg-remote-on').disabled = true;
      try { S.remote = await post('/api/owner/remote-access', { enabled: true, automatic: $('#sg-remote-mode').value === 'automatic' }); $('#sg-remote-state').innerHTML = remoteState(S.remote); msg('sg-remote-msg', 'Remote access is on. Test the address from mobile data.'); $('#sg-remote-off').disabled = false; S.dirty = true; }
      catch (e) { msg('sg-remote-msg', e.message, true); } finally { $('#sg-remote-on').disabled = false; }
    },
    'remote-off': async () => {
      try { S.remote = await post('/api/owner/remote-access', { enabled: false, automatic: false }); $('#sg-remote-state').innerHTML = remoteState(S.remote); msg('sg-remote-msg', 'Remote access is off.'); $('#sg-remote-off').disabled = true; }
      catch (e) { msg('sg-remote-msg', e.message, true); }
    }
  };
  function toggleExit(show) { const sheet = $('#sg-sheet'); if (!sheet) return; sheet.hidden = !show; sheet.classList.toggle('on', show); if (show) sheet.querySelector('button').focus(); }

  async function saveKey(kind, raw, say, button) {
    const k = KEYS[kind], key = field(raw).trim();
    if (!key) { say({ text: 'Paste the key first.', bad: true }); return { ok: false }; }
    if (/\s/.test(key)) { say({ text: 'The key must not contain spaces or line breaks. Copy it again.', bad: true }); return { ok: false }; }
    if (button) button.disabled = true;
    say({ text: 'Saving…' });
    try {
      await post(k.save, { key });
      say({ text: `Testing with ${k.name}…` });
      try {
        const r = await api(k.test);
        const n = (r.results || []).length;
        say({ text: `It works: ${k.name} answered with ${n} result${n === 1 ? '' : 's'}.`, ok: true });
        return { ok: true };
      } catch (e) {
        if (e.code === k.rejected) {
          await post(k.save, { key: '' }).catch(() => {});
          say({ text: `${k.name} rejected this key, so nothing was saved. ${kind === 'tmdb' ? 'Make sure you copied the long “API Read Access Token” (starts with eyJ), not the short API Key.' : 'Copy the API key of your consumer again and check for missing characters.'}`, bad: true });
          return { ok: false };
        }
        say({ text: `Saved, but ${k.name} could not be reached to test it (no internet?). It will be used once the connection works.`, ok: true });
        return { ok: true };
      }
    } catch (e) { say({ text: e.message, bad: true }); return { ok: false }; }
    finally { if (button) button.disabled = false; }
  }

  async function runScan() {
    const libs = S.libs.filter(l => l.roots.length), seen = new Set();
    if (!libs.length) return;
    S.scan = { running: true, processed: 0, total: 0, pct: 0, lib: libs[0].name, file: '', done: false, cancelled: false, errors: 0 };
    const sync = () => {
      const sc = S.scan;
      $('#sg-scan-line') && ($('#sg-scan-line').textContent = scanLine());
      $('#sg-scan-file') && ($('#sg-scan-file').textContent = sc.running ? sc.file : '');
      const bar = $('#sg-bar'); if (bar) bar.style.transform = `scaleX(${sc.pct / 100})`;
      $('.sg-bar')?.setAttribute('aria-valuenow', Math.round(sc.pct));
    };
    $('#sg-scan-btn').hidden = true; $('#sg-scan-stop').hidden = false; $('#sg-primary').disabled = true; $('#sg-primary').textContent = 'Scanning…';
    let finished = 0;
    try {
      for (const [n, lib] of libs.entries()) {
        const roots = lib.roots.filter(r => !seen.has(r)); roots.forEach(r => seen.add(r));
        if (!roots.length || S.scan.cancelled) continue;
        S.scan.lib = lib.name;
        try { await post('/api/scan', { roots, library_id: S.autoSort ? null : lib.id, full_scan: false }); }
        catch (e) { if (e.status !== 409) throw e; }
        for (;;) {
          await sleep(450);
          const st = await api('/api/status');
          S.scan.processed = finished + st.processed; S.scan.total = finished + (st.total || 0); S.scan.file = (st.current_file || '').split('/').pop();
          S.scan.pct = Math.min(99, ((n + (st.total ? st.processed / st.total : 0)) / libs.length) * 100);
          S.scan.errors = Math.max(S.scan.errors, (st.errors || []).length); S.scan.errorList = (st.errors || []).slice(0, 3).concat(S.scan.errorList || []).slice(0, 3); sync();
          if (!st.running) { finished += st.processed; break; }
        }
      }
      S.scan.pct = 100; S.scan.done = true; S.dirty = true;
      const files = await api('/api/files');
      S.scan.summary = files.reduce((a, f) => { const r = f.result || {}; if (r.kind === 'movie') a.movies++; else if (r.kind === 'tv') a.episodes++; else a.other++; if (r.requires_review) a.review++; return a; }, { movies: 0, episodes: 0, other: 0, review: 0 });
    } catch (e) { S.scan.error = e.message; }
    S.scan.running = false; S.state.scan = S.scan.error || S.scan.cancelled ? undefined : 'done';
    if (S.open && step().id === 'scan' && !S.leaving) { await paint(RENDER.scan()); }
  }

  /* ---------- API-key help, shown whenever a feature needs a missing key ---------- */
  function closeHelp(result) { const done = lastHelp.resolve; lastHelp.resolve = null; S.keyHelpOpen = false; unmount().then(() => { if (done) done(result); }); }
  function openHelp(kind, reason, scanPrompt) {
    if (S.open || root) return Promise.resolve('busy');
    const k = KEYS[kind], isOwner = owner();
    const why = reason === 'rejected' ? `${k.name} rejected the key saved on this server. It may have been mistyped, copied from the wrong field, or revoked.` : `No ${k.name} key is saved on this server yet.`;
    const html = `<div class="sg-card-wrap"><section class="sg-help" aria-labelledby="sg-help-title"><h2 id="sg-help-title">${reason === 'rejected' ? `${esc(k.name)} did not accept the key` : `${esc(k.name)} key needed`}</h2><p class="sg-lead">${esc(why)}</p>
<h3>${scanPrompt ? 'What happens if you scan without it' : 'What does not work until it is added'}</h3>${bullets(k.loses)}<p class="muted">${esc(k.still)}</p>
${isOwner ? howTo(kind, true) : `<section class="sg-card"><h3>Ask the server owner</h3><p class="muted">Only the owner can add provider keys. They do it in ${esc(k.where)}.</p>${bullets(k.steps.map(s => s.t))}</section>`}
<div class="sg-actions">${scanPrompt ? '<button class="secondary" data-act="help-scan">Scan anyway</button>' : ''}${isOwner ? '<button class="secondary" data-act="help-settings">Open Settings</button>' : ''}<button data-act="help-close" id="sg-help-close">${scanPrompt ? 'Add the key first' : 'Close'}</button></div></section></div>`;
    return new Promise(resolve => {
      lastHelp = { resolve, scan: !!scanPrompt, added: false };
      mount(html, 'sg sg-modal');
      ACT['help-close'] = () => closeHelp(lastHelp.added && lastHelp.scan ? 'scan' : 'close');
      ACT['help-scan'] = () => closeHelp('scan');
      ACT['help-settings'] = () => { closeHelp('close'); window.view?.('settings'); };
      root.querySelector('#sg-help-title').setAttribute('tabindex', '-1'); root.querySelector('#sg-help-title').focus({ preventScroll: true });
    });
  }
  const shown = {};
  function keyMissing(code) {
    const m = { tmdb_key_missing: ['tmdb', 'missing'], tmdb_key_rejected: ['tmdb', 'rejected'], opensubtitles_key_missing: ['opensubtitles', 'missing'], opensubtitles_key_rejected: ['opensubtitles', 'rejected'] }[code];
    if (!m || S.open || root || Date.now() - (shown[code] || 0) < 8000) return;
    shown[code] = Date.now(); openHelp(m[0], m[1]);
  }
  async function beforeScan() {
    if (S.open || root || sessionStorage.getItem('platno:scan-warned') === '1') return true;
    let status; try { status = await api('/api/admin/provider-status'); } catch { return true; }
    if (status.tmdb) return true;
    sessionStorage.setItem('platno:scan-warned', '1');
    const choice = await openHelp('tmdb', 'missing', true);
    return choice === 'scan' || choice === 'busy';
  }

  /* ---------- entry points ---------- */
  async function open() {
    if (S.open || root) return;
    Object.assign(S, { open: true, i: 0, max: 0, state: {}, warn: false, scan: null, leaving: false, dirty: false, checks: {} });
    mount(shell(), 'sg');
    loadSystem();
    await refreshServer().catch(() => {}); await refreshProviders().catch(() => {});
    await show();
  }
  async function maybeStart() {
    if (!owner() || S.open || root) return;
    try { const server = await api('/api/owner/settings'); if (!server.setup_guide && new URLSearchParams(location.search).get('guide') !== 'off') open(); } catch { /* the guide is optional */ }
  }
  window.PlatnoSetup = { open, maybeStart, keyMissing, beforeScan, help: openHelp, _steps: STEPS.map(s => s.id) };
})();
