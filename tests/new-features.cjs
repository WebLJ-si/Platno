const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const html=fs.readFileSync('static/index.html','utf8');
function source(name){let start=html.indexOf(`function ${name}(`);assert(start>=0,name);if(html.slice(start-6,start)==='async ')start-=6;let end=html.indexOf('\n',start);return html.slice(start,end)}
const ctx={shuffleSeed:.25};vm.createContext(ctx);
for(let name of ['languageMatches','browseFilter','playerKey','nextPlayable'])vm.runInContext(source(name),ctx);
assert(ctx.languageMatches('slv','sl'));assert(ctx.languageMatches('eng','en'));assert(!ctx.languageMatches('eng','sl'));assert(!ctx.languageMatches('eng',''));
const items=[{title:'Alpha',year:'2020',rating:8,runtime:85,genres:['Comedy'],files:[{path:'a',scanned:'2026-01-01'}]},{title:'Beta',year:'2022',rating:6,runtime:150,genres:['Drama'],files:[{path:'b',scanned:'2026-10-01'}]},{title:'Unknown',files:[{path:'c'}]}];
const watched=new Map([['a',{completed:true}]]);
assert.equal(ctx.browseFilter(items,{minutes:'90'},watched)[0].item.title,'Alpha');
assert.equal(ctx.browseFilter(items,{genre:'Drama'},watched)[0].item.title,'Beta');
assert.equal(ctx.browseFilter(items,{year:'2020',unwatched:true},watched).length,0);
assert.equal(ctx.browseFilter(items,{rating:'7'},watched).length,1);
assert.equal(ctx.browseFilter(items,{sort:'added'},watched)[0].item.title,'Beta');
assert.equal(ctx.browseFilter(items,{sort:'year'},watched)[0].item.title,'Beta');
assert.equal(ctx.browseFilter(items,{sort:'rating'},watched)[0].item.title,'Alpha');
let calls=[];Object.assign(ctx,{togglePlay:()=>calls.push('play'),jumpVideo:n=>calls.push(n),toggleFullscreen:()=>calls.push('full'),toggleMute:()=>calls.push('mute'),nextEpisode:()=>calls.push('next')});
const press=(key,extra={})=>ctx.playerKey({key,target:{tagName:'DIV'},preventDefault(){calls.push('prevent')},...extra});
press('j');press('l');press('N',{shiftKey:true});assert.deepEqual(calls,[-10,'prevent',10,'prevent','next','prevent']);
calls=[];press(' ',{target:{tagName:'TEXTAREA'}});press('j',{ctrlKey:true});assert.equal(calls.length,0);
vm.runInContext('let playerState={path:"one"}',ctx);ctx.files=[{path:'one',result:{season:1,episodes:[1,2]}},{path:'duplicate',result:{season:1,episodes:[1]}},{path:'offline',available:false,result:{season:1,episodes:[3]}},{path:'next',result:{season:1,episodes:[4]}}];ctx.playableEpisodes=()=>ctx.files;
assert.equal(ctx.nextPlayable().path,'next');
console.log('New feature behavior checks passed');
for(let name of ['remainingLabel','nextCardEnabled','hidePostPlay','cancelPostPlay','updatePostPlay','showPostPlay','askHideNextCard','hideNextCardForever','applySubtitleDelay'])vm.runInContext(source(name),ctx);
vm.runInContext('const NEXT_CARD_SECONDS=90',ctx);
const classes=new Set(),bar={style:{}},label={innerHTML:''};
const card={hidden:true,innerHTML:'',classList:{add:(...n)=>n.forEach(x=>classes.add(x)),remove:(...n)=>n.forEach(x=>classes.delete(x)),toggle:(n,on)=>on?classes.add(n):classes.delete(n),contains:n=>classes.has(n)},querySelector:()=>bar};
const video={paused:false,ended:false},status={textContent:''};let posted=[];
Object.assign(ctx,{profilePrefs:{auto_play_next:'false'},esc:String,positionSubtitles:()=>{},requestAnimationFrame:fn=>fn(),nextPlayable:()=>({result:{title:'Show',season:1,episodes:[4],episode_title:'Four'}}),post:async(url,data)=>{posted.push([url,data])},$:id=>({'#post-play':card,'#player-video':video,'#next-label':label,'#player-status':status}[id])});
assert.equal(ctx.remainingLabel(75),'1:15');assert.equal(ctx.remainingLabel(9.2),'10 s');
const fresh=duration=>{vm.runInContext('playerState={path:"one",duration:'+duration+'}',ctx);card.hidden=true;classes.clear();return vm.runInContext('playerState',ctx)};
// 25-minute episode: nothing until 90 s remain, then a card that counts down, hides again if the viewer seeks back
let st=fresh(1500);ctx.updatePostPlay(st,1400);assert.equal(card.hidden,true,'no card with 100 s left');
ctx.updatePostPlay(st,1420);assert.equal(card.hidden,false);assert(classes.has('is-visible'));assert(label.innerHTML.includes('1:20'),label.innerHTML);assert(card.innerHTML.includes('Show · S01E04 · Four'));
ctx.updatePostPlay(st,1470);assert(label.innerHTML.includes('30 s'));assert.equal(bar.style.width,'33.33333333333333%');
ctx.updatePostPlay(st,600);assert.equal(card.hidden,true,'seeking back hides the card without cancelling');assert(!st.nextCancelled);
// Close cancels for this episode only
ctx.updatePostPlay(st,1450);assert.equal(card.hidden,false);ctx.cancelPostPlay();assert.equal(card.hidden,true);ctx.updatePostPlay(st,1460);assert.equal(card.hidden,true);
// short video uses 20% of its length
st=fresh(120);ctx.updatePostPlay(st,80);assert.equal(card.hidden,true);ctx.updatePostPlay(st,100);assert.equal(card.hidden,false);
st=fresh(30);ctx.updatePostPlay(st,29);assert.equal(card.hidden,true,'videos under 40 s never show it');
// after the end the card says so and the bar is empty
st=fresh(1500);ctx.showPostPlay(st,true);assert(label.innerHTML.includes('finished'));assert.equal(bar.style.width,'0%');
// the account preference switches it off
ctx.profilePrefs.show_next_card='false';st=fresh(1500);ctx.updatePostPlay(st,1450);assert.equal(card.hidden,true);ctx.profilePrefs.show_next_card='true';
// Don't show again asks first, then saves to the account
st=fresh(1500);ctx.updatePostPlay(st,1450);ctx.askHideNextCard(true);assert(classes.has('is-confirming'));ctx.askHideNextCard(false);assert(!classes.has('is-confirming'));
ctx.askHideNextCard(true);ctx.hideNextCardForever().then(()=>{assert.equal(card.hidden,true);assert.equal(JSON.stringify(posted),JSON.stringify([['/api/user/settings',{key:'show_next_card',value:'false'}]]));assert.equal(ctx.profilePrefs.show_next_card,'false');assert(status.textContent.includes('Profile preferences'));console.log('Next-episode card checks passed')});
vm.runInContext('const subtitleCueTimes=new WeakMap();playerState={subtitleDelay:1.5}',ctx);
const cue={startTime:1,endTime:3};video.textTracks=[{cues:[cue]}];ctx.applySubtitleDelay();assert.equal(cue.startTime,2.5);assert.equal(cue.endTime,4.5);
vm.runInContext('playerState.subtitleDelay=-.5',ctx);ctx.applySubtitleDelay();assert.equal(cue.startTime,.5);assert.equal(cue.endTime,2.5,'delay changes must not accumulate');
console.log('Post-play cancellation, pause behavior, manual mode, and subtitle timing checks passed');
