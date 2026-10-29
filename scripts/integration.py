#!/usr/bin/env python3
"""Isolated HTTP/FFmpeg regression suite. Never touches the real library."""
import http.cookiejar, json, math, os, sys, pathlib, re, socket, sqlite3, struct, subprocess, tempfile, time, urllib.request, urllib.error
ROOT = pathlib.Path(__file__).resolve().parents[1]
def run(*args): return subprocess.run(args, check=True, capture_output=True)
with tempfile.TemporaryDirectory(prefix='plexv2-test-') as tmp:
    base=pathlib.Path(tmp); data=base/'data'; data.mkdir()
    sub=base/'test.srt'; sub.write_text('1\n00:00:01,000 --> 00:00:04,000\nSubtitle regression\n')
    sub2=base/'test.sl.srt';sub2.write_text('1\n00:00:01,000 --> 00:00:04,000\nSlovenski podnapis\n')
    meta=base/'chapters.txt'; meta.write_text(';FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=3000\ntitle=Intro\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=3000\nEND=12000\ntitle=Story\n')
    video=base/'fixture.mkv'
    run('ffmpeg','-v','error','-f','lavfi','-i','testsrc2=size=320x180:rate=24:duration=12','-f','lavfi','-i','sine=frequency=440:duration=12','-f','lavfi','-i','sine=frequency=880:duration=12','-i',str(sub),'-i',str(meta),'-i',str(sub2),'-map','0:v','-map','1:a','-map','2:a','-map','3:s','-map','5:s','-map_metadata','4','-map_chapters','4','-c:v','libx264','-preset','ultrafast','-c:a','aac','-c:s','srt','-metadata:s:a:0','language=eng','-metadata:s:a:1','language=slv','-metadata:s:s:0','language=eng','-metadata:s:s:1','language=slv',str(video))
    direct=base/'direct.mp4';run('ffmpeg','-v','error','-i',str(video),'-map','0:v','-map','0:a:0','-c','copy',str(direct))
    hdr=base/'hdr.mkv';run('ffmpeg','-v','error','-f','lavfi','-i','testsrc2=size=160x90:rate=24:duration=3','-vf','format=yuv420p10le,setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc','-c:v','libx264','-preset','ultrafast',str(hdr))
    with socket.socket() as sock: sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]
    url=f'http://127.0.0.1:{port}'
    log=open(base/'server.log','w')
    proc=subprocess.Popen([os.environ.get('PLEXV2_TEST_BINARY',str(ROOT/'target/debug/smart-media-scanner'))],cwd=ROOT,env={**os.environ,'SMART_MEDIA_DATA':str(data),'SMART_MEDIA_HOST':'127.0.0.1','SMART_MEDIA_PORT':str(port),'TMDB_TOKEN':'','OPENSUBTITLES_API_KEY':'','SMART_MEDIA_COOKIE_SECURE':'0','SMART_MEDIA_WIKIDATA':'off'},stdout=log,stderr=log)
    jar=http.cookiejar.CookieJar(); client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    def request(path, body=None, status=200, headers=None, opener=None):
        req=urllib.request.Request(url+path,data=None if body is None else json.dumps(body).encode(),headers={**({'Content-Type':'application/json'} if body is not None else {}),**(headers or {})})
        try:
            with (opener or client).open(req,timeout=140) as r: code=r.status; raw=r.read(); response_headers=r.headers
        except urllib.error.HTTPError as e: code=e.code;raw=e.read();response_headers=e.headers
        assert code==status,(path,code,status,raw[:400])
        return raw,response_headers
    def api(path, body=None, status=200, **kwargs):
        raw=request(path,body,status,**kwargs)[0]
        return json.loads(raw) if raw else None
    try:
        for i in range(100):
            try: api('/api/auth/setup-status');break
            except (OSError,urllib.error.URLError): time.sleep(.1)
        else: raise AssertionError('isolated server failed to start')
        api('/api/playback/info?path=nope',status=401,opener=urllib.request.build_opener())
        assert api('/manifest.webmanifest',opener=urllib.request.build_opener())['display']=='standalone'
        assert request('/assets/icon-192.png',opener=urllib.request.build_opener())[0].startswith(b'\x89PNG')
        assert b'Never cache authenticated' in request('/sw.js',opener=urllib.request.build_opener())[0]
        api('/api/auth/setup',{'email':'owner@example.test','password':'integration password 123','confirm_password':'integration password 123'})
        api('/api/owner/settings',{'key':'setup_guide','value':'done'})  # the first-run guide has its own browser test
        db=sqlite3.connect(data/'library.sqlite3')
        for n,path in enumerate([video,direct,hdr]):
            result={'kind':'movie','provider':'tmdb','provider_id':n+1,'title':f'Fixture {n}','confidence':100,'requires_review':False,'subtitles':[],'year':str(2020+n),'rating':[8,6,7][n],'runtime':[85,150,25][n],'genres':[['Comedy'],['Drama'],['Comedy']][n],'language':['en','sl','en'][n]}
            db.execute("INSERT INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?,?,?,?,?,?,?)",(str(path),str(base),path.stat().st_size,0,'{}',json.dumps(result),'2026-10-29'))
            db.execute('INSERT INTO library_files(path,library_id) VALUES(?,(SELECT id FROM libraries WHERE kind=\'movie\' LIMIT 1))',(str(path),))
        db.commit()
        from urllib.parse import urlencode
        q=urlencode({'path':str(video)})
        info=api('/api/playback/info?'+q)
        assert len(info['audio_tracks'])==2 and info['audio_tracks'][1]['language']=='slv',info
        assert 'chapters' not in info
        assert info['direct'] is False and api('/api/playback/info?'+urlencode({'path':str(direct)}))['direct'] is True
        raw,h=request('/api/playback/file?'+urlencode({'path':str(direct)}),headers={'Range':'bytes=0-15'},status=206)
        assert len(raw)==16 and h['Content-Range'].startswith('bytes 0-15/')
        request('/api/playback/file?'+urlencode({'path':str(direct)}),headers={'Range':'bytes=999999999-'},status=416)
        preview=request('/api/playback/thumbnail?'+q+'&time=1')[0]
        assert preview.startswith(b'\xff\xd8') and request('/api/playback/thumbnail?'+q+'&time=2')[0]==preview
        request('/api/playback/thumbnail?'+q+'&time=-1',status=400)
        api('/api/playback/hls/start',{'path':str(video),'quality':'nope'},status=400)
        api('/api/playback/hls/start',{'path':'/etc/passwd'},status=404)
        api('/api/playback/hls/start',{'path':str(video),'audio':7},status=400)
        hls=api('/api/playback/hls/start',{'path':str(video),'quality':'auto','audio':1})
        master=request(hls['url'])[0].decode(); variants=[line for line in master.splitlines() if line and not line.startswith('#')]
        assert len(variants)==3,master
        for variant in variants:
            playlist=request(hls['url'].rsplit('/',1)[0]+'/'+variant)[0].decode()
            segment=next(x for x in playlist.splitlines() if x and not x.startswith('#'))
            content=request(hls['url'].rsplit('/',1)[0]+'/'+segment)[0]
            ts=base/(variant+'.ts');ts.write_bytes(content)
            run('ffmpeg','-v','error','-i',str(ts),'-f','null','-')
            pcm=run('ffmpeg','-v','error','-i',str(ts),'-map','0:a:0','-ac','1','-ar','8000','-f','f32le','-').stdout
            samples=struct.unpack('<'+'f'*(len(pcm)//4),pcm)
            crossings=sum(a<=0<b for a,b in zip(samples,samples[1:]));frequency=crossings/(len(samples)/8000)
            assert 800<frequency<950,('selected second audio track frequency',frequency)
        api('/api/playback/hls/stop',{'id':hls['id']})
        request(hls['url'],status=404)
        assert api('/api/playback/info?'+urlencode({'path':str(hdr)}))['hdr'] is True
        hdr_session=api('/api/playback/hls/start',{'path':str(hdr),'quality':'480'})
        hdr_playlist=request(hdr_session['url'])[0].decode()
        hdr_segment=next(x for x in hdr_playlist.splitlines() if x and not x.startswith('#'))
        hdr_ts=base/'hdr-output.ts';hdr_ts.write_bytes(request(hdr_session['url'].rsplit('/',1)[0]+'/'+hdr_segment)[0])
        hdr_probe=json.loads(run('ffprobe','-v','error','-show_streams','-of','json',str(hdr_ts)).stdout)
        assert hdr_probe['streams'][0]['color_transfer']=='bt709',hdr_probe
        run('ffmpeg','-v','error','-i',str(hdr_ts),'-f','null','-')
        api('/api/playback/hls/stop',{'id':hdr_session['id']})
        for removed in ['/api/playback/markers','/api/admin/playback/markers','/api/playback/report','/api/admin/playback/reports']:
            request(removed,status=404)
            request(removed,{},status=404)
        api('/api/playback/watched',{'paths':[str(video)],'action':'watched'})
        assert api('/api/playback/progress?'+q)['completed']
        api('/api/playback/watched',{'paths':[str(video),'/not/in/library'],'action':'reset'},status=404)
        assert api('/api/playback/progress?'+q)['completed'],'invalid batch must be atomic'
        api('/api/playback/watched',{'paths':[str(video)],'action':'reset'})
        assert not api('/api/playback/progress?'+q)['completed']
        api('/api/playback/progress',{'path':str(video),'position':1,'duration':12})
        assert api('/api/playback/progress?'+q)['progress']['position']==1
        api('/api/playback/progress',{'path':str(video),'position':4,'duration':12})
        assert not api('/api/playback/progress?'+q)['completed'],'short videos must not complete at one third'
        api('/api/playback/progress',{'path':str(video),'position':11,'duration':12})
        assert api('/api/playback/progress?'+q)['completed']
        health=api('/api/admin/health');assert health['ffmpeg']['available'] and health['ffprobe']['available'] and health['files']==3
        api('/api/user/settings',{'key':'playback_volume','value':'NaN'},status=400)
        api('/api/user/settings',{'key':'playback_volume','value':'0.35'})
        api('/api/user/settings',{'key':'audio_language','value':'sl'})
        assert api('/api/user/settings')['audio_language']=='sl'
        owned_session=api('/api/playback/hls/start',{'path':str(video),'quality':'480'})
        slots=[api('/api/playback/hls/start',{'path':str(video),'quality':'480'}) for _ in range(3)]
        api('/api/playback/hls/start',{'path':str(video),'quality':'480'},status=429)
        request('/api/playback/transcode?'+q+'&quality=480',status=429)
        for session in slots: api('/api/playback/hls/stop',{'id':session['id']})
        for cycle in range(16):
            session=api('/api/playback/hls/start',{'path':str(video),'quality':'480'})
            directory=pathlib.Path(tempfile.gettempdir())/('smart-media-hls-'+session['id'])
            assert directory.is_dir()
            api('/api/playback/hls/stop',{'id':session['id']})
            assert not directory.exists(),('stop must wait for FFmpeg and remove cache',cycle)
        print('PASS: shared conversion limit and 16 immediate HLS start/stop/cleanup cycles',flush=True)
        second=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        api('/api/auth/login',{'email':'owner@example.test','password':'integration password 123'},opener=second)
        assert len(api('/api/auth/sessions')['items'])==2
        api('/api/auth/sessions',{'others':True})
        assert len(api('/api/auth/sessions')['items'])==1
        api('/api/auth/me',status=401,opener=second)
        api('/api/profiles',{'name':'Other','pin':'1234','confirm_pin':'1234'})
        other=next(x for x in api('/api/auth/me')['profiles'] if x['name']=='Other')
        api('/api/auth/profile',{'profile_id':other['id'],'pin':'1234'})
        request(owned_session['url'],status=404)
        api('/api/playback/hls/stop',{'id':owned_session['id']},status=404)
        assert api('/api/playback/watched')['items']==[]
        assert api('/api/user/settings')=={}
        api('/api/auth/profile',{'profile_id':1,'pin':'4321','confirm_pin':'4321'})
        api('/api/playback/hls/stop',{'id':owned_session['id']})
        api('/api/admin/accounts',{'email':'viewer@example.test','password':'viewer password 123','confirm_password':'viewer password 123','role':'user'})
        viewer=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        api('/api/auth/login',{'email':'viewer@example.test','password':'viewer password 123'},opener=viewer)
        api('/api/admin/health',status=403,opener=viewer)
        request('/assets/hls-1.7.3.js',opener=viewer)
        owner_session_id=api('/api/auth/sessions')['items'][0]['id']
        assert len(api('/api/auth/sessions',opener=viewer)['items'])==1
        assert api('/api/auth/sessions',{'id':owner_session_id},opener=viewer)['revoked']==0
        api('/api/auth/me')
        api('/api/metadata/detail?kind=movie&id=1',opener=viewer)
        sys.path.insert(0,str(ROOT/'tests'))
        from api_regressions import run_regressions
        cases=run_regressions(api,request,db,base,video,direct)
        print(f'PASS: {len(cases)} additional API feature groups',flush=True)
        if '--browser' in sys.argv: subprocess.run(['node',str(ROOT/'scripts/browser.cjs'),url],check=True,timeout=180)
        shutdown_client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        api('/api/auth/login',{'email':'owner@example.test','password':'integration password 123'},opener=shutdown_client)
        shutdown_session=api('/api/playback/hls/start',{'path':str(video),'quality':'480'},opener=shutdown_client)
        shutdown_dir=pathlib.Path(tempfile.gettempdir())/('smart-media-hls-'+shutdown_session['id'])
        proc.terminate();proc.wait(timeout=8)
        assert proc.returncode==0,('graceful SIGTERM',proc.returncode)
        assert not shutdown_dir.exists(),'graceful shutdown must remove active HLS cache'
        print('PASS: SIGTERM with active playback reaps FFmpeg, removes cache and exits cleanly',flush=True)
        print('PASS: isolated HTTP, audio tracks + decoded language, removed-feature checks, direct ranges, three HLS variants, validation, atomic watched/reset, profile isolation')
    finally:
        proc.terminate()
        try: proc.wait(timeout=10)
        except subprocess.TimeoutExpired: proc.kill();proc.wait()
        log.close()
