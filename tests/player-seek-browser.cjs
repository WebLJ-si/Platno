// Real-Chrome test of the seek bar and its hover preview on a direct-play MP4 and on a converted (HLS) video.
// Needs the Node `playwright` package and ffmpeg.
const assert = require('node:assert/strict'), { chromium } = require('playwright'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), net = require('node:net'), { spawn, execFileSync } = require('node:child_process');
const PASSWORD = 'seek password 12345';
(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'plexv2-seek-')), data = path.join(tmp, 'data'), media = path.join(tmp, 'media');
  fs.mkdirSync(data); fs.mkdirSync(media);
  const make = (name, codec) => execFileSync('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=640x360:rate=24:duration=90', '-f', 'lavfi', '-i', 'sine=frequency=330:duration=90', '-c:v', 'libx264', '-preset', 'ultrafast', '-g', '48', '-pix_fmt', 'yuv420p', '-c:a', codec, '-movflags', '+faststart', path.join(media, name)]);
  make('Direct.Seek.Film.2019.mp4', 'aac'); make('Converted.Seek.Film.2020.mkv', 'ac3');
  const port = await new Promise(r => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
  const binary = process.env.PLEXV2_TEST_BINARY || path.resolve('target/release/smart-media-scanner');
  const server = spawn(binary, [], { env: { ...process.env, SMART_MEDIA_DATA: data, SMART_MEDIA_HOST: '127.0.0.1', SMART_MEDIA_PORT: String(port), TMDB_TOKEN: '', OPENSUBTITLES_API_KEY: '', SMART_MEDIA_COOKIE_SECURE: '0', SMART_MEDIA_WIKIDATA: 'off' }, stdio: 'ignore' }), url = 'http://127.0.0.1:' + port;
  let browser;
  try {
    for (let i = 0; i < 150; i++) { try { await fetch(url + '/api/auth/setup-status'); break; } catch { await new Promise(r => setTimeout(r, 100)); } }
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const errors = [], page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
    page.on('pageerror', e => errors.push(e.message));
    await page.goto(url);
    await page.locator('#email').fill('owner@example.test'); await page.locator('#password').fill(PASSWORD); await page.locator('#confirm').fill(PASSWORD); await page.locator('form button').click();
    await page.locator('.sg').waitFor();
    await page.evaluate(() => post('/api/owner/settings', { key: 'setup_guide', value: 'done' }));
    await page.reload(); await page.locator('#account-controls .profile-trigger').waitFor();
    await page.evaluate(async root => { await post('/api/scan', { roots: [root], full_scan: true }); while ((await api('/api/status')).running) await new Promise(r => setTimeout(r, 200)); await refreshFiles(); }, media);
    const state = () => page.evaluate(() => { const s = document.querySelector('#player-seek'), v = document.querySelector('#player-video'); return { value: s ? Number(s.value) : null, max: s ? Number(s.max) : null, disabled: s?.disabled, time: v.currentTime, mode: playerState.mode }; });
    for (const [name, mode] of [['Direct.Seek.Film', 'direct'], ['Converted.Seek.Film', 'transcode']]) {
      await page.evaluate(n => playFile(files.findIndex(f => f.path.includes(n))), name);
      // The bar exists and is usable as soon as the duration is known, before the first frame plays.
      await page.waitForFunction(() => { const s = document.querySelector('#player-seek'); return s && !s.disabled && Number(s.max) > 80; }, null, { timeout: 15000 });
      await page.waitForFunction(() => document.querySelector('#player-video').currentTime > 0.5, null, { timeout: 30000 });
      assert.equal((await state()).mode, mode);
      await page.mouse.move(640, 500); await page.waitForTimeout(200);
      const box = await page.locator('#player-seek').boundingBox();
      // The preview label names the frame that is shown, and the frames step in 5 seconds.
      await page.mouse.move(box.x + box.width * 0.4, box.y + box.height / 2); await page.mouse.move(box.x + box.width * 0.41, box.y + box.height / 2);
      await page.waitForFunction(() => { const b = document.querySelector('#seek-preview'); return !b.hidden && b.querySelector('img').naturalWidth === 240; }, null, { timeout: 15000 });
      assert.equal(await page.locator('#seek-preview span').innerText(), '0:35', 'hover at 36.9 s previews the 35 s frame');
      await page.mouse.click(box.x + box.width * 0.5, box.y + box.height / 2);
      await page.waitForFunction(() => Math.abs(Number(document.querySelector('#player-seek').value) - 45) < 3, null, { timeout: 20000 });
      // After seeking, the bar keeps following playback (it used to freeze while it had focus).
      await page.waitForFunction(() => document.querySelector('#player-video').currentTime > 0.5 || playerState.mode === 'direct', null, { timeout: 20000 });
      const before = (await state()).value; await page.waitForTimeout(3500); const after = await state();
      assert(after.value >= before + 2, `${mode}: the bar follows playback after a seek (${before} -> ${after.value})`);
      // Secondary controls stay in a single, keyboard-dismissable settings panel.
      await page.locator('#player-settings-toggle').click();
      assert(await page.locator('#player-settings').isVisible());
      assert.equal(await page.locator('.player-action-row button').count(), 3);
      assert(await page.locator('#player-settings #player-quality').isVisible());
      await page.locator('#player-quality').click();await page.keyboard.press('Escape');
      assert(await page.locator('#player-settings').isVisible(),'Escape closes the picker before settings');
      await page.keyboard.press('Escape');
      assert(await page.locator('#player-settings').isHidden());
      assert(await page.locator('#dialog').isVisible(), 'Escape dismisses settings before the player');
      await page.locator('#player-settings-toggle').click();
      // Fullscreen takes the player area (a <dialog> itself cannot go fullscreen) and leaves again.
      await page.mouse.move(640, 500); await page.locator('button[aria-label="Fullscreen"]').click();
      await page.waitForFunction(() => document.fullscreenElement?.id === 'player-shell', null, { timeout: 5000 });
      const fullscreen = await page.evaluate(() => { const r = document.querySelector('#player-shell').getBoundingClientRect(); return [Math.round(r.width), Math.round(r.height), innerWidth, innerHeight]; });
      assert.deepEqual(fullscreen.slice(0, 2), fullscreen.slice(2), `${mode}: the player fills the screen`);
      await page.mouse.move(600, 450); await page.locator('button[aria-label="Fullscreen"]').click(); await page.waitForFunction(() => !document.fullscreenElement, null, { timeout: 5000 });
      await page.locator('#player-settings-toggle').click();
      await page.locator('#player-shell').focus(); await page.keyboard.press('f'); await page.waitForFunction(() => document.fullscreenElement?.id === 'player-shell', null, { timeout: 5000 }); await page.keyboard.press('f'); await page.waitForFunction(() => !document.fullscreenElement, null, { timeout: 5000 });
      // Episode and settings panels are mutually exclusive; selectors keep working.
      await page.evaluate(() => { const f=files.find(x=>x.path===playerState.path); f.result={...f.result,kind:'tv',provider:'tvmaze',provider_id:999,season:1,episodes:[1],confidence:100}; const actions=document.querySelector('.player-top-actions'); actions.insertAdjacentHTML('afterbegin',episodeSelectors(f)); document.querySelector('#player-shell').insertAdjacentHTML('beforeend',episodeSelectors(f,true)); });
      await page.locator('#player-episodes-toggle').click();
      assert(await page.locator('#player-season').isVisible());
      await page.locator('#player-settings-toggle').click();
      assert(await page.locator('#player-episodes').isHidden());
      for(const [width,height] of [[320,640],[390,844],[768,900],[844,390],[1280,800]]){
        await page.setViewportSize({width,height});
        assert(await page.locator('#player-settings').evaluate(el=>{const r=el.getBoundingClientRect(),shell=document.querySelector('#player-shell').getBoundingClientRect();return r.left>=shell.left&&r.right<=shell.right+1&&r.top>=shell.top&&r.bottom<=shell.bottom+1&&el.scrollWidth<=el.clientWidth+1}), `settings fit ${width}x${height}`);
        assert(await page.locator('#player-shell').evaluate(el=>el.scrollWidth<=el.clientWidth+1));
      }
      await page.locator('#player-settings').evaluate(el=>el.scrollTop=0);
      await page.screenshot({path:'/tmp/plexv2-player-settings.png'});
      await page.locator('#player-settings-toggle').click();
      await page.screenshot({path:'/tmp/plexv2-player-clean.png'});
      // Leaving the player with the back arrow must also leave fullscreen.
      await page.setViewportSize({ width: 1280, height: 800 });
      // Switching episode (the player is rebuilt) must keep the page in fullscreen.
      await page.mouse.move(640, 500); await page.locator('button[aria-label="Fullscreen"]').click();
      await page.waitForFunction(() => document.fullscreenElement?.id === 'player-shell', null, { timeout: 5000 });
      await page.evaluate(() => { const i = files.findIndex(f => f.path === playerState.path); playFile(i); });
      await page.waitForTimeout(1500);
      assert.equal(await page.evaluate(() => document.fullscreenElement?.id), 'player-shell', `${mode}: switching episode must not leave fullscreen`);
      await page.evaluate(() => document.exitFullscreen()); await page.waitForFunction(() => !document.fullscreenElement, null, { timeout: 5000 });
      await page.mouse.move(640, 500); await page.locator('button[aria-label="Fullscreen"]').click();
      await page.waitForFunction(() => document.fullscreenElement?.id === 'player-shell', null, { timeout: 5000 });
      await page.locator('button[aria-label="Back to library"]').click();
      await page.waitForFunction(() => !document.fullscreenElement, null, { timeout: 5000 });
      assert(await page.locator('#dialog').isHidden(), `${mode}: the player closed`);
      await page.evaluate(() => dialog.close()); await page.waitForTimeout(300);
    }
    assert.deepEqual(errors, []);
    console.log('PASS: seek bar is usable early, follows playback after seeking, and its preview label matches the frame (direct and converted)');
  } finally { await browser?.close(); server.kill(); fs.rmSync(tmp, { recursive: true, force: true }); }
})().catch(e => { console.error(e); process.exit(1); });
