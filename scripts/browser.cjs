const assert=require('node:assert/strict');
const version=require('node:fs').readFileSync('Cargo.toml','utf8').match(/^version = "([^"]+)"/m)[1];
// Without this, Playwright's setOffline() does not affect service-worker fetches in current Chrome.
process.env.PW_EXPERIMENTAL_SERVICE_WORKER_NETWORK_EVENTS??='1';
const playwright=require('playwright');
(async()=>{
 const browser=await playwright.chromium.launch({channel:'chrome',headless:true});
 try {
  const context=await browser.newContext({viewport:{width:1440,height:950}});
  const page=await context.newPage(),errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(process.argv[2]);await page.locator('#email').fill('owner@example.test');await page.locator('#password').fill('integration password 123');await page.locator('form button').click();
  await page.locator('#account-controls .profile-trigger').waitFor();
  // API regressions may leave these fixtures watched; start this UI scenario unwatched.
  await page.evaluate(async()=>{await post('/api/playback/watched',{paths:files.map(f=>f.path),action:'reset'});await loadWatchedStates()});
  await page.keyboard.press('Control+k');assert.equal(await page.locator('#library-search').evaluate(el=>el===document.activeElement),true);
  const selectLibrary=async name=>{let menu=page.locator('#nav').getByRole('button',{name:/^Libraries/});if(await menu.getAttribute('aria-expanded')!=='true')await menu.click();await page.locator('#nav').getByRole('button',{name}).click()};
  await selectLibrary(/^Movies/);
  await page.locator('.browse-controls').waitFor();await page.locator('.card').first().waitFor();
  assert.equal(await page.locator('.card').count(),3);
  // Browsing an empty library must not shrink global search or Overview's snapshot.
  await selectLibrary(/^TV Shows/);
  await page.locator('#app .empty').filter({hasText:'No titles match'}).waitFor();
  await page.locator('#library-search').fill('Fixture 1');
  await page.getByRole('heading',{name:'Fixture 1',exact:true}).first().waitFor();
  await page.locator('#library-search').fill('');
  await page.locator('#nav').getByRole('button',{name:'Overview',exact:true}).click();
  assert.equal(await page.evaluate(()=>files.length),3);
  await selectLibrary(/^Movies/);
  await page.locator('.browse-controls').waitFor();await page.locator('.card').first().waitFor();
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
  // Without a TMDB key the movie search explains how to add one instead of failing silently.
  await page.locator('.sg-help').waitFor();assert((await page.locator('.sg-help').innerText()).toLowerCase().includes('tmdb key needed'));await page.locator('#sg-help-close').click();await page.locator('.sg-help').waitFor({state:'detached'});
  await page.getByRole('button',{name:'Close',exact:true}).click();await page.locator('.card').filter({has:page.getByRole('heading',{name:'Fixture 0',exact:true})}).click();
  assert(await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).isHidden());
  await page.getByRole('button',{name:'✓ Mark movie watched',exact:true}).click();await page.locator('#watched-status').filter({hasText:'Marked as watched.'}).waitFor();assert(await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).isVisible());
  await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).click();await page.locator('#watched-status').filter({hasText:'progress cleared'}).waitFor();assert(await page.getByRole('button',{name:'Mark unwatched / reset',exact:true}).isHidden());
  await page.getByRole('button',{name:'▶ Play movie',exact:true}).click();
  await page.locator('#player-audio option').nth(1).waitFor({state:'attached'});
  assert.equal(await page.locator('#player-chapter,#player-skip,.player-marker-editor').count(),0);
  assert.equal(await page.getByRole('button',{name:'Report playback issue',exact:true}).count(),0);
  await page.waitForFunction(()=>document.getElementById('player-video')?.currentTime>.3,null,{timeout:40000});
  await page.locator('#player-settings-toggle').click();
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
  await selectLibrary(/^Movies/);await page.locator('.browse-controls').waitFor();
  for(const [name,count] of [['Fixture 1',1],['Fixture 2',0]]){
   await page.locator('.card').filter({has:page.getByRole('heading',{name,exact:true})}).click();await page.getByRole('button',{name:'▶ Play movie',exact:true}).click();
   await page.waitForFunction(()=>typeof playerState!=='undefined'&&!!playerState?.mediaInfo);
   assert.equal(await page.locator('#player-audio option').count(),count);assert(await page.locator('#player-audio').isHidden());
   assert.equal(await page.locator('#player-chapter,#player-skip,.player-marker-editor').count(),0);
   await page.getByRole('button',{name:'Back to library',exact:true}).click();
  }
  await page.locator('#nav').getByRole('button',{name:'Settings',exact:true}).click();
  const remotePanel=page.locator('.remote-access-panel');await remotePanel.waitFor();
  assert(await remotePanel.getByText('Disabled',{exact:true}).isVisible());
  const remoteReady=await page.evaluate(async()=> (await api('/api/owner/remote-access')).configured);
  assert.equal(await remotePanel.getByRole('button',{name:'Enable remote access',exact:true}).isDisabled(),!remoteReady);
  await remotePanel.getByRole('button',{name:'Disable remote access',exact:true}).click();
  await page.locator('#remote-access-status').filter({hasText:'Remote access disabled.'}).waitFor();
  assert.equal(await remotePanel.getByRole('button',{name:'Enable remote access',exact:true}).isDisabled(),!remoteReady);
  const updatePanel=page.locator('#update-body');await updatePanel.getByText('Installed:').waitFor();assert((await updatePanel.innerText()).includes('Platno '+version));assert(await updatePanel.getByRole('button',{name:'Check for updates',exact:true}).isVisible());
  await remotePanel.getByRole('button',{name:'Refresh status',exact:true}).click();
  await page.locator('#remote-access-status').filter({hasText:'Status refreshed.'}).waitFor();
  assert.equal(await remotePanel.locator('a').getAttribute('href'),'/help#remote-access');
  for(const width of [320,390,768,1440]){await page.setViewportSize({width,height:950});await page.waitForFunction(()=>document.documentElement.scrollWidth<=innerWidth+1,null,{timeout:3000}).catch(()=>{});assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),`remote settings overflow at ${width}`)}
  await page.setViewportSize({width:390,height:950});await remotePanel.screenshot({path:'/tmp/plexv2-remote-settings-mobile.png'});
  await page.setViewportSize({width:1440,height:950});
  await page.getByLabel('Accent color',{exact:true}).selectOption('mint');await page.waitForFunction(()=>document.body.dataset.accent==='mint');
  await page.getByLabel('Reduce motion',{exact:true}).check();await page.waitForFunction(()=>document.body.classList.contains('reduce-motion'));

  await page.getByRole('button',{name:'Check server health',exact:true}).click();await page.locator('#diagnostic-output').filter({hasText:'FFmpeg: available'}).waitFor();
  await page.getByRole('button',{name:'Show sessions',exact:true}).click();await page.locator('#session-output').filter({hasText:'This device'}).waitFor();
  await selectLibrary(/^Movies/);await page.locator('.browse-controls').waitFor();
  await page.setViewportSize({width:390,height:844});
  await page.reload();await page.locator('#account-controls .profile-trigger').waitFor();await page.locator('.browse-controls').waitFor();
  // Phone navigation survives re-rendering the clicked button and closes after selection.
  const phoneLibraries=page.locator('#nav').getByRole('button',{name:/^Libraries/});
  await phoneLibraries.click();await page.locator('#library-submenu').waitFor({state:'visible'});
  await page.keyboard.press('Escape');assert.equal(await phoneLibraries.getAttribute('aria-expanded'),'false');
  await selectLibrary(/^Movies/);await page.locator('.browse-controls').waitFor();
  assert.equal(await phoneLibraries.getAttribute('aria-expanded'),'false');
  for(const width of [320,390,768,1024,1920]){
   await page.setViewportSize({width,height:844});
   await page.waitForFunction(()=>document.documentElement.scrollWidth<=innerWidth+1,null,{timeout:3000}).catch(()=>{});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,`layout must not overflow at ${width}px`);
  }
  await page.setViewportSize({width:390,height:844});
  await page.waitForFunction(()=>document.documentElement.scrollWidth<=innerWidth+1,null,{timeout:3000}).catch(()=>{});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),true,'mobile page must not overflow');
  await page.locator('.card').first().focus();await page.keyboard.press('Enter');assert(await page.locator('#dialog').evaluate(el=>el.open));
  await page.getByRole('button',{name:'Close',exact:true}).click();
  await page.screenshot({path:process.env.PLEXV2_SCREENSHOT||'/tmp/plexv2-mobile-regression.png',fullPage:true});
  await page.waitForFunction(()=>!!navigator.serviceWorker.controller);
  await context.setOffline(true);await page.goto(process.argv[2],{waitUntil:'domcontentloaded'});
  await page.getByRole('heading',{name:'Your media server is unavailable',exact:true}).waitFor();
  await context.setOffline(false);await page.goto(process.argv[2]);await page.locator('#account-controls .profile-trigger').waitFor();
  const help=await context.newPage();await help.goto(process.argv[2]+'/help');await help.getByRole('heading',{name:'PlexV2: user guide',exact:true}).waitFor();
  await help.setViewportSize({width:390,height:844});assert(await help.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));await help.close();
  await page.locator('#account-controls .profile-trigger').click();await page.getByRole('button',{name:'Sign out',exact:true}).click();await page.locator('#email').waitFor();
  const viewerContext=await browser.newContext();const viewer=await viewerContext.newPage();
  let forbiddenPolls=0;viewer.on('response',r=>{if(r.url().endsWith('/api/status')&&r.status()===403)forbiddenPolls++});
  await viewer.goto(process.argv[2]);await viewer.locator('#email').fill('viewer@example.test');await viewer.locator('#password').fill('viewer password 123');await viewer.locator('form button').click();
  await viewer.locator('#account-controls .profile-trigger').waitFor();await viewer.waitForTimeout(2200);
  assert.equal(forbiddenPolls,0);await viewerContext.close();
  assert.deepEqual(errors,[]);

  console.log('PASS: Chrome UI login, library filters, watched/reset, 0/1/2 audio visibility, removed controls, filters/search/sort, watchlist, HLS quality/audio, dual subtitles, profile appearance, sessions, mobile layout, offline/help/logout');
 } catch(error) {
  for(const context of browser.contexts())for(const page of context.pages()){
   console.error('Browser failure state:',await page.evaluate(()=>{let v=document.getElementById('player-video');return {status:document.getElementById('player-status')?.textContent,video:v?{time:v.currentTime,paused:v.paused,ended:v.ended,readyState:v.readyState,error:v.error?.message}:null}}).catch(()=>null));
  }
  throw error;
 } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
