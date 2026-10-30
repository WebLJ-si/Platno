// Fast Chrome checks against the actual shared CSS, including native picker interaction.
const assert=require('node:assert/strict'),fs=require('node:fs'),{chromium}=require('playwright');
(async()=>{
 const browser=await chromium.launch({channel:'chrome',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1100,height:900}});
  const html=fs.readFileSync('static/index.html','utf8'),base=html.match(/<style>([\s\S]*?)<\/style>/)[1],css=fs.readFileSync('static/platno.css','utf8');
  const select=id=>`<select id="${id}" aria-label="${id}"><option value="one">Original</option><option value="two">A long option label that should wrap inside the menu instead of overflowing a small screen</option><option disabled>Unavailable</option><option value="three">Third option</option></select>`;
  await page.setContent(`<style>${base}\n${css}</style><main><div id="app" tabindex="-1"><section class="panel settings-panel"><h2 tabindex="-1">Settings</h2><div class="settings-tabs"><a href="#form">Playback</a></div><label id="form">Quality${select('quality')}</label><label>Encoder${select('encoder')}</label><div class="browse-controls"><label>Genre${select('genre')}</label></div><div class="sg-library-fields"><label>Content${select('content')}</label></div><div class="detail-episode-picks"><label>Season${select('detail-season')}</label></div><div class="player-panel" style="position:static;max-height:none;max-width:100%;width:360px"><label class="player-setting">Subtitles${select('player-subtitle')}</label><div class="player-nav"><label>Season${select('player-season')}</label></div></div><button id="action" class="secondary">Save</button></section></div></main>`);
  const enhance=html.split('\n').find(line=>line.startsWith('function enhanceSelects('));
  await page.addScriptTag({content:enhance+';enhanceSelects(document)'});
  assert(await page.evaluate(()=>CSS.supports('appearance','base-select')));
  for(const id of ['quality','encoder','genre','content','detail-season','player-subtitle','player-season']){
   const control=page.locator('#'+id);
   assert.equal(await control.evaluate(el=>getComputedStyle(el).appearance),'base-select',id);
   await control.click();assert(await control.evaluate(el=>el.matches(':open')),id+' opens');
   assert.equal(await control.evaluate(el=>getComputedStyle(el,'::picker(select)').backgroundColor),'rgb(23, 23, 30)');
   await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');
   assert.equal(await control.inputValue(),'two',id+' keyboard selection');
   assert(await control.locator('selectedcontent').evaluate(el=>el.clientWidth>0&&el.clientWidth<el.closest('select').clientWidth-20&&getComputedStyle(el).textOverflow==='ellipsis'),id+' truncates long labels');
   assert(!(await control.evaluate(el=>el.matches(':open'))));
  }
  for(const width of [320,390,768,1100]){
   await page.setViewportSize({width,height:900});
   const control=page.locator('#quality');await control.click();
   const option=control.locator('option[value="two"]');
   assert(await option.evaluate(el=>{const r=el.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth+1&&r.width>0}),'picker fits '+width);
   await page.keyboard.press('Escape');
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'form fits '+width);
  }
  await page.setViewportSize({width:1100,height:900});await page.locator('#quality').click();
  await page.screenshot({path:'/tmp/platno-select-styles.png'});
  await page.keyboard.press('Escape');
  console.log('PASS: shared select styling, keyboard selection, disabled options and responsive pickers across settings, setup, filters, details and player');
 }finally{await browser.close()}
})().catch(e=>{console.error(e);process.exitCode=1});
