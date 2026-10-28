const assert=require('node:assert/strict');
let playwright;
try {playwright=require('playwright')} catch {playwright=require('/Users/svitsmacbookpro/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright')}
(async()=>{
 const browser=await playwright.chromium.launch({channel:'chrome',headless:true});
 try {
  const context=await browser.newContext({viewport:{width:1440,height:950}});
  const page=await context.newPage(),errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(process.argv[2]);await page.locator('#email').fill('owner@example.test');await page.locator('#password').fill('integration password 123');await page.locator('form button').click();
  await page.locator('#account-controls .profile-trigger').waitFor();
  await page.keyboard.press('Control+k');assert.equal(await page.locator('#library-search').evaluate(el=>el===document.activeElement),true);
  await page.getByRole('button',{name:/^Libraries/}).click();
  await page.locator('#nav').getByRole('button',{name:/^Movies/}).click();
  await page.locator('.browse-controls').waitFor();await page.locator('.card').first().waitFor();
  await page.locator('.card').filter({has:page.getByRole('heading',{name:'Fixture 0',exact:true})}).click();
  await page.getByRole('button',{name:'✓ Mark movie watched',exact:true}).click();await page.locator('#watched-status').filter({hasText:'Marked as watched.'}).waitFor();
  await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).click();await page.locator('#watched-status').filter({hasText:'progress cleared'}).waitFor();
  await page.getByRole('button',{name:'▶ Play movie',exact:true}).click();
  await page.locator('#player-audio option').nth(1).waitFor({state:'attached'});
  assert.equal(await page.locator('#player-chapter option').count(),3);
  await page.waitForFunction(()=>document.getElementById('player-video')?.currentTime>.3,{timeout:40000});
  await page.locator('#player-audio').selectOption('1');
  await page.waitForFunction(()=>document.getElementById('player-video')?.currentTime>.3,{timeout:40000});
  await page.locator('#player-subtitle').selectOption({index:1});await page.locator('#subtitle-delay').fill('1.5');await page.locator('#subtitle-delay').dispatchEvent('change');
  await page.locator('#player-shell').focus();await page.keyboard.press('m');assert.equal(await page.locator('#player-video').evaluate(v=>v.muted),true);await page.keyboard.press('m');
  await page.getByRole('button',{name:'Back to library',exact:true}).click();
  await page.waitForTimeout(500);
  const progress=await page.evaluate(async()=>{let d=await(await fetch('/api/playback/watched')).json();return d.items});assert(progress.length>0);
  await page.setViewportSize({width:390,height:844});
  await page.reload();await page.locator('#account-controls .profile-trigger').waitFor();await page.locator('.browse-controls').waitFor();
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,'mobile page must not overflow');
  await page.locator('.card').first().focus();await page.keyboard.press('Enter');assert(await page.locator('#dialog').evaluate(el=>el.open));
  await page.getByRole('button',{name:'Close',exact:true}).click();
  assert.deepEqual(errors,[]);
  console.log('PASS: Chrome UI login, library filters, watched/reset, chapter/audio controls, HLS playback, subtitle delay, shortcuts, close progress, keyboard cards, mobile layout');
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
