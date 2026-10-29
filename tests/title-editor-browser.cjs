// Real-Chrome test of the no-key experience: unidentified titles are named from file names, grouped,
// and can be edited by hand (name, details, cover, episodes). Needs the Node `playwright` package.
const assert = require('node:assert/strict'), { chromium } = require('playwright'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), net = require('node:net'), zlib = require('node:zlib'), { spawn, execFileSync } = require('node:child_process');
const PASSWORD = 'editor password 12345';
function png() { // a real 2x3 PNG, large enough to pass the server's minimum size check
  const crc = b => { let c, t = []; for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } let r = 0xFFFFFFFF; for (const x of b) r = t[(r ^ x) & 255] ^ (r >>> 8); return (r ^ 0xFFFFFFFF) >>> 0; };
  const chunk = (type, data) => { const body = Buffer.concat([Buffer.from(type), data]), out = Buffer.alloc(body.length + 8); out.writeUInt32BE(data.length, 0); body.copy(out, 4); out.writeUInt32BE(crc(body), body.length + 4); return out; };
  const header = Buffer.alloc(13); header.writeUInt32BE(40, 0); header.writeUInt32BE(60, 4); header[8] = 8; header[9] = 2;
  const rows = Buffer.concat(Array.from({ length: 60 }, (_, y) => Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: 120 }, (_, i) => (i * 7 + y * 3) & 255))])));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), chunk('IHDR', header), chunk('IDAT', zlib.deflateSync(rows)), chunk('IEND', Buffer.alloc(0))]);
}
(async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'plexv2-editor-')), data = path.join(tmp, 'data'), media = path.join(tmp, 'media');
  fs.mkdirSync(data);
  const files = ['Movies/Unknown.Home.Film.2021.mkv', 'Shows/Zzq Testshow Qx/Season 01/Zzq.Testshow.Qx.S01E01.mkv', 'Shows/Zzq Testshow Qx/Season 01/Zzq.Testshow.Qx.S01E02.mkv', 'Shows/Zzq Testshow Qx/Season 02/Zzq.Testshow.Qx.S02E01.mkv'];
  for (const f of files) { fs.mkdirSync(path.dirname(path.join(media, f)), { recursive: true }); fs.writeFileSync(path.join(media, f), 'not a real video'); }
  const port = await new Promise(r => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
  const binary = process.env.PLEXV2_TEST_BINARY || path.resolve('target/release/smart-media-scanner');
  const env = { ...process.env, SMART_MEDIA_DATA: data, SMART_MEDIA_HOST: '127.0.0.1', SMART_MEDIA_PORT: String(port), TMDB_TOKEN: '', OPENSUBTITLES_API_KEY: '', SMART_MEDIA_COOKIE_SECURE: '0', SMART_MEDIA_WIKIDATA: 'off' };
  const server = spawn(binary, [], { env, stdio: 'ignore' }), url = 'http://127.0.0.1:' + port;
  let browser;
  try {
    for (let i = 0; i < 150; i++) { try { await fetch(url + '/api/auth/setup-status'); break; } catch { await new Promise(r => setTimeout(r, 100)); } }
    execFileSync('sqlite3', [path.join(data, 'library.sqlite3'), `INSERT INTO cache VALUES('tvmaze:search:zzq testshow qx','[]',strftime('%s','now'));`]);
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const errors = [], watch = page => { page.on('pageerror', e => errors.push(e.message)); page.on('console', m => { if (m.type() === 'error' && !/Failed to load resource/.test(m.text())) errors.push(m.text()); }); return page; };
    const page = watch(await (await browser.newContext({ viewport: { width: 1280, height: 900 } })).newPage());
    await page.goto(url);
    await page.locator('#email').fill('owner@example.test'); await page.locator('#password').fill(PASSWORD); await page.locator('#confirm').fill(PASSWORD); await page.locator('form button').click();
    await page.locator('.sg').waitFor();
    await page.evaluate(() => post('/api/owner/settings', { key: 'setup_guide', value: 'done' }));
    await page.reload(); await page.locator('#account-controls .profile-trigger').waitFor();
    const scan = async () => { await page.evaluate(async root => { await post('/api/scan', { roots: [root], full_scan: true }); while ((await api('/api/status')).running) await new Promise(r => setTimeout(r, 200)); await refreshFiles(); }, media); };
    await scan();

    // Unidentified titles still appear, with a name taken from the file name and a default picture.
    await page.evaluate(() => view('movies'));
    const card = page.locator('.card', { hasText: 'Unknown Home Film' });
    await card.waitFor();
    assert((await card.innerText()).includes('2021'));
    assert.equal(await card.locator('.poster-fallback').isVisible(), true, 'default picture');
    await card.click();
    await page.locator('.detail-copy h1', { hasText: 'Unknown Home Film' }).waitFor();
    assert((await page.locator('.detail-copy').innerText()).toLowerCase().includes('named from file name'));
    assert.equal(await page.locator('#detail-rating').textContent(), 'No rating');

    // Edit name, details and cover.
    await page.getByRole('button', { name: '✎ Edit details' }).click();
    await page.locator('#edit-name').fill('The Home Film'); await page.locator('#edit-year').fill('2020');
    await page.locator('#edit-genres').fill('Drama, Family'); await page.locator('#edit-description').fill('Filmed at home.\nSecond line.');
    await page.locator('#edit-year').fill('abc'); await page.locator('#edit-save').click();
    await page.waitForFunction(() => document.querySelector('#edit-msg').dataset.bad === '1');
    await page.locator('#edit-year').fill('2020');
    await page.locator('#edit-cover-file').setInputFiles({ name: 'cover.png', mimeType: 'image/png', buffer: png() });
    await page.locator('#edit-cover-preview img').waitFor();
    await page.locator('#edit-cover-file').setInputFiles({ name: 'bad.png', mimeType: 'image/png', buffer: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>'.padEnd(200)) });
    await page.waitForFunction(() => document.querySelector('#edit-cover-msg').dataset.bad === '1');
    await page.locator('#edit-save').click();
    await page.locator('.detail-copy h1', { hasText: 'The Home Film' }).waitFor();
    const cover = await page.locator('.detail-poster img').getAttribute('src');
    assert(/^\/api\/covers\/[0-9a-f]{32}\.png$/.test(cover), cover);
    assert.equal(await page.evaluate(async c => { const r = await fetch(c); return r.status + r.headers.get('content-type') + r.headers.get('x-content-type-options'); }, cover), '200image/pngnosniff');
    const facts = await page.locator('.detail-copy').innerText();
    for (const text of ['2020', 'Filmed at home.']) assert(facts.includes(text), text);
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    await scan(); // a full rescan must not undo hand edits
    await page.evaluate(() => view('movies'));
    await page.locator('.card', { hasText: 'The Home Film' }).locator('img').waitFor();
    assert.equal(await page.locator('.card', { hasText: 'Unknown Home Film' }).count(), 0);
    await page.locator('.card', { hasText: 'The Home Film' }).click();
    await page.getByRole('button', { name: '✎ Edit details' }).click();
    await page.locator('#edit-cover-remove').click(); await page.locator('#edit-save').click();
    await page.locator('.detail-copy h1', { hasText: 'The Home Film' }).waitFor();
    await page.waitForFunction(() => !document.querySelector('.detail-poster img'));
    await page.getByRole('button', { name: 'Close', exact: true }).click();

    // A series without a provider still lists its seasons and episodes, and they can be corrected.
    await page.evaluate(() => view('shows'));
    await page.locator('.card', { hasText: 'Zzq Testshow Qx' }).click();
    await page.locator('#episodes .episode-season').first().waitFor();
    assert.equal(await page.locator('#episodes .episode-season').count(), 2);
    assert.equal(await page.locator('#episodes .episode').count(), 3);
    assert.deepEqual(await page.locator('#detail-season option').allTextContents(), ['Season 1', 'Season 2']);
    await page.getByRole('button', { name: '✎ Edit details' }).click();
    const rows = page.locator('.edit-episode');
    assert.equal(await rows.count(), 3);
    const second = rows.filter({ hasText: 'S01E02' });
    await second.locator('.ee-numbers').fill('0'); await second.getByRole('button', { name: 'Save episode' }).click();
    await second.locator('.edit-status[data-bad="1"]').waitFor();
    await second.locator('.ee-season').fill('3'); await second.locator('.ee-numbers').fill('5, 6'); await second.locator('.ee-title').fill('The Long One');
    await second.getByRole('button', { name: 'Save episode' }).click(); await second.locator('.edit-status', { hasText: 'Saved' }).waitFor();
    await page.locator('#edit-name').fill('Testshow Renamed'); await page.locator('#edit-save').click();
    await page.locator('.detail-copy h1', { hasText: 'Testshow Renamed' }).waitFor();
    await page.locator('#episodes .episode-season').first().waitFor();
    assert.deepEqual(await page.locator('#detail-season option').allTextContents(), ['Season 1', 'Season 2', 'Season 3']);
    assert((await page.locator('#episodes').innerText()).includes('5. The Long One'));
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    // A new episode of the renamed show joins it after a rescan.
    fs.writeFileSync(path.join(media, 'Shows/Zzq Testshow Qx/Season 01/Zzq.Testshow.Qx.S01E03.mkv'), 'not a real video');
    await scan();
    await page.evaluate(() => view('shows'));
    assert.equal(await page.locator('.card', { hasText: 'Zzq Testshow Qx' }).count(), 0, 'no duplicate show under the old name');
    await page.locator('.card', { hasText: 'Testshow Renamed' }).click();
    await page.locator('#episodes .episode-season').first().waitFor();
    assert.equal(await page.locator('#episodes .episode').count(), 5, 'S01E01, S01E03, S02E01 and the double episode 5 and 6');
    await page.getByRole('button', { name: 'Close', exact: true }).click();

    // Viewers can see covers but never edit.
    await page.evaluate(() => post('/api/admin/accounts', { email: 'viewer@example.test', password: 'viewer password 123', confirm_password: 'viewer password 123', role: 'user' }));
    const viewerContext = await browser.newContext({ viewport: { width: 390, height: 800 } }), viewer = watch(await viewerContext.newPage());
    await viewer.goto(url); await viewer.locator('#email').fill('viewer@example.test'); await viewer.locator('#password').fill('viewer password 123'); await viewer.locator('form button').click();
    await viewer.locator('#account-controls .profile-trigger').waitFor();
    await viewer.evaluate(() => view('shows')); await viewer.locator('.card', { hasText: 'Testshow Renamed' }).click();
    await viewer.locator('.detail-copy h1').waitFor();
    assert.equal(await viewer.getByRole('button', { name: '✎ Edit details' }).count(), 0);
    assert.equal(await viewer.evaluate(async () => (await fetch('/api/admin/title', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' })).status), 403);
    await viewerContext.close();
    for (const width of [320, 768]) {
      await page.setViewportSize({ width, height: 900 });
      await page.evaluate(() => view('movies')); await page.locator('.card', { hasText: 'The Home Film' }).click();
      await page.getByRole('button', { name: '✎ Edit details' }).click();
      await page.waitForFunction(() => { const d = document.querySelector('#dialog'); return d.scrollWidth <= d.clientWidth + 1; }, null, { timeout: 3000 });
      await page.keyboard.press('Escape');
    }
    assert.deepEqual(errors, [], JSON.stringify(errors));
    console.log('PASS: names and default pictures without a key, local show and episode grouping, hand-editing of name/details/cover/episodes, edits survive rescans, new episodes join renamed shows, viewers cannot edit, validation and layout');
  } finally {
    if (browser) await browser.close();
    server.kill('SIGTERM'); await new Promise(r => setTimeout(r, 300));
    fs.rmSync(tmp, { recursive: true, force: true });
  }
})().catch(e => { console.error(e); process.exitCode = 1; });
