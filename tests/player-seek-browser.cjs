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
      await page.evaluate(() => dialog.close()); await page.waitForTimeout(300);
    }
    assert.deepEqual(errors, []);
    console.log('PASS: seek bar is usable early, follows playback after seeking, and its preview label matches the frame (direct and converted)');
  } finally { await browser?.close(); server.kill(); fs.rmSync(tmp, { recursive: true, force: true }); }
})().catch(e => { console.error(e); process.exit(1); });
