// Real-Chrome test of the one-time diagnostics notice: the old top banner is gone, only the server owner sees the
// notice, once, the answer applies to the whole server and is remembered, and Settings keeps the controls.
// Needs the Node `playwright` package.
const assert = require('node:assert/strict'), { chromium } = require('playwright'), http = require('node:http'), fs = require('node:fs'), os = require('node:os'), path = require('node:path'), net = require('node:net'), { spawn } = require('node:child_process');
const PASSWORD = 'notice password 12345';
const freePort = () => new Promise(r => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
async function start(collector, dataDir) {
  const port = await freePort(), binary = process.env.PLEXV2_TEST_BINARY || path.resolve('target/release/smart-media-scanner');
  const proc = spawn(binary, [], { env: { ...process.env, SMART_MEDIA_DATA: dataDir, SMART_MEDIA_HOST: '127.0.0.1', SMART_MEDIA_PORT: String(port), TMDB_TOKEN: '', SMART_MEDIA_COOKIE_SECURE: '0', SMART_MEDIA_RELEASE_CHANNEL: 'beta', SMART_MEDIA_TELEMETRY_ENDPOINT: `http://127.0.0.1:${collector}/api/ingest` }, stdio: 'ignore' });
  const url = 'http://127.0.0.1:' + port;
  for (let i = 0; i < 150; i++) { try { await fetch(url + '/api/auth/setup-status'); break; } catch { await new Promise(r => setTimeout(r, 100)); } }
  return { proc, url };
}
(async () => {
  const received = [], collector = http.createServer((req, res) => { req.on('data', () => {}); req.on('end', () => { received.push(req.url); res.statusCode = 202; res.end(); }); });
  await new Promise(r => collector.listen(0, '127.0.0.1', r));
  const collectorPort = collector.address().port, tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'plexv2-notice-'));
  const procs = []; let browser;
  try {
    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const errors = [];
    const config = page => page.evaluate(async () => (await (await fetch('/api/telemetry/config')).json()));
    async function ownerPage(url) {
      const page = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
      page.on('pageerror', e => errors.push(e.message));
      await page.goto(url);
      await page.locator('#email').fill('owner@example.test'); await page.locator('#password').fill(PASSWORD); await page.locator('#confirm').fill(PASSWORD); await page.locator('form button').click();
      await page.locator('.sg').waitFor();
      await page.evaluate(() => post('/api/owner/settings', { key: 'setup_guide', value: 'done' }));
      return page;
    }

    // 1. A fresh beta server: the owner is asked once; there is no banner; "Keep it on" is remembered.
    const a = await start(collectorPort, fs.mkdtempSync(path.join(tmp, 'a-'))); procs.push(a.proc);
    const page = await ownerPage(a.url);
    assert.equal((await config(page)).noticePending, true);
    await page.reload();
    await page.locator('#diagnostics-notice').waitFor({ timeout: 15000 });
    assert.equal(await page.locator('#beta-diagnostics-notice').count(), 0, 'the old banner is gone');
    assert((await page.locator('#diagnostics-notice').innerText()).includes('whole server'));
    await page.screenshot({ path: '/tmp/platno-diagnostics-notice.png' });
    await page.locator('#diag-keep').click();
    await page.waitForFunction(() => !document.getElementById('diagnostics-notice'));
    const kept = await config(page); assert.equal(kept.noticePending, false); assert.equal(kept.enabled, true);
    await page.reload(); await page.locator('#account-controls .profile-trigger').waitFor(); await page.waitForTimeout(1500);
    assert.equal(await page.locator('#diagnostics-notice').count(), 0, 'not shown again after answering');
    assert.equal(await page.locator('#beta-diagnostics-notice').count(), 0, 'still no banner');
    // Settings keeps the controls, including the owner's server switch.
    await page.evaluate(() => view('settings')); await page.locator('.telemetry-panel').waitFor();
    assert(await page.locator('#telemetry-errors').isEnabled());
    assert(await page.getByLabel('Allow beta diagnostics for this server').isChecked());

    // 2. Turning it off applies to the whole server and is remembered across restarts.
    const b = await start(collectorPort, fs.mkdtempSync(path.join(tmp, 'b-'))); procs.push(b.proc);
    const second = await ownerPage(b.url);
    await second.reload(); await second.locator('#diagnostics-notice').waitFor({ timeout: 15000 });
    await second.locator('#diag-off').click();
    await second.waitForFunction(() => !document.getElementById('diagnostics-notice'));
    const off = await config(second); assert.equal(off.enabled, false); assert.equal(off.noticePending, false);
    b.proc.kill('SIGTERM'); await new Promise(r => b.proc.once('exit', r));
    const again = await start(collectorPort, path.join(tmp, fs.readdirSync(tmp).find(n => n.startsWith('b-'))));
    procs.push(again.proc);
    const back = await (await browser.newContext()).newPage();
    await back.goto(again.url); await back.locator('#email').fill('owner@example.test'); await back.locator('#password').fill(PASSWORD); await back.locator('form button').click();
    await back.locator('#account-controls .profile-trigger').waitFor();
    const afterRestart = await config(back);
    assert.equal(afterRestart.enabled, false, 'the server-wide choice survives a restart'); assert.equal(afterRestart.noticePending, false);

    // 3. Only the owner is asked: another account never sees the notice, even while it is still pending.
    const c = await start(collectorPort, fs.mkdtempSync(path.join(tmp, 'c-'))); procs.push(c.proc);
    const third = await ownerPage(c.url);
    await third.evaluate(async () => { await post('/api/admin/accounts', { email: 'viewer@example.test', password: 'viewer password 12345', confirm_password: 'viewer password 12345', role: 'user' }); });
    const other = await (await browser.newContext({ viewport: { width: 1280, height: 800 } })).newPage();
    other.on('pageerror', e => errors.push(e.message));
    await other.goto(c.url);
    await other.locator('#email').fill('viewer@example.test'); await other.locator('#password').fill('viewer password 12345'); await other.locator('form button').click();
    await other.locator('#account-controls .profile-trigger').waitFor(); await other.waitForTimeout(3000);
    assert.equal(await other.locator('#diagnostics-notice').count(), 0, 'a non-owner is never asked');
    assert.equal((await config(other)).noticePending, true, 'it stays pending for the owner');

    assert.deepEqual(errors, []);
    console.log('PASS: no top banner; the owner is asked once, the answer covers the whole server and survives restarts, other accounts are never asked, Settings keeps the controls');
  } finally { await browser?.close(); for (const p of procs) p.kill(); collector.close(); fs.rmSync(tmp, { recursive: true, force: true }); }
})().catch(e => { console.error(e); process.exit(1); });
