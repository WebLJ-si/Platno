const assert=require('node:assert/strict');
const playwright=require('playwright');
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
  assert.equal(await page.locator('.card').count(),3);
  await page.getByLabel('Year',{exact:true}).selectOption('2021');assert.equal(await page.locator('.card').count(),1);assert(await page.getByRole('heading',{name:'Fixture 1',exact:true}).isVisible());
  await page.getByLabel('Year',{exact:true}).selectOption('');await page.getByLabel('Genre',{exact:true}).selectOption('Comedy');assert.equal(await page.locator('.card').count(),2);
  await page.getByLabel('Genre',{exact:true}).selectOption('');await page.getByLabel('Time available',{exact:true}).selectOption('90');assert.equal(await page.locator('.card').count(),2);
  await page.getByLabel('Time available',{exact:true}).selectOption('');await page.getByLabel('Minimum rating',{exact:true}).selectOption('8');assert.equal(await page.locator('.card').count(),1);
  await page.getByLabel('Minimum rating',{exact:true}).selectOption('');await page.getByLabel('Sort',{exact:true}).selectOption('year');assert.equal(await page.locator('.card h3').first().textContent(),'Fixture 2');
  await page.getByLabel('Sort',{exact:true}).selectOption('title');
  await page.getByRole('button',{name:'Surprise me',exact:true}).click();assert(await page.locator('#dialog').evaluate(el=>el.open));await page.getByRole('button',{name:'Close',exact:true}).click();
  await page.locator('#library-search').fill('Fixture 1');await page.getByRole('heading',{name:'Fixture 1',exact:true}).first().waitFor();assert.equal(await page.getByRole('heading',{name:'Fixture 0',exact:true}).count(),0);
  await page.locator('#library-search').fill('');await page.locator('.browse-controls').waitFor();
  await page.locator('.card').filter({has:page.getByRole('heading',{name:'Fixture 0',exact:true})}).click();
  await page.locator('#detail-watchlist-button').click();await page.locator('#detail-watchlist-button').filter({hasText:'In Watchlist'}).waitFor();

  await page.locator('.detail-files summary').click();await page.getByRole('button',{name:'Edit identification',exact:true}).click();
  await page.getByRole('heading',{name:'Edit identification',exact:true}).waitFor();assert.equal(await page.locator('#edit-query').inputValue(),'Fixture 0');
  await page.getByRole('button',{name:'Close',exact:true}).click();await page.locator('.card').filter({has:page.getByRole('heading',{name:'Fixture 0',exact:true})}).click();
  await page.getByRole('button',{name:'✓ Mark movie watched',exact:true}).click();await page.locator('#watched-status').filter({hasText:'Marked as watched.'}).waitFor();
  await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).click();await page.locator('#watched-status').filter({hasText:'progress cleared'}).waitFor();
  await page.getByRole('button',{name:'▶ Play movie',exact:true}).click();
  await page.locator('#player-audio option').nth(1).waitFor({state:'attached'});
  assert.equal(await page.locator('#player-chapter,#player-skip,.player-marker-editor').count(),0);
  assert.equal(await page.getByRole('button',{name:'Report playback issue',exact:true}).count(),0);
  await page.waitForFunction(()=>document.getElementById('player-video')?.currentTime>.3,null,{timeout:40000});
  assert(await page.locator('#player-audio').isVisible());
  await page.locator('#player-audio').selectOption('1');
  await page.waitForFunction(()=>document.getElementById('player-video')?.currentTime>.3,null,{timeout:40000});
  await page.locator('#player-subtitle').selectOption({index:1});await page.locator('#subtitle-delay').fill('1.5');await page.locator('#subtitle-delay').dispatchEvent('change');
  await page.locator('#player-subtitle-secondary').selectOption({index:2});
  await page.waitForFunction(()=>Array.from(document.getElementById('player-video').textTracks).filter(t=>t.cues?.length>0).length===2);
  assert.equal(await page.locator('#player-chapter,#player-skip,.player-marker-editor').count(),0);
  await page.locator('#player-speed').selectOption('1.25');assert.equal(await page.locator('#player-video').evaluate(v=>v.playbackRate),1.25);
  await page.locator('#player-quality').selectOption('480');
  await page.waitForFunction(()=>document.getElementById('player-video').currentTime>.3,null,{timeout:40000});
  assert.equal(await page.locator('#player-audio').inputValue(),'1');

  await page.setViewportSize({width:390,height:844});await page.locator('#player-shell').focus();
  await page.screenshot({path:'/tmp/plexv2-player-mobile-regression.png'});
  assert(await page.locator('#player-shell').evaluate(el=>el.scrollWidth<=innerWidth+1&&el.clientHeight>=innerHeight*.95),'mobile player must not overflow');
  await page.setViewportSize({width:1440,height:950});
  await page.getByRole('button',{name:'Stream statistics',exact:true}).click();assert(await page.locator('#stream-stats').isVisible());
  await page.locator('#player-shell').focus();await page.keyboard.press('m');assert.equal(await page.locator('#player-video').evaluate(v=>v.muted),true);await page.keyboard.press('m');
  await page.getByRole('button',{name:'Back to library',exact:true}).click();
  await page.waitForTimeout(500);
  const progress=await page.evaluate(async()=>{let d=await(await fetch('/api/playback/watched')).json();return d.items});assert(progress.length>0);
  await page.locator('#nav').getByRole('button',{name:'Watchlist',exact:true}).click();await page.locator('.watchlist-card').waitFor();
  await page.getByRole('button',{name:'Remove',exact:true}).click();await page.getByText('Your watchlist is empty.',{exact:true}).waitFor();
  await page.locator('#nav').getByRole('button',{name:/^Movies/}).click();await page.locator('.browse-controls').waitFor();
  for(const [name,count] of [['Fixture 1',1],['Fixture 2',0]]){
   await page.locator('.card').filter({has:page.getByRole('heading',{name,exact:true})}).click();await page.getByRole('button',{name:'▶ Play movie',exact:true}).click();
   await page.waitForFunction(()=>typeof playerState!=='undefined'&&!!playerState?.mediaInfo);
   assert.equal(await page.locator('#player-audio option').count(),count);assert(await page.locator('#player-audio').isHidden());
   assert.equal(await page.locator('#player-chapter,#player-skip,.player-marker-editor').count(),0);
   await page.getByRole('button',{name:'Back to library',exact:true}).click();
  }
  await page.locator('#nav').getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByLabel('Accent color',{exact:true}).selectOption('mint');await page.waitForFunction(()=>document.body.dataset.accent==='mint');
  await page.getByLabel('Reduce motion',{exact:true}).check();await page.waitForFunction(()=>document.body.classList.contains('reduce-motion'));

  await page.getByRole('button',{name:'Check server health',exact:true}).click();await page.locator('#diagnostic-output').filter({hasText:'FFmpeg: available'}).waitFor();
  await page.getByRole('button',{name:'Show sessions',exact:true}).click();await page.locator('#session-output').filter({hasText:'This device'}).waitFor();
  await page.locator('#nav').getByRole('button',{name:/^Movies/}).click();await page.locator('.browse-controls').waitFor();
  await page.setViewportSize({width:390,height:844});
  await page.reload();await page.locator('#account-controls .profile-trigger').waitFor();await page.locator('.browse-controls').waitFor();
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,'mobile page must not overflow');
  await page.locator('.card').first().focus();await page.keyboard.press('Enter');assert(await page.locator('#dialog').evaluate(el=>el.open));
  await page.getByRole('button',{name:'Close',exact:true}).click();
  await page.screenshot({path:process.env.PLEXV2_SCREENSHOT||'/tmp/plexv2-mobile-regression.png',fullPage:true});
  await page.waitForFunction(()=>!!navigator.serviceWorker.controller);
  await context.setOffline(true);await page.goto(process.argv[2],{waitUntil:'domcontentloaded'});
  await page.getByRole('heading',{name:'Your media server is unavailable',exact:true}).waitFor();
  await context.setOffline(false);await page.goto(process.argv[2]);await page.locator('#account-controls .profile-trigger').waitFor();
  const help=await context.newPage();await help.goto(process.argv[2]+'/help');await help.getByRole('heading',{name:'PlexV2: navodila za uporabo',exact:true}).waitFor();
  await help.setViewportSize({width:390,height:844});assert(await help.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await help.close();
  await page.locator('#account-controls .profile-trigger').click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.locator('#email').waitFor();
  assert.deepEqual(errors,[]);

  console.log('PASS: Chrome UI login, library filters, watched/reset, 0/1/2 audio visibility, removed controls, filters/search/sort, watchlist, HLS quality/audio, dual subtitles, profile appearance, sessions, mobile layout, offline/help/logout');
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
