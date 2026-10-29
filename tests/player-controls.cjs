const fs=require('node:fs');
const vm=require('node:vm');
const assert=require('node:assert/strict');
const {test}=require('node:test');
const html=fs.readFileSync('static/index.html','utf8');
function setup(names){
  let posts=[],starts=[],stored=new Map(),clock=20000;
  let video={currentTime:12,volume:1,muted:false,hidden:false,paused:false,ended:false,textTracks:[],playbackRate:1};
  const elements={'#player-video':video,'#player-mute':{textContent:''},'#player-status':{textContent:''}};
  const context={console,Blob,Number,JSON,Math,Map,WeakMap,Set,Date:class extends Date {static now(){return clock}},localStorage:{getItem:k=>stored.get(k)??null,setItem:(k,v)=>stored.set(k,v),removeItem:k=>stored.delete(k)},post:(url,data)=>{posts.push({url,data});return Promise.resolve({})},startPlayback:(...args)=>starts.push(args),showPlayerControls(){},setSubtitleTrack(){},setTimeout:fn=>{fn();return 1},clearTimeout(){},window:{},navigator:{},fetch:()=>Promise.resolve({}),$:id=>elements[id]};
  vm.createContext(context);vm.runInContext('let playerState=null,profilePrefs={},authUser={profile_id:7};',context);
  for(const name of names){let start=html.indexOf(`function ${name}(`);assert(start>=0,name);if(html.slice(start-6,start)==='async ')start-=6;let end=html.indexOf('\n',start);vm.runInContext(html.slice(start,end),context)}
  return {context,video,posts,starts,stored,elements,setState:state=>{context.nextState=state;vm.runInContext('playerState=nextState',context)},state:()=>vm.runInContext('playerState',context),advance:n=>clock+=n};
}
test('audio changes validate the chosen track and preserve position',()=>{
 const h=setup(['currentPosition','changeAudio']);h.setState({mode:'direct',audioTracks:[{id:0},{id:1}],duration:120});
 h.context.changeAudio('7');assert.equal(h.starts.length,0);
 h.context.changeAudio('1');assert.equal(h.state().audio,1);assert.equal(h.state().mode,'transcode');assert.equal(h.starts[0][0],12);
});
test('Auto quality preserves a selected alternate audio track',()=>{
 const h=setup(['currentPosition','changeQuality']);h.setState({mode:'transcode',offset:20,direct:true,audio:1});
 h.context.changeQuality('auto');assert.equal(h.state().mode,'transcode');assert.equal(h.starts[0][0],32);
 h.state().audio=0;h.context.changeQuality('auto');assert.equal(h.state().mode,'direct');
});
test('unsupported quality and speed do not restart or save playback',()=>{
 const h=setup(['currentPosition','changeQuality','changeSpeed']);h.setState({mode:'direct'});
 for(const value of ['4k','',null])h.context.changeQuality(value);
 for(const value of ['NaN','0','-1','3'])h.context.changeSpeed(value);
 assert.equal(h.posts.length,0);assert.equal(h.starts.length,0);
});
test('valid playback speed updates the video and profile preference',()=>{
 const h=setup(['changeSpeed']);h.setState({});h.context.changeSpeed('1.5');assert.equal(h.video.playbackRate,1.5);assert.equal(h.state().speed,1.5);assert.equal(h.posts[0].data.key,'playback_speed');
});
test('direct seek changes currentTime; converted seek restarts at an absolute position',()=>{
 const h=setup(['currentPosition','jumpVideo','seekPlayer']);h.setState({mode:'direct',duration:100});
 h.context.jumpVideo(-30);assert.equal(h.video.currentTime,0);h.context.jumpVideo(200);assert.equal(h.video.currentTime,100);
 h.context.seekPlayer('42');assert.equal(h.video.currentTime,42);
 h.setState({mode:'transcode',duration:100,offset:30});h.video.currentTime=5;h.context.jumpVideo(10);assert.equal(h.starts[0][0],45);h.context.seekPlayer('60');assert.equal(h.starts[1][0],60);
});
test('mute toggles the media element and visible control',()=>{
 const h=setup(['toggleMute']);h.context.toggleMute();assert(h.video.muted);assert.equal(h.elements['#player-mute'].textContent,'🔇');h.context.toggleMute();assert(!h.video.muted);
});
test('volume changes persist to local storage and the profile',()=>{
 const h=setup(['setVolume']);h.context.setVolume('0.35');assert.equal(h.video.volume,.35);assert.equal(h.stored.get('player-volume'),'0.35');assert.equal(h.posts[0].data.value,'0.35');h.context.setVolume('0');assert(h.video.muted);
});
test('play/pause toggles actual playback and ignores an unprepared hidden video',()=>{
 const h=setup(['togglePlay']);let plays=0,pauses=0;h.video.play=()=>{plays++;return Promise.resolve()};h.video.pause=()=>pauses++;
 h.video.paused=true;h.context.togglePlay();assert.equal(plays,1);h.video.paused=false;h.context.togglePlay();assert.equal(pauses,1);h.video.hidden=true;h.context.togglePlay();assert.equal(pauses,1);
});
test('subtitle delay rejects non-finite and out-of-range input',()=>{
 const h=setup(['changeSubtitleDelay']);let applied=0;h.context.applySubtitleDelay=()=>applied++;h.setState({subtitleDelay:0});
 for(const value of ['NaN','Infinity','31','-31'])h.context.changeSubtitleDelay(value);assert.equal(applied,0);assert.equal(h.state().subtitleDelay,0);h.context.changeSubtitleDelay('-1.25');assert.equal(h.state().subtitleDelay,-1.25);assert.equal(applied,1);
});
test('subtitle delay clamps cues at zero and never accumulates previous offsets',()=>{
 const h=setup(['applySubtitleDelay']);vm.runInContext('const subtitleCueTimes=new WeakMap()',h.context);let cue={startTime:1,endTime:4};h.video.textTracks=[{cues:[cue]}];h.setState({subtitleDelay:-2});h.context.applySubtitleDelay();assert.equal(cue.startTime,0);assert.equal(cue.endTime,2);h.state().subtitleDelay=3;h.context.applySubtitleDelay();assert.equal(cue.startTime,4);assert.equal(cue.endTime,7);
});
test('progress saves locally immediately and throttles network heartbeats',()=>{
 const h=setup(['savePlaybackProgress']);h.setState({path:'/fixture',mode:'direct',duration:100});h.context.savePlaybackProgress(12);assert.equal(h.posts.length,2);assert.equal(h.stored.get('playback:7:/fixture'),'12');h.context.savePlaybackProgress(13);assert.equal(h.posts.length,2);assert.equal(h.stored.get('playback:7:/fixture'),'13');h.advance(8001);h.context.savePlaybackProgress(20);assert.equal(h.posts.length,4);h.context.savePlaybackProgress(100,true);assert.equal(h.posts.length,6);assert(!h.stored.has('playback:7:/fixture'));
});
test('page-hide progress uses beacon and profile-specific storage',()=>{
 const h=setup(['currentPosition','flushPlaybackProgress']);let sent=[];h.context.navigator.sendBeacon=(url,body)=>(sent.push({url,body}),true);h.setState({path:'/fixture',mode:'transcode',offset:30,duration:100});h.context.flushPlaybackProgress();assert.equal(sent[0].url,'/api/playback/progress');assert.equal(sent[0].body.type,'application/json');assert.equal(h.stored.get('playback:7:/fixture'),'42');
});
test('page-hide progress falls back to keepalive when beacon cannot queue',()=>{
 const h=setup(['currentPosition','flushPlaybackProgress']);let sent=[];h.context.navigator.sendBeacon=()=>false;h.context.fetch=(url,options)=>(sent.push({url,options}),Promise.resolve({}));h.setState({path:'/fixture',mode:'direct',duration:100});h.context.flushPlaybackProgress();assert.equal(sent[0].options.keepalive,true);assert.equal(JSON.parse(sent[0].options.body).position,12);
});
test('time labels handle negative, invalid and hour-long durations',()=>{
 const h=setup(['playTime','formatRuntime']);assert.equal(h.context.playTime(-1),'0:00');assert.equal(h.context.playTime('bad'),'0:00');assert.equal(h.context.playTime(3661),'1:01:01');assert.equal(h.context.formatRuntime(90),'1h 30m');assert.equal(h.context.formatRuntime(null),'Runtime unavailable');
});
test('search normalization handles Slovenian accents and punctuation',()=>{
 const h=setup(['titleKey','pathParts']);assert.equal(h.context.titleKey('Čarobni ŠČ žarek!'),'carobni sc zarek');assert.deepEqual(Array.from(h.context.pathParts('D:\\Media\\Film.mp4')),['D:','Media','Film.mp4']);
});
test('removed player features have no controls, calls or implementations',()=>{
 for(const value of ['player-chapter','player-skip','player-marker-editor','reportPlaybackIssue','/api/playback/markers','/api/playback/report','Recent playback reports'])assert(!html.includes(value),value);
 assert(html.includes('<select id="player-audio" hidden '),'audio must start hidden until tracks are known');
});

