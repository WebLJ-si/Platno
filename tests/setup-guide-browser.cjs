// Real-Chrome test of the first-run guide and the missing-API-key help. Needs the Node `playwright` package.
const assert = require('node:assert/strict'), { chromium } = require('playwright'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), net = require('node:net'), { spawn, execFileSync } = require('node:child_process');
const PASSWORD = 'guide password 12345';
(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'plexv2-guide-')), data = path.join(tmp, 'data'), media = path.join(tmp, 'media'), work = path.join(tmp, 'work');
  fs.mkdirSync(data); fs.mkdirSync(path.join(media, 'Movies'), { recursive: true });
  fs.writeFileSync(path.join(media, 'Movies', 'Test.Film.2020.mkv'), 'not a real video');
  fs.writeFileSync(path.join(media, 'Movies', 'VID_20201010_123400.mp4'), 'not a real video');
  fs.mkdirSync(path.join(media,'Movies','Zzq Setup Series','Season 01'),{recursive:true});
  fs.writeFileSync(path.join(media,'Movies','Zzq Setup Series','Season 01','Zzq.Setup.Series.S01E01.mkv'),'not a real video');
  const port = await new Promise(r => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
  const binary = process.env.PLEXV2_TEST_BINARY || path.resolve('target/release/smart-media-scanner');
  const env = { ...process.env, SMART_MEDIA_DATA: data, SMART_MEDIA_HOST: '127.0.0.1', SMART_MEDIA_PORT: String(port), TMDB_TOKEN: '', OPENSUBTITLES_API_KEY: '', SMART_MEDIA_COOKIE_SECURE: '0', SMART_MEDIA_WIKIDATA: 'off' };
  delete env.SMART_MEDIA_TRANSCODE_DIR;
  const server = spawn(binary, [], { env, stdio: 'ignore' }), url = 'http://127.0.0.1:' + port;
  let browser;
  try {
    for (let i = 0; i < 150; i++) { try { await fetch(url + '/api/auth/setup-status'); break; } catch { await new Promise(r => setTimeout(r, 100)); } }
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const errors = [], watch = page => { page.on('pageerror', e => errors.push(e.message)); page.on('console', m => { if (m.type() === 'error' && !/Failed to load resource/.test(m.text())) errors.push(m.text()); }); return page; };
    const context = await browser.newContext({ viewport: { width: 1280, height: 860 } }), page = watch(await context.newPage());
    const title = text => page.locator('#sg-title', { hasText: text }).waitFor();
    const primary = () => page.locator('#sg-primary').click();

    // Owner setup, then the guide opens by itself.
    await page.goto(url);
    await page.locator('#email').fill('owner@example.test'); await page.locator('#password').fill(PASSWORD); await page.locator('#confirm').fill(PASSWORD); await page.locator('form button').click();
    await page.locator('.sg').waitFor();
    assert.equal(await page.locator('.sg').evaluate(el => el.tagName + el.open), 'DIALOGtrue', 'a modal dialog in the top layer');
    assert.equal(await page.locator('#sg-steps li').count(), 10);
    await title('set up your media server');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'sg-title', 'focus moves to the heading');
    assert.equal(await page.locator('#sg-primary').textContent(), 'Start');
    // Keyboard: Escape asks before leaving; Tab never leaves the dialog.
    await page.keyboard.press('Escape'); await page.locator('#sg-sheet').waitFor({ state: 'visible' });
    await page.getByRole('button', { name: 'Keep going', exact: true }).click(); await page.locator('#sg-sheet').waitFor({ state: 'hidden' });
    for (let i = 0; i < 25; i++) await page.keyboard.press('Tab');
    assert(await page.evaluate(() => !!document.activeElement.closest('.sg')), 'focus stays inside the guide');
    for (const width of [320, 390, 768, 1440]) {
      await page.setViewportSize({ width, height: 860 });
      await page.waitForFunction(() => { const s = document.querySelector('.sg-scroll'); return s.scrollWidth <= s.clientWidth + 1 && document.documentElement.scrollWidth <= innerWidth + 1; }, null, { timeout: 3000 });
    }
    await page.setViewportSize({ width: 1280, height: 860 });

    // Media folders: existing and missing folders are told apart before saving.
    execFileSync('python3',['-c',"import sqlite3,sys,time; c=sqlite3.connect(sys.argv[1]); c.execute('INSERT OR REPLACE INTO cache(key,value,saved) VALUES(?,?,?)',('tvmaze:search:zzq setup series','[]',int(time.time()))); c.commit()",path.join(data,'library.sqlite3')]);
    assert.deepEqual(await page.evaluate(async ()=>(await api('/api/libraries')).libraries),[]);
    await primary(); await title('Where is your media');
    assert.equal(await page.locator('[data-lib-kind]').count(),1);
    assert.equal(await page.locator('#sg-kind-0').inputValue(),'mixed');
    await page.waitForFunction(()=>document.querySelector('#sg-body').getAnimations({subtree:true}).every(a=>a.playState==='finished'));
    await page.screenshot({path:'/tmp/plexv2-setup-mixed.png'});

    // The folder browser works in any browser and any system: pick the folder from a list instead of typing it.
    await page.locator('[data-act="choose-root"][data-n="0"]').click();
    const picker = page.locator('dialog.fb'); await picker.waitFor();
    await page.locator('#fb-path').fill(media); await page.locator('#fb-path').press('Enter');
    await picker.locator('.fb-list button', { hasText: 'Movies' }).waitFor();
    assert.equal(await picker.locator('.fb-list button').count(), 1, 'only folders are listed, never files');
    await picker.locator('.fb-list button', { hasText: 'Movies' }).click();
    await picker.locator('#fb-path').waitFor(); await page.waitForFunction(() => document.querySelector('#fb-path').value.endsWith('Movies'));
    await page.locator('#fb-path').fill(path.join(media, 'nope')); await page.locator('#fb-path').press('Enter');
    await page.locator('#fb-msg.bad').waitFor();
    await page.locator('#fb-up').click();
    await page.locator('#fb-cancel').click(); await picker.waitFor({ state: 'detached' });
    assert.equal(await page.locator('.sg-root').count(), 0, 'cancelling adds nothing');
    await page.locator('[data-act="choose-root"][data-n="0"]').click(); await picker.waitFor();
    await page.locator('#fb-path').fill(path.join(media, 'Movies')); await page.locator('#fb-path').press('Enter');
    await page.waitForFunction(() => document.querySelector('#fb-path').value.endsWith('Movies') && !document.querySelector('#fb-use').disabled);
    await page.locator('#fb-use').click(); await picker.waitFor({ state: 'detached' });
    await page.locator('.sg-root', { hasText: 'Movies' }).waitFor();
    await page.locator('.sg-x').first().click();
    await page.locator('#sg-add-0').fill(path.join(media, 'Movies')); await page.locator('[data-act="add-root"][data-n="0"]').click();
    await page.locator('#sg-roots-0 .sg-pill[data-k="ok"]').waitFor();
    await page.locator('[data-act="add-library"]').click();
    await page.locator('#sg-kind-1').selectOption('video');
    await page.locator('#sg-add-1').fill('/definitely/missing/folder'); await page.locator('[data-act="add-root"][data-n="1"]').click();
    await page.locator('#sg-roots-1 .sg-pill[data-k="missing"]').waitFor();
    await page.locator('#sg-add-1').fill('relative/path'); await page.locator('[data-act="add-root"][data-n="1"]').click();
    await page.locator('#sg-roots-1 .sg-pill[data-k="invalid"]').waitFor();
    await page.locator('[data-act="remove-root"][data-n="1"]').first().click(); await page.locator('[data-act="remove-root"][data-n="1"]').first().click();
    assert.equal(await page.locator('#sg-roots-1 .sg-root').count(), 0);
    await page.locator('#sg-add-0').fill(path.join(media, 'Movies')); await page.locator('[data-act="add-root"][data-n="0"]').click();
    assert((await page.locator('#sg-msg-0').textContent()).includes('already'));
    await page.locator('[data-act="remove-root"][data-n="0"]').click();
    await page.locator('#sg-add-0').fill(path.join(media,'Movies'));
    await primary(); await title('Other locations');
    const savedLibraries=await page.evaluate(async ()=>(await api('/api/libraries')).libraries);
    assert.equal(savedLibraries.length,1,'only the configured library is saved');
    assert.equal(savedLibraries[0].kind,'mixed');
    assert.equal(savedLibraries[0].roots.length,1);

    // Conversion folder: refuses media folders, accepts a new writable one.
    await page.locator('#sg-work').fill(path.join(media, 'Movies', 'conversion')); await page.locator('[data-act="save-work"]').click();
    await page.waitForFunction(() => document.querySelector('#sg-work-msg').dataset.bad === '1');
    await page.locator('#sg-work').fill(work); await page.locator('[data-act="save-work"]').click();
    await page.locator('#sg-work-msg', { hasText: 'Saved' }).waitFor();
    assert(fs.existsSync(work)); assert(!fs.existsSync(path.join(media, 'Movies', 'conversion')), 'a refused folder is not created');
    await primary(); await title('Memory and graphics');
    await page.locator('.sg-facts').waitFor();
    await page.locator('#sg-slots').fill('2'); await page.locator('input[name="sg-q"][value="speed"]').check();
    await primary(); await title('Read your library');
    const settings = await page.evaluate(async () => (await fetch('/api/owner/settings')).json());
    assert.equal(settings.effective_conversion_slots, 2); assert.equal(settings.transcode_quality, 'speed');

    // Scan really runs and reports what it found.
    await page.locator('#sg-scan-btn').click();
    await page.locator('.sg-result').waitFor({ timeout: 30000 });
    assert((await page.locator('.sg-result').textContent()).includes('1 movies'));
    const detected=await page.evaluate(async()=>await api('/api/files'));
    assert(detected.some(f=>f.result.kind==='video'),'camera clips are videos');
    assert(detected.some(f=>f.result.kind==='tv'),'episode filenames are series');
    assert.equal(detected.length,3);
    assert(detected.every(f=>f.library_ids.length===1 && f.library_ids[0]===savedLibraries[0].id),'one folder feeds one mixed library');
    await primary(); await title('Who will use this');
    await page.locator('#sg-a-email').fill('viewer@example.test'); await page.locator('#sg-a-pass').fill('viewer password 123'); await page.locator('#sg-a-pass2').fill('different password');
    await page.locator('[data-act="add-account"]').click(); await page.waitForFunction(() => document.querySelector('#sg-a-msg').dataset.bad === '1');
    await page.locator('#sg-a-pass2').fill('viewer password 123'); await page.locator('[data-act="add-account"]').click();
    await page.locator('#sg-accounts .sg-account', { hasText: 'viewer@example.test' }).waitFor();

    // API-key steps: skipping warns, names what is lost, and can be cancelled.
    await primary(); await title('Movie information');
    assert.equal(await page.locator('#sg-warn').getAttribute('inert'), '');
    await page.locator('.sg-skip').click();
    await page.waitForFunction(() => !document.querySelector('#sg-warn').hasAttribute('inert'));
    const warning = await page.locator('#sg-warn').innerText();
    for (const text of ['No posters', 'No movie collections', 'TV shows keep working', 'edit names, covers and episodes by hand']) assert(warning.includes(text), text);
    await page.getByRole('button', { name: 'Stay on this step', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('#sg-warn').hasAttribute('inert'));
    assert((await page.locator('.sg-card', { hasText: 'How to get it' }).innerText()).includes('API Read Access Token'));
    assert.equal(await page.locator('a[href="https://www.themoviedb.org/settings/api"]').getAttribute('rel'), 'noopener noreferrer');
    await page.locator('#sg-key-tmdb').fill('has a space'); await page.locator('[data-act="save-key"]').click();
    await page.waitForFunction(() => document.querySelector('#sg-key-msg-tmdb').dataset.bad === '1');
    assert.equal((await page.evaluate(async () => (await fetch('/api/owner/providers')).json())).tmdb_configured, false);
    await page.locator('.sg-skip').click(); await page.getByRole('button', { name: 'Skip anyway', exact: true }).click(); await title('Subtitle search');
    assert.equal(await page.locator('#sg-steps li[data-state="skipped"]').count(), 1);
    await page.locator('.sg-skip').click();
    assert((await page.locator('#sg-warn').innerText()).includes('online search'));
    await page.getByRole('button', { name: 'Skip anyway', exact: true }).click(); await title('Watch away from home');
    assert((await page.locator('.sg-pre').innerText()).includes('SMART_MEDIA_REMOTE_HOSTNAME'));
    assert((await page.locator('.sg-card').filter({ has: page.getByRole('heading', { name: 'Home network', exact: true }) }).innerText()).includes('127.0.0.1'));
    await primary(); await page.locator('.sg-summary').waitFor();
    assert.equal(await page.locator('.sg-summary li[data-state="open"]').count(), 3, 'accounts done; keys and remote open');
    await page.locator('.sg-summary li', { hasText: 'Movie data' }).getByRole('button', { name: 'Set up' }).click(); await title('Movie information');
    await page.locator('#sg-steps li').last().getByRole('button').click(); await page.locator('.sg-summary').waitFor();
    await page.getByRole('button', { name: 'Open my library', exact: true }).click(); await page.locator('.sg').waitFor({ state: 'detached' });
    await page.waitForFunction(() => !document.body.children.length || ![...document.body.children].some(e => e.hasAttribute('inert')));
    assert.equal((await page.evaluate(async () => (await fetch('/api/owner/settings')).json())).setup_guide, 'done');
    await page.locator('#account-controls .profile-trigger').waitFor();
    await page.reload(); await page.locator('#account-controls .profile-trigger').waitFor(); await page.waitForTimeout(500);
    assert.equal(await page.locator('.sg').count(), 0, 'a finished guide does not return');
    await page.evaluate(id=>view('library:'+id),savedLibraries[0].id);
    for(const name of ['Movies','TV shows','Videos & other files']) await page.locator('#app').getByRole('heading',{name,exact:true,level:2}).waitFor();
    assert.equal(await page.locator('#app .card').count(),3);
    await page.screenshot({path:'/tmp/plexv2-library-mixed.png'});
    for(const width of [320,390,768,1440]){
      await page.setViewportSize({width,height:860});
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'mixed library fits '+width);
    }
    await page.setViewportSize({width:1280,height:860});


    // Using a feature that needs a missing key explains it, in place.
    await page.evaluate(() => view('subtitles')); await page.locator('#subtitle-query').fill('The Matrix'); await page.getByRole('button', { name: 'Search', exact: true }).click();
    const help = page.locator('.sg-help'); await help.waitFor();
    assert((await help.innerText()).toLowerCase().includes('opensubtitles key needed'));
    assert.deepEqual(await page.evaluate(() => [subtitleLink('https://www.opensubtitles.com/en/x'), subtitleLink('javascript:alert(1)'), subtitleLink('https://evil.test/opensubtitles.com'), subtitleLink('')]), ['https://www.opensubtitles.com/en/x', '', '', '']);
    for (const text of ['opensubtitles.com/consumers', 'API key', 'Save and test', 'OPENSUBTITLES_API_KEY']) assert((await help.innerHTML()).includes(text), text);
    await page.locator('#sg-key-opensubtitles').fill('bad key'); await page.locator('[data-act="save-key"]').click();
    await page.waitForFunction(() => document.querySelector('#sg-key-msg-opensubtitles').dataset.bad === '1');
    await page.keyboard.press('Escape'); await help.waitFor({ state: 'detached' });
    await page.evaluate(() => api('/api/metadata/search?kind=movie&q=x').catch(() => {}));
    await page.locator('.sg-help h2', { hasText: 'TMDB key needed' }).waitFor();
    await page.locator('#sg-help-close').click(); await page.locator('.sg-help').waitFor({ state: 'detached' });
    await page.evaluate(() => { window.__scan = PlatnoSetup.beforeScan().then(v => { window.__allowed = v; }); });
    await page.locator('.sg-help').waitFor(); assert((await page.locator('.sg-help').innerText()).includes('What happens if you scan without it'));
    await page.getByRole('button', { name: 'Scan anyway', exact: true }).click(); await page.waitForFunction(() => window.__allowed === true);
    assert.equal(await page.evaluate(() => PlatnoSetup.beforeScan().then(v => v)), true, 'the scan warning is shown once per session');

    // Non-owners never see the guide. An administrator who hits a missing key is told to ask the owner.
    await page.evaluate(() => post('/api/admin/accounts', { email: 'admin@example.test', password: 'admin password 1234', confirm_password: 'admin password 1234', role: 'admin' }));
    for (const [email, password, role] of [['viewer@example.test', 'viewer password 123', 'user'], ['admin@example.test', 'admin password 1234', 'admin']]) {
      const personContext = await browser.newContext({ viewport: { width: 390, height: 800 } }), person = watch(await personContext.newPage());
      await person.goto(url); await person.locator('#email').fill(email); await person.locator('#password').fill(password); await person.locator('form button').click();
      await person.locator('#account-controls .profile-trigger').waitFor(); await person.waitForTimeout(500);
      assert.equal(await person.locator('.sg').count(), 0, `${role} must not see the owner guide`);
      if (role === 'admin') {
        await person.evaluate(() => api('/api/subtitles/search?q=x').catch(() => {}));
        await person.locator('.sg-help', { hasText: 'Ask the server owner' }).waitFor();
        assert.equal(await person.locator('.sg-help input').count(), 0, 'no key field for non-owners');
      }
      await personContext.close();
    }

    // Reduced motion: the guide still works, without animation.
    const reducedContext = await browser.newContext({ viewport: { width: 1280, height: 860 }, reducedMotion: 'reduce' }), reduced = watch(await reducedContext.newPage());
    await reduced.goto(url); await reduced.locator('#email').fill('owner@example.test'); await reduced.locator('#password').fill(PASSWORD); await reduced.locator('form button').click();
    await reduced.locator('#account-controls .profile-trigger').waitFor();
    await reduced.evaluate(() => PlatnoSetup.open()); await reduced.locator('.sg').waitFor();
    assert.equal(await reduced.locator('.sg-body [data-stagger]').first().evaluate(el => getComputedStyle(el).animationName), 'none', 'the app-wide reduced-motion rule removes animation');
    assert.equal(await reduced.locator('.sg-hero-line').evaluate(el => getComputedStyle(el).strokeDashoffset), '0px', 'drawn artwork is complete without animation');
    await reduced.locator('#sg-primary').click(); await reduced.locator('#sg-title', { hasText: 'Where is your media' }).waitFor();
    await reduced.getByRole('button', { name: 'Close guide', exact: true }).click(); await reduced.getByRole('button', { name: 'Remind me next time', exact: true }).click();
    await reduced.locator('.sg').waitFor({ state: 'detached' });
    await reducedContext.close();

    assert.deepEqual(errors, [], JSON.stringify(errors));
    console.log('PASS: first-run guide (folders, conversion folder, hardware facts, scan, accounts, key steps with skip warnings, remote, summary), re-run, missing-key help, owner-only, keyboard focus, reduced motion, 320-1440px layout');
  } finally {
    if (browser) await browser.close();
    server.kill('SIGTERM'); await new Promise(r => setTimeout(r, 300));
    fs.rmSync(tmp, { recursive: true, force: true });
  }
})().catch(e => { console.error(e); process.exitCode = 1; });
