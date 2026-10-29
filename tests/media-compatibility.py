#!/usr/bin/env python3
"""Disposable codec conversion tests; --speech also runs the real local Whisper engine."""
import argparse, http.cookiejar, json, os, pathlib, shutil, socket, sqlite3, subprocess, tempfile, time, urllib.error, urllib.request, uuid
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run(*args):
    result=subprocess.run(args,capture_output=True)
    if result.returncode:print(result.stderr.decode(errors='replace'),flush=True)
    result.check_returncode();return result
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--speech',action='store_true');parser.add_argument('--browser',action='store_true');args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='plexv2-compat-') as directory:
        base=pathlib.Path(directory);data=base/'data';data.mkdir()
        fixtures=[]
        def make(name,options):
            path=base/name;run('ffmpeg','-v','error','-f','lavfi','-i','testsrc=size=320x180:rate=12:duration=2','-f','lavfi','-i','sine=frequency=440:duration=2',*options,str(path));fixtures.append(path)
        make('legacy.avi',['-c:v','mpeg4','-c:a','pcm_s16le'])
        make('legacy.flv',['-c:v','flv','-c:a','libmp3lame','-ar','44100'])
        make('browser.webm',['-c:v','libvpx-vp9','-deadline','realtime','-c:a','libopus'])
        make('high-depth.mp4',['-c:v','libx264','-pix_fmt','yuv420p10le','-c:a','aac'])
        silent=base/'odd-silent.mkv';run('ffmpeg','-v','error','-f','lavfi','-i','testsrc=size=321x181:rate=12:duration=2','-c:v','ffv1','-pix_fmt','yuv444p',str(silent));fixtures.append(silent)
        with socket.socket() as sock:sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
        model=os.environ.get('SMART_MEDIA_WHISPER_MODEL',str(pathlib.Path.home()/'Library/Application Support/smart-media-scanner/models/ggml-base.bin'))
        env={**os.environ,'SMART_MEDIA_DATA':str(data),'SMART_MEDIA_PORT':str(port),'SMART_MEDIA_HOST':'127.0.0.1','SMART_MEDIA_COOKIE_SECURE':'0','TMDB_TOKEN':'','OPENSUBTITLES_API_KEY':''}
        if args.speech:env['SMART_MEDIA_WHISPER_MODEL']=model
        log=open(base/'server.log','w');proc=subprocess.Popen([os.environ.get('PLEXV2_TEST_BINARY',str(ROOT/'target/release/smart-media-scanner'))],cwd=ROOT,env=env,stdout=log,stderr=log)
        url=f'http://127.0.0.1:{port}';client=urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        def request(path,body=None,status=200):
            req=urllib.request.Request(url+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'} if body is not None else {})
            try:
                with client.open(req,timeout=300) as response:code=response.status;raw=response.read()
            except urllib.error.HTTPError as error:code=error.code;raw=error.read()
            assert code==status,(path,code,status,raw[:400]);return raw
        def api(path,body=None,status=200):return json.loads(request(path,body,status))
        try:
            for _ in range(100):
                try:api('/api/auth/setup-status');break
                except OSError:time.sleep(.1)
            api('/api/auth/setup',{'email':'compat@example.test','password':'compatibility test password','confirm_password':'compatibility test password'})
            db=sqlite3.connect(data/'library.sqlite3')
            def add(path,subtitles=None):
                result={'kind':'movie','provider':'tmdb','provider_id':100+len(fixtures),'title':path.stem,'confidence':100,'subtitles':subtitles or []}
                db.execute('INSERT OR REPLACE INTO files(path,root,size,mtime,parsed,result,scanned) VALUES(?,?,?,?,?,?,?)',(str(path),str(base),path.stat().st_size,0,'{}',json.dumps(result),'2026-10-29'));db.commit()
            from urllib.parse import urlencode
            for path in fixtures:
                add(path);query=urlencode({'path':str(path)})
                info=api('/api/playback/info?'+query);assert not info['direct'],path.name
                session=api('/api/playback/hls/start',{'path':str(path),'quality':'480','compatible':True})
                playlist=request(session['url']).decode();segment=next(line for line in playlist.splitlines() if line.endswith('.ts'))
                raw=request(session['url'].rsplit('/',1)[0]+'/'+segment);stream=base/'segment.ts';stream.write_bytes(raw)
                probe=json.loads(run('ffprobe','-v','error','-show_streams','-of','json',str(stream)).stdout)
                video=next(s for s in probe['streams'] if s['codec_type']=='video');assert video['codec_name']=='h264' and video['pix_fmt']=='yuv420p';assert video['width']%2==video['height']%2==0
                if path!=silent:
                    audio=next(s for s in probe['streams'] if s['codec_type']=='audio');assert audio['codec_name']=='aac' and audio['channels']==2
                run('ffmpeg','-v','error','-i',str(stream),'-f','null','-');api('/api/playback/hls/stop',{'id':session['id']})
                converted=request('/api/playback/transcode?'+query+'&quality=480&compatible=true');mp4=base/'converted.mp4';mp4.write_bytes(converted);run('ffmpeg','-v','error','-i',str(mp4),'-f','null','-')
                print('PASS compatibility:',path.name,flush=True)
            # Validation is checked even when the optional speech runtime isn't configured.
            api('/api/playback/subtitle-sync',{'request_id':'test','path':'/not-in-library','subtitle':'file:0'},404)
            api('/api/playback/subtitle-sync/cancel',{'request_id':'not-an-owned-job'})
            if args.speech:
                cli=shutil.which('whisper-cli');assert cli and pathlib.Path(model).is_file(),'Install whisper-cli and a model before --speech'
                wav=base/'speech.wav';source=os.environ.get('PLEXV2_SPEECH_FIXTURE')
                if source:shutil.copyfile(source,wav)
                else:
                    assert shutil.which('say'),'Set PLEXV2_SPEECH_FIXTURE to a clear speech WAV on non-macOS hosts'
                    text='The northern train will leave the station in twenty minutes. Please remember to bring your blue jacket and the old map. Yesterday we discovered a small wooden house beyond the river. My brother found a silver key hidden under the kitchen table. We should take the mountain road before the storm arrives. After sunset the village becomes quiet and the stars appear.'
                    run('say','-v','Samantha','-r','150','-o',str(base/'speech.aiff'),text);run('ffmpeg','-v','error','-i',str(base/'speech.aiff'),'-ac','1','-ar','16000',str(wav))
                prefix=base/'reference';run(cli,'-m',model,'-f',str(wav),'-l','auto','-oj','-ml','64','-sow','-t','4','-of',str(prefix))
                spoken=json.loads(prefix.with_suffix('.json').read_text())['transcription'];assert len(spoken)>=3
                movie=base/'speech movie.mkv';run('ffmpeg','-v','error','-f','lavfi','-i','color=c=black:s=320x180:r=12','-i',str(wav),'-c:v','libx264','-preset','ultrafast','-c:a','aac','-shortest',str(movie))
                sub=base/'speech movie.en.srt'
                def stamp(ms):return f'{ms//3600000:02}:{ms//60000%60:02}:{ms//1000%60:02},{ms%1000:03}'
                def shifted(delay):return '\n\n'.join(f"{i+1}\n{stamp(s['offsets']['from']+delay)} --> {stamp(s['offsets']['to']+delay)}\n{s['text'].strip()}" for i,s in enumerate(spoken))+'\n'
                sub.write_text(shifted(4000));add(movie,[sub.name]);original=sub.read_bytes();body={'request_id':str(uuid.uuid4()),'path':str(movie),'subtitle':'file:0','audio':0,'position':0}
                result=api('/api/playback/subtitle-sync',body);a=result['alignment'];assert abs(a['offset']+4)<1.5,(a,spoken);assert a['anchors']>=3;assert sub.read_bytes()==original
                cached=api('/api/playback/subtitle-sync',body);assert cached['cached'] is True
                sub.write_text(shifted(7000));changed=api('/api/playback/subtitle-sync',body);assert changed['cached'] is False;assert abs(changed['alignment']['offset']+7)<1.5,changed
                api('/api/playback/subtitle-sync',{**body,'subtitle':'file:999'},400)
                print('PASS speech: real transcription, offset recovery, source preservation, cache reuse/invalidation and subtitle allowlist',flush=True)
                if args.browser:subprocess.run(['node',str(ROOT/'tests/subtitle-sync-browser.cjs'),url],check=True,timeout=180)
            else:print('SKIP real speech recognition (use --speech with local whisper-cli/model)',flush=True)
        finally:
            proc.terminate()
            try:proc.wait(timeout=10)
            except subprocess.TimeoutExpired:proc.kill();proc.wait()
            log.close()
if __name__=='__main__':main()
