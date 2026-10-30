// Real-Chrome test of layout-problem reporting: the healthy app raises nothing at four widths, and deliberately
// broken layouts (the old play button over a title, page overflow, clipped button text, overlapping buttons,
// a control pushed off screen) produce flagged-geometry reports that carry no text.
// Needs the Node `playwright` package and ffmpeg.
const assert = require('node:assert/strict'), { chromium } = require('playwright'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), net = require('node:net'), { spawn, execFileSync } = require('node:child_process');
const PASSWORD = 'layout password 12345', SECRET_TEXT = 'Private movie title that must never be reported';
(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'plexv2-layout-')), data = path.join(tmp, 'data'), media = path.join(tmp, 'media');
  fs.mkdirSync(data); fs.mkdirSync(media);
  for (const name of ['2.Fast.2.Furious.2003.mp4', 'Second.Film.Title.2011.mp4']) execFileSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=10:duration=20', '-f', 'lavfi', '-i', 'sine=frequency=330:duration=20', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest', path.join(media, name)]);
  const port = await new Promise(r => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
  const binary = process.env.PLEXV2_TEST_BINARY || path.resolve('target/release/smart-media-scanner');
  const server = spawn(binary, [], { env: { ...process.env, SMART_MEDIA_DATA: data, SMART_MEDIA_HOST: '127.0.0.1', SMART_MEDIA_PORT: String(port), TMDB_TOKEN: '', SMART_MEDIA_COOKIE_SECURE: '0', SMART_MEDIA_TELEMETRY: 'off' }, stdio: 'ignore' });
  const url = 'http://127.0.0.1:' + port;
  let browser;
  try {
    for (let i = 0; i < 150; i++) { try { await fetch(url + '/api/auth/setup-status'); break; } catch { await new Promise(r => setTimeout(r, 100)); } }
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const errors = [], reports = [], page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
    page.on('pageerror', e => errors.push(e.message));
    await page.route('**/api/telemetry/config', route => route.fulfill({ json: { available: true, enabled: true, destination: 'collector.example.test', release: '0.2.2', channel: 'beta', serverUptimeSeconds: 5 } }));
    await page.route('**/api/telemetry/report', async route => { reports.push(route.request().postDataJSON()); await route.fulfill({ status: 202, body: '' }); });
    await page.goto(url);
    await page.locator('#email').fill('owner@example.test'); await page.locator('#password').fill(PASSWORD); await page.locator('#confirm').fill(PASSWORD); await page.locator('form button').click();
    await page.locator('.sg').waitFor();
    await page.evaluate(() => post('/api/owner/settings', { key: 'setup_guide', value: 'done' }));
    await page.reload(); await page.locator('#account-controls .profile-trigger').waitFor();
    await page.evaluate(async root => { await post('/api/scan', { roots: [root], full_scan: true }); while ((await api('/api/status')).running) await new Promise(r => setTimeout(r, 200)); await refreshFiles(); for (const f of files) await post('/api/playback/progress', { path: f.path, position: 20, duration: 120 }); }, media);
    await page.reload(); await page.locator('.resume-card').first().waitFor();
    await page.waitForFunction(() => window.PlatnoTelemetry?.getState().enabled);
    const problems = () => page.evaluate(() => [...window.PlatnoTelemetry.layoutProblems()].map(([code, v]) => code + ':' + v.count));

    // 1. The healthy interface stays quiet on every main screen at four widths.
    for (const [w, h] of [[1280, 800], [768, 900], [390, 844], [320, 640]]) {
      await page.setViewportSize({ width: w, height: h });
      for (const view of ['dashboard', 'movies', 'shows', 'collections', 'settings']) {
        await page.evaluate(v => window.view(v), view); await page.waitForTimeout(500);
        assert.deepEqual(await problems(), [], `${view} at ${w}px reported layout problems`);
      }
    }
    await page.setViewportSize({ width: 1280, height: 800 }); await page.evaluate(() => window.view('dashboard')); await page.waitForTimeout(400);
    assert.equal(reports.filter(r => r.kind === 'ui_issue').length, 0, 'nothing was sent for a healthy interface');

    // 2. The old bug: the play button covering the first letters of a title on a card without artwork.
    await page.addStyleTag({ content: '.resume-art .resume-play{left:12px!important;right:auto!important;bottom:10px!important}.resume-art .poster-fallback{padding-right:16px!important}' });
    await page.locator('.resume-play').first().scrollIntoViewIfNeeded(); await page.waitForTimeout(300);
    assert((await problems()).some(p => p.startsWith('layout-overlap')), 'play button over the title is detected');
    await page.evaluate(() => window.scrollTo(0, 0));

    // A fixed bar over a button that scrolls beneath it (the phone navigation) is normal, not a problem.
    await page.evaluate(() => { const a = document.createElement('button'); a.id = 'test-under'; a.textContent = 'Under the bar'; a.style.cssText = 'position:relative;display:block;margin:12px 0'; document.querySelector('main').prepend(a); window.scrollTo(0, 0); const r = a.getBoundingClientRect(), b = document.createElement('button'); b.id = 'test-bar'; b.textContent = 'Fixed bar'; b.style.cssText = `position:fixed;left:${r.left}px;top:${r.top}px;width:${r.width}px;height:${r.height}px;z-index:9`; document.body.append(b); });
    assert.deepEqual(await problems(), [], 'a fixed bar over scrolling content is not reported');
    await page.evaluate(() => document.querySelectorAll('#test-under,#test-bar').forEach(n => n.remove()));

    // 3. Page overflow, clipped button text, overlapping buttons, all with private text that must not leak.
    await page.evaluate(secret => {
      const main = document.querySelector('main');
      const wide = document.createElement('div'); wide.id = 'test-wide'; wide.style.cssText = 'width:' + (innerWidth + 400) + 'px;height:12px;background:#333'; wide.title = secret; main.append(wide);
      const clip = document.createElement('button'); clip.id = 'test-clip'; clip.textContent = secret; clip.style.cssText = 'position:fixed;left:20px;top:150px;z-index:5;width:60px;overflow:hidden;white-space:nowrap;text-overflow:clip'; main.append(clip);
      const a = document.createElement('button'), b = document.createElement('button'); a.textContent = secret; b.textContent = 'Other'; a.style.cssText = 'position:fixed;left:300px;top:300px;width:120px;height:40px;z-index:5'; b.style.cssText = 'position:fixed;left:330px;top:305px;width:120px;height:40px;z-index:5'; main.append(a, b);
    }, SECRET_TEXT);
    await page.waitForTimeout(300);
    const found = await problems();
    for (const code of ['layout-overflow', 'layout-clipped', 'layout-overlap']) assert(found.some(p => p.startsWith(code)), code + ' is detected: ' + found);

    // The healthy player (controls, then the settings panel open) stays quiet at desktop, phone and landscape sizes.
    await page.evaluate(() => document.querySelectorAll('#test-wide,#test-clip').forEach(n => n.remove()));
    await page.evaluate(() => { document.querySelectorAll('button').forEach(b => { if (b.style.position === 'fixed') b.remove(); }); });
    await page.evaluate(() => { playFile(files.findIndex(f => f.path.includes('Second.Film'))); });
    await page.waitForFunction(() => document.querySelector('#player-video')?.currentTime > 0.5, null, { timeout: 30000 }).catch(() => {});
    for (const [w, h] of [[1280, 800], [390, 844], [844, 390]]) {
      await page.setViewportSize({ width: w, height: h }); await page.mouse.move(w / 2, h / 3); await page.waitForTimeout(700);
      assert.deepEqual(await problems(), [], `player controls at ${w}x${h}`);
      await page.locator('#player-settings-toggle').click(); await page.waitForTimeout(500);
      assert.deepEqual(await problems(), [], `player settings at ${w}x${h}`);
      await page.locator('#player-settings .player-panel-heading button').click(); await page.waitForTimeout(300);
    }
    await page.evaluate(() => dialog.close()); await page.setViewportSize({ width: 1280, height: 800 });

    // 4. A control pushed outside the screen inside the player.
    await page.evaluate(() => { document.querySelectorAll('#test-wide,#test-clip').forEach(n => n.remove()); });
    await page.evaluate(() => playFile(files.findIndex(f => f.path.includes('Second.Film'))));
    await page.waitForFunction(() => document.querySelector('#player-video')?.currentTime > 0.5, null, { timeout: 30000 }).catch(() => {});
    await page.evaluate(() => { const b = document.querySelector('#player-fullscreen'); if (b) { b.style.position = 'fixed'; b.style.left = '-80px'; b.style.top = '20px'; } });
    assert((await problems()).some(p => p.startsWith('layout-offscreen')), 'a control outside the screen is detected');
    await page.evaluate(() => dialog.close());

    // 5. Reports are sent once, carry flagged boxes and a count, and never contain any of the page's text.
    await page.evaluate(() => document.querySelectorAll('#test-wide,#test-clip').forEach(n => n.remove()));
    await page.evaluate(() => { const w = document.createElement('div'); w.style.cssText = 'width:' + (innerWidth + 500) + 'px;height:10px'; document.querySelector('main').append(w); });
    await page.evaluate(() => window.PlatnoTelemetry.checkLayout());
    await page.evaluate(() => window.PlatnoTelemetry.checkLayout());
    const issues = reports.filter(r => r.kind === 'ui_issue');
    assert(issues.length >= 1, 'a ui_issue report was sent');
    const sent = issues.find(r => r.code === 'layout-overflow') || issues[0];
    assert(Number.isInteger(sent.count) && sent.count >= 1);
    const snapshot = sent.replay.filter(s => s.boxes).at(-1);
    assert(snapshot && snapshot.boxes.some(b => b.flag === true), 'the snapshot has flagged boxes');
    assert(snapshot.boxes.every(b => Object.keys(b).every(k => ['x', 'y', 'w', 'h', 'kind', 'flag'].includes(k))), 'boxes hold geometry only');
    const duplicate = issues.filter(r => r.code === sent.code && r.scene === sent.scene).length;
    assert.equal(duplicate, 1, 'the same problem is reported once per session');
    const body = JSON.stringify(reports);
    assert(!body.includes(SECRET_TEXT) && !body.includes('Furious') && !body.includes('owner@example.test'), 'no page text leaves the browser');
    assert.deepEqual(errors, []);
    console.log('PASS: no layout reports on the healthy interface at four widths; play-button overlap, overflow, clipped text, overlapping buttons and off-screen controls are detected; reports hold flagged boxes and counts only, once per session');
  } finally { await browser?.close(); server.kill(); fs.rmSync(tmp, { recursive: true, force: true }); }
})().catch(e => { console.error(e); process.exit(1); });