test('Picture-in-Picture enters and exits supported browser mode',async()=>{
 const h=setup(['togglePictureInPicture']);let entered=0,exited=0;h.context.document={pictureInPictureElement:null,exitPictureInPicture:async()=>exited++};h.video.requestPictureInPicture=async()=>entered++;
 await h.context.togglePictureInPicture();assert.equal(entered,1);h.context.document.pictureInPictureElement=h.video;await h.context.togglePictureInPicture();assert.equal(exited,1);
});
test('Picture-in-Picture unsupported and rejected paths show an actionable status',async()=>{
 const h=setup(['togglePictureInPicture']);h.context.document={};await h.context.togglePictureInPicture();assert(h.elements['#player-status'].textContent.includes('unavailable'));
 h.video.requestPictureInPicture=async()=>{throw Error('denied')};await h.context.togglePictureInPicture();assert(h.elements['#player-status'].textContent.includes('Start playback first'));
});
test('fullscreen targets the dialog and exits when already active',async()=>{
 const h=setup(['toggleFullscreen']);let entered=0,exited=0;h.context.document={fullscreenElement:null,exitFullscreen:async()=>exited++};h.elements['#dialog']={requestFullscreen:async()=>entered++};await h.context.toggleFullscreen();assert.equal(entered,1);h.context.document.fullscreenElement={};await h.context.toggleFullscreen();assert.equal(exited,1);
});
test('fullscreen rejection leaves the player usable with a clear message',async()=>{
 const h=setup(['toggleFullscreen']);h.context.document={};h.elements['#dialog']={requestFullscreen:async()=>{throw Error('not allowed')}};await h.context.toggleFullscreen();assert(h.elements['#player-status'].textContent.includes('not available'));
});
test('completion timeupdate events stay throttled and seeking back resets completion',()=>{
 const h=setup(['savePlaybackProgress']);h.setState({path:'/fixture',mode:'direct',duration:100});
 h.context.savePlaybackProgress(93,true);assert.equal(h.posts.length,2);
 for(let i=0;i<1000;i++)h.context.savePlaybackProgress(94,true);
 assert.equal(h.posts.length,2,'completed playback must not flood SQLite');
 h.context.savePlaybackProgress(30,false);assert.equal(h.posts.length,4);assert.equal(h.posts[2].data.completed,false);
 h.advance(8001);h.context.savePlaybackProgress(31,false);assert.equal(h.posts.length,6);
});
test('slow scan polling never overlaps and ordinary accounts never request admin status',async()=>{
 const h=setup(['pollStatus']);let requests=0,finish;
 h.context.api=()=>{requests++;return new Promise(resolve=>finish=resolve)};
 h.context.updateScanNotice=()=>{};h.context.refreshFiles=()=>{};
 vm.runInContext('let statusPolling=false,job={},current="dashboard",collectionRefreshTick=0;authUser={role:"user"}',h.context);
 await h.context.pollStatus();assert.equal(requests,0);
 vm.runInContext('authUser={role:"owner"}',h.context);
 let first=h.context.pollStatus();await h.context.pollStatus();assert.equal(requests,1);
 finish({running:false});await first;assert.equal(vm.runInContext('statusPolling',h.context),false);
});
test('failed status polling releases the in-flight guard for recovery',async()=>{
 const h=setup(['pollStatus']);h.context.console={error(){}};h.context.api=()=>Promise.reject(Error('offline'));
 vm.runInContext('let statusPolling=false,job={};authUser={role:"owner"}',h.context);
 await h.context.pollStatus();assert.equal(vm.runInContext('statusPolling',h.context),false);
});
