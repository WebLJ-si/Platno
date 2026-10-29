"""Named feature-level HTTP regressions, called by scripts/integration.py.

Only use with its disposable fixtures; the harness refuses normal external servers.
"""
import contextlib
import http.cookiejar
import json
import pathlib
import time
import urllib.request
from urllib.parse import urlencode


def run_regressions(api, request, db, base, video, direct):
    completed = []

    @contextlib.contextmanager
    def case(name):
        try:
            yield
        except Exception as error:
            raise AssertionError(f"API regression failed: {name}: {error}") from error
        completed.append(name)
        print(f"  PASS API: {name}", flush=True)

    def client():
        return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def login(email, password):
        opener = client()
        api('/api/auth/login', {'email': email, 'password': password}, opener=opener)
        return opener

    def account(email, role='user'):
        return {'email': email, 'password': 'regression password 123', 'confirm_password': 'regression password 123', 'role': role}

    q = urlencode({'path': str(video)})
    with case('public assets and setup state'):
        anonymous = client()
        assert api('/api/auth/setup-status', opener=anonymous)['configured']
        assert b'Sign in' in request('/', opener=anonymous)[0]
        for path in ['/assets/icon-192.png', '/assets/icon-512.png', '/assets/hls-1.7.3.js', '/manifest.webmanifest', '/sw.js', '/offline.html', '/help']:
            request(path, opener=anonymous)
        api('/api/auth/me', status=401, opener=anonymous)
    with case('protected APIs require a session'):
        for path in ['/api/files', '/api/libraries', '/api/watchlist', '/api/playback/progress', '/api/playback/watched', '/api/user/settings', '/api/auth/sessions', '/api/admin/accounts', '/api/admin/health', '/api/owner/settings']:
            request(path, status=401, opener=anonymous)
    with case('owner setup cannot be repeated'):
        api('/api/auth/setup', account('second-owner@example.test'), status=409, opener=anonymous)
    with case('malformed and incorrect login is rejected'):
        api('/api/auth/login', {'email': 'invalid', 'password': 'anything'}, status=401, opener=anonymous)
        api('/api/auth/login', {'email': 'absent@example.test', 'password': 'incorrect'}, status=401, opener=anonymous)
    with case('login brute-force lockout and expired-window recovery'):
        email='rate-limit@example.test'
        for _ in range(10):api('/api/auth/login',{'email':email,'password':'wrong'},status=401,opener=anonymous)
        api('/api/auth/login',{'email':email,'password':'wrong'},status=429,opener=anonymous)
        db.execute('UPDATE auth_login_attempts SET blocked_until=0,window_started=? WHERE username=?',(int(time.time())-901,email));db.commit()
        api('/api/auth/login',{'email':email,'password':'wrong'},status=401,opener=anonymous)
        assert db.execute('SELECT attempts FROM auth_login_attempts WHERE username=?',(email,)).fetchone()[0]==1
    with case('account creation validation and uniqueness'):
        for patch in [{'email': 'bad'}, {'password': 'short', 'confirm_password': 'short'}, {'confirm_password': 'different'}, {'role': 'owner'}]:
            api('/api/admin/accounts', {**account('bad@example.test'), **patch}, status=400)
        api('/api/admin/accounts', account('reg-user@example.test'))
        api('/api/admin/accounts', account('reg-user@example.test'), status=409)
        api('/api/admin/accounts', account('reg-admin@example.test', 'admin'))
        user = login('reg-user@example.test', 'regression password 123')
        admin = login('reg-admin@example.test', 'regression password 123')
    with case('user permissions: browse, watch and preferences; no administration'):
        for path in ['/api/files', '/api/libraries', '/api/watchlist', '/api/user/settings', '/api/auth/sessions', '/api/server/info', '/api/metadata/detail?kind=movie&id=1']:
            api(path, opener=user)
        for path in ['/api/admin/accounts', '/api/admin/dashboard', '/api/admin/health', '/api/admin/library-settings', '/api/owner/settings', '/api/owner/providers', '/api/owner/encoders']:
            api(path, status=403, opener=user)
        for path, body in [('/api/scan', {'roots': [str(base)]}), ('/api/override', {}), ('/api/metadata/match', {}), ('/api/owner/settings', {'key': 'friendly_name', 'value': 'No'})]:
            api(path, body, status=403, opener=user)
    with case('administrator can manage users but cannot elevate roles or change owner settings'):
        api('/api/admin/accounts', opener=admin)
        api('/api/admin/dashboard', opener=admin)
        api('/api/admin/accounts', account('admin-child@example.test'), opener=admin)
        api('/api/admin/accounts', account('elevated@example.test', 'admin'), status=400, opener=admin)
        api('/api/owner/settings', status=403, opener=admin)
        api('/api/owner/accounts/credentials', {}, status=403, opener=admin)
    with case('account password reset revokes prior logins'):
        accounts = api('/api/admin/accounts')['accounts']
        target = next(x for x in accounts if x['email'] == 'reg-user@example.test')
        change = {'account_id': target['id'], 'email': 'reg-user@example.test', 'password': 'replacement password 456', 'confirm_password': 'replacement password 456'}
        api('/api/owner/accounts/credentials', {**change, 'confirm_password': 'mismatch'}, status=400)
        api('/api/owner/accounts/credentials', change)
        api('/api/auth/me', status=401, opener=user)
        api('/api/auth/login', {'email': 'reg-user@example.test', 'password': 'regression password 123'}, status=401, opener=user)
        user = login('reg-user@example.test', 'replacement password 456')
    with case('owner credential update requires current password'):
        owner = api('/api/auth/me')
        change = {'account_id': owner['account_id'], 'email': owner['email'], 'password': 'integration password 123', 'confirm_password': 'integration password 123'}
        api('/api/owner/accounts/credentials', change, status=403)
        api('/api/owner/accounts/credentials', {**change, 'current_owner_password': 'incorrect'}, status=403)
    with case('profile name, PIN and confirmation validation'):
        for patch in [{'name': ''}, {'name': 'x'*49}, {'pin': '12', 'confirm_pin': '12'}, {'pin': 'abcd', 'confirm_pin': 'abcd'}, {'confirm_pin': '9999'}]:
            api('/api/profiles', {**{'name': 'Regression', 'pin': '2468', 'confirm_pin': '2468'}, **patch}, status=400, opener=user)
        api('/api/profiles', {'name': 'Regression', 'pin': '2468', 'confirm_pin': '2468'}, opener=user)
        api('/api/profiles', {'name': 'Regression', 'pin': '2468', 'confirm_pin': '2468'}, status=409, opener=user)
    with case('profile PIN lockout and cross-account isolation'):
        profile = next(x for x in api('/api/auth/me', opener=user)['profiles'] if x['name'] == 'Regression')
        for _ in range(5):
            api('/api/auth/profile', {'profile_id': profile['id'], 'pin': '0000'}, status=403, opener=user)
        api('/api/auth/profile', {'profile_id': profile['id'], 'pin': '2468'}, status=429, opener=user)
        db.execute('DELETE FROM profile_pin_attempts WHERE profile_id=?', (profile['id'],)); db.commit()
        api('/api/auth/profile', {'profile_id': profile['id'], 'pin': '2468'}, opener=user)
        api('/api/auth/profile', {'profile_id': profile['id'], 'pin': '2468'}, status=404)
    with case('watchlist add, idempotence, removal and profile isolation'):
        for _ in range(2):
            api('/api/watchlist/toggle', {'path': str(video), 'enabled': True}, opener=user)
        assert len(api('/api/watchlist', opener=user)['items']) == 1
        assert not any(x['path'] == str(video) for x in api('/api/watchlist')['items'])
        api('/api/watchlist/toggle', {'path': '/not-in-library', 'enabled': True}, status=404, opener=user)
        api('/api/watchlist/toggle', {'path': '', 'enabled': True}, status=400, opener=user)
        api('/api/watchlist/toggle', {'path': str(video), 'enabled': False}, opener=user)
        assert api('/api/watchlist', opener=user)['items'] == []
    with case('all profile setting types persist and reject unsupported values'):
        values = {'playback_quality':'720','playback_speed':'1.25','playback_volume':'0.4','audio_language':'sl','subtitle_language':'en','subtitle_mode':'always','subtitle_size':'120','subtitle_color':'yellow','subtitle_background':'transparent','subtitle_shadow':'false','auto_play_next':'false','show_continue':'false','show_recommendations':'false','show_popular':'false','reduce_motion':'true','continue_limit':'24','accent_color':'blue'}
        for key, value in values.items(): api('/api/user/settings', {'key':key,'value':value}, opener=user)
        assert api('/api/user/settings', opener=user) == values
        for key in values:
            api('/api/user/settings', {'key':key,'value':'unsupported-value'}, status=400, opener=user)
        api('/api/user/settings', {'key':'unknown','value':'anything'}, status=400, opener=user)
        for value in ['NaN','inf','-1','1.01']:
            api('/api/user/settings', {'key':'playback_volume','value':value}, status=400, opener=user)
        assert api('/api/user/settings', opener=user) == values
    with case('watch progress validation, completion, reset and profile isolation'):
        for patch in [{'position': -1}, {'duration': -1}, {'position':86401}, {'path':'/missing'}]:
            api('/api/playback/progress', {**{'path':str(video),'position':1,'duration':12}, **patch}, status=400, opener=user)
        api('/api/playback/progress', {'path':str(video),'position':2,'duration':12}, opener=user)
        assert api('/api/playback/progress?'+q, opener=user)['progress']['position']==2
        api('/api/playback/watched', {'paths':[str(video)],'action':'watched'}, opener=user)
        assert api('/api/playback/progress?'+q, opener=user)['completed']
        api('/api/playback/watched', {'paths':[str(video)],'action':'reset'}, opener=user)
        assert api('/api/playback/progress?'+q, opener=user)['progress'] is None
        for body in [{'paths':[],'action':'watched'},{'paths':[str(video)],'action':'unknown'}]: api('/api/playback/watched',body,status=400,opener=user)
        api('/api/playback/watched',{'paths':[str(video),'/missing'],'action':'watched'},status=404,opener=user)
        assert api('/api/playback/watched',opener=user)['items']==[]
    with case('activity heartbeat and administrator dashboard'):
        payload={'path':str(video),'position':3,'duration':12,'mode':'direct','active':True}
        api('/api/playback/activity',payload,opener=user)
        assert any(x['path']==str(video) and x['profile']=='Regression' for x in api('/api/admin/dashboard')['active'])
        api('/api/playback/activity',{**payload,'active':False},opener=user)
        assert not any(x['profile']=='Regression' for x in api('/api/admin/dashboard')['active'])
        api('/api/playback/activity',{**payload,'path':'/missing'},status=404,opener=user)
    with case('server settings validation and owner encoder discovery'):
        for key, value in [('friendly_name','Regression server'),('transcode_quality','speed'),('hardware_encoder','auto')]: api('/api/owner/settings',{'key':key,'value':value})
        assert api('/api/server/info')['friendly_name']=='Regression server'
        assert api('/api/owner/settings')['transcode_quality']=='speed'
        for key, value in [('friendly_name',''),('friendly_name','x'*49),('transcode_quality','wrong'),('hardware_encoder','not-an-encoder'),('unknown','x')]: api('/api/owner/settings',{'key':key,'value':value},status=400)
        assert 'libx264' in api('/api/owner/encoders')['encoders']
        api('/api/owner/settings',{'key':'friendly_name','value':'Smart Media'})
        api('/api/owner/settings',{'key':'transcode_quality','value':'balanced'})
    with case('library creation, editing, validation, uniqueness and safe deletion'):
        api('/api/admin/libraries',{'name':'Regression library','kind':'video','roots':[]})
        lib=next(x for x in api('/api/libraries')['libraries'] if x['name']=='Regression library')
        for patch in [{'name':''},{'name':'x'*49},{'kind':'music'},{'roots':['']},{'roots':['x\n']}]: api('/api/admin/libraries',{**lib,**patch},status=400)
        api('/api/admin/libraries',{'name':'Regression library','kind':'video'},status=409)
        api('/api/admin/libraries',{**lib,'name':'Renamed regression library','roots':[str(base/'empty')]})
        assert api(f"/api/libraries/{lib['id']}/files")['files']==[]
        api('/api/libraries/999999/files',status=404)
        # Moving only the membership in the fixture proves deletion keeps source media.
        db.execute('UPDATE library_files SET library_id=? WHERE path=?',(lib['id'],str(direct)));db.commit()
        assert len(api(f"/api/libraries/{lib['id']}/files")['files'])==1
        api('/api/admin/libraries/delete',{'id':lib['id']})
        assert direct.is_file()
        assert db.execute('SELECT library_id FROM library_files WHERE path=?',(str(direct),)).fetchone()[0]!=lib['id']
        # Put it back in Movies for UI tests.
        db.execute("UPDATE library_files SET library_id=(SELECT id FROM libraries WHERE kind='movie' LIMIT 1) WHERE path=?",(str(direct),));db.commit()
        api('/api/admin/libraries/delete',{'id':lib['id']},status=404)
        only_movie=next(x for x in api('/api/libraries')['libraries'] if x['kind']=='movie')
        api('/api/admin/libraries/delete',{'id':only_movie['id']},status=409)
    with case('scan settings and empty incremental/full scan lifecycle'):
        root=base/'empty';root.mkdir(exist_ok=True)
        api('/api/admin/library-settings',{'auto_sort':False,'scan_roots':[str(root)]})
        assert api('/api/admin/library-settings')['auto_sort'] is False
        api('/api/scan',{'roots':[]},status=400)
        api('/api/scan',{'roots':[str(root)]},status=400)
        api('/api/scan',{'roots':[str(root)],'library_id':999999},status=400)
        target=next(x['id'] for x in api('/api/libraries')['libraries'] if x['kind']=='video')
        for full in [False, True]:
            api('/api/scan',{'roots':[str(root)],'library_id':target,'full_scan':full})
            for _ in range(100):
                if not api('/api/status')['running']: break
                time.sleep(.03)
            else: raise AssertionError('empty scan did not finish')
            assert api('/api/status')['processed']==0
        api('/api/cancel',{})
        api('/api/admin/library-settings',{'auto_sort':True,'scan_roots':[]})
        api('/api/admin/library-settings',{'auto_sort':True,'scan_roots':['']},status=400)
    with case('scanning a real tree: unicode, nesting, ignored files, symlink loops, unreadable folders, locks, rescans and removed media'):
        tree = base / 'scan-tree'
        for relative in ['Movies/Blade.Runner.2049.2017.1080p.BluRay.mkv', 'Movies/Zażółć gęślą jaźń (2020).mp4',
                         'Movies/Deep/A/B/C/D/Nested.Film.2015.avi', 'Movies/empty.2012.mkv', 'Show/Extras/Interview.mkv',
                         'Movies/notes.txt', 'Movies/.DS_Store', 'Movies/poster.jpg']:
            target = tree / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b'' if 'empty' in relative else b'not a real video')
        (tree / 'Movies/loop').symlink_to('..', target_is_directory=True)
        denied = tree / 'Movies/denied'
        denied.mkdir()
        (denied / 'Hidden.Film.2001.mkv').write_bytes(b'x')
        denied.chmod(0)

        def scan(full=False):
            api('/api/scan', {'roots': [str(tree)], 'full_scan': full})
            for _ in range(300):
                status = api('/api/status')
                if not status['running']:
                    return status
                time.sleep(.05)
            raise AssertionError('scan did not finish')

        def listed():
            return {pathlib.Path(f['path']).name: f for f in api('/api/files') if f['path'].startswith(str(tree.resolve()))}
        try:
            api('/api/admin/library-settings', {'auto_sort': True, 'scan_roots': [str(tree)]})
            status = scan()
            assert status['phase'] == 'Complete' and status['processed'] == status['total'] == 5, status
            assert any('denied' in e for e in status['errors']), status['errors']
            files = listed()
            assert set(files) == {'Blade.Runner.2049.2017.1080p.BluRay.mkv', 'Zażółć gęślą jaźń (2020).mp4', 'Nested.Film.2015.avi', 'empty.2012.mkv', 'Interview.mkv'}, set(files)
            blade = files['Blade.Runner.2049.2017.1080p.BluRay.mkv']
            assert (blade['parsed']['title'], blade['parsed']['year'], blade['parsed']['kind']) == ('Blade Runner 2049', 2017, 'movie'), blade['parsed']
            assert files['Interview.mkv']['result']['kind'] == 'extra'
            assert 'Hidden.Film.2001.mkv' not in files and 'loop' not in files
            movie_library = next(x['id'] for x in api('/api/libraries')['libraries'] if x['kind'] == 'movie')
            in_movies = {pathlib.Path(f['path']).name for f in api(f'/api/libraries/{movie_library}/files')['files']}
            assert 'Zażółć gęślą jaźń (2020).mp4' in in_movies and 'Interview.mkv' not in in_movies

            skipped = lambda st: sum(1 for e in st['events'] if e['message'].startswith('Skipped unchanged or locked'))
            assert skipped(scan()) == 5

            path = blade['path']
            for bad in [{'kind': 'podcast'}, {'kind': 'movie', 'title': 'bad\u0000title'}, {'kind': 'movie', 'provider_id': 0}, {'kind': 'tv', 'season': 5000}]:
                api('/api/override', {'path': path, 'override': bad}, status=400)
            api('/api/override', {'path': path, 'override': {'kind': 'movie', 'title': '  Chosen by hand  '}})
            scan(full=True)
            locked = listed()['Blade.Runner.2049.2017.1080p.BluRay.mkv']
            assert locked['locked'] is True and locked['result']['title'] == 'Chosen by hand', locked['result']

            (tree / 'Show/Extras/Interview.mkv').unlink()
            scan()
            for _ in range(100):
                gone = listed()['Interview.mkv']
                if gone['available'] is False:
                    break
                time.sleep(.05)
            else:
                raise AssertionError('removed media was not reported unavailable')
            assert listed()['Nested.Film.2015.avi']['available'] is not False
        finally:
            denied.chmod(0o755)
            # Later groups and the browser suite expect only the three fixture videos.
            for table in ('library_files', 'files'):
                db.execute(f'DELETE FROM {table} WHERE path LIKE ?', (str(tree.resolve()).replace('%', '[%]') + '/%',))
            db.commit()
            assert db.execute('SELECT COUNT(*) FROM files').fetchone()[0] == 3
    with case('owner-chosen conversion folder: validation, use by HLS, cleanup and reset'):
        import shutil, tempfile
        outside = pathlib.Path(tempfile.mkdtemp(prefix='plexv2-work-'))  # the fixture media root is the whole test directory
        work = outside / 'work-folder'
        for bad in ['relative/path', '', ' ', str(base / 'inside-media-root'), '/proc/definitely/not/writable\u0000']:
            if bad.strip():
                api('/api/owner/settings', {'key': 'transcode_dir', 'value': bad}, status=400)
        assert api('/api/owner/settings')['transcode_dir_from_environment'] is False
        api('/api/owner/settings', {'key': 'transcode_dir', 'value': 'x' * 2000}, status=400)
        saved = api('/api/owner/settings', {'key': 'transcode_dir', 'value': str(work)})
        assert saved['effective_transcode_dir'] == str(work.resolve()), saved
        session = api('/api/playback/hls/start', {'path': str(video), 'quality': '480'})
        session_dir = work.resolve() / ('smart-media-hls-' + session['id'])
        assert session_dir.is_dir(), list(work.iterdir())
        api('/api/playback/hls/stop', {'id': session['id']})
        for _ in range(100):
            if not session_dir.exists():
                break
            time.sleep(.05)
        else:
            raise AssertionError('stopped stream left its files in the chosen folder')
        admin = login('reg-admin@example.test', 'regression password 123')
        api('/api/owner/settings', {'key': 'transcode_dir', 'value': str(base)}, status=403, opener=admin)
        api('/api/owner/settings', {'key': 'transcode_dir', 'value': ''})
        assert api('/api/owner/settings')['effective_transcode_dir'] != str(work.resolve())
        shutil.rmtree(outside, ignore_errors=True)
    with case('metadata validation, missing keys and cached collection reads'):
        for query in [{'q':'','kind':'movie'},{'q':'title','kind':'unknown'},{'q':'x'*201,'kind':'tv'}]:api('/api/metadata/search?'+urlencode(query),status=400)
        api('/api/metadata/search?'+urlencode({'q':'Example','kind':'movie'}),status=400)
        api('/api/metadata/detail?kind=invalid&id=1',status=400)
        api('/api/metadata/detail?kind=movie&id=-1',status=400)
        api('/api/metadata/match',{'path':str(video),'kind':'movie','id':-1},status=400)
        api('/api/override',{'path':str(video),'override':{}},status=400)
        api('/api/override',{'path':'/missing','override':{'kind':'movie'}},status=404)
        assert isinstance(api('/api/movie-collections')['collections'],dict)
    with case('subtitle lookup validation, conversion and local provider storage'):
        api('/api/subtitles/status')
        api('/api/subtitles/search?q=Example',status=400)
        api('/api/owner/providers/tmdb',{'key':'bad token'},status=400)
        api('/api/owner/providers/opensubtitles',{'key':'bad key'},status=400)
        providers=api('/api/owner/providers')
        assert not providers['tmdb_configured'] and not providers['opensubtitles_configured']
        # Test writes use fictional local-only values; never call a provider with them.
        api('/api/owner/providers/tmdb',{'key':'regression-local-token'})
        assert api('/api/owner/providers')['tmdb_configured']
        assert b'regression-local-token' not in request('/api/owner/providers')[0]
        token=base/'data'/'tmdb-token';assert token.read_text().strip()=='regression-local-token'
        if __import__('os').name!='nt':assert token.stat().st_mode & 0o077==0
        token.unlink()
        info=api('/api/playback/info?'+q)
        subtitle=info['subtitles'][0]['id']
        text=request('/api/playback/subtitle?'+q+'&'+urlencode({'subtitle':subtitle}))[0]
        assert b'WEBVTT' in text and b'Subtitle regression' in text
        request('/api/playback/subtitle?'+q+'&subtitle=embedded:999',status=400)
        request('/api/playback/subtitle?'+q+'&subtitle=file:999',status=400)
        request('/api/playback/subtitle?'+q+'&subtitle=../../etc/passwd',status=400)
        request('/api/playback/subtitle?'+q+'&subtitle='+subtitle+'&start=-1',status=400)
    with case('media boundaries, suffix ranges, missing files and HLS input validation'):
        raw,headers=request('/api/playback/file?'+urlencode({'path':str(direct)}),headers={'Range':'bytes=-16'},status=206)
        assert raw==direct.read_bytes()[-16:]
        for value in ['bytes=5-1','bytes=-0','bytes=0-1,4-5','invalid']:
            request('/api/playback/file?'+urlencode({'path':str(direct)}),headers={'Range':value},status=416)
        for route in ['/api/playback/file','/api/playback/info','/api/playback/transcode','/api/playback/subtitle','/api/playback/thumbnail']:
            request(route+'?'+urlencode({'path':'/not-in-library','time':0}),status=404)
        for payload in [{'start':-1},{'start':1000001},{'quality':'bad'},{'audio':999}]:api('/api/playback/hls/start',{'path':str(video),**payload},status=400)
        request('/api/playback/transcode?'+q+'&quality=bad',status=400)
        request('/api/playback/transcode?'+q+'&start=-1',status=400)
        request('/api/playback/thumbnail?'+q+'&time=NaN',status=400)
        api('/api/playback/hls/stop',{'id':'unknown'},status=404)
    with case('progressive conversion produces decodable video after seeking'):
        import subprocess
        converted=base/'progressive-output.mp4'
        converted.write_bytes(request('/api/playback/transcode?'+q+'&quality=480&audio=1&start=2')[0])
        probe=json.loads(subprocess.run(['ffprobe','-v','error','-show_streams','-of','json',str(converted)],check=True,capture_output=True).stdout)
        assert any(x['codec_type']=='video' and x['codec_name']=='h264' for x in probe['streams'])
        assert any(x['codec_type']=='audio' and x['codec_name']=='aac' for x in probe['streams'])
        subprocess.run(['ffmpeg','-v','error','-i',str(converted),'-f','null','-'],check=True,capture_output=True)
    with case('sidecar subtitles are allowlisted, retimed and Unicode-safe'):
        sidecar=base/'fixture.sl.srt'
        sidecar.write_text('1\n00:00:02,000 --> 00:00:06,000\nŽivjo čšž\n')
        original=db.execute('SELECT result FROM files WHERE path=?',(str(video),)).fetchone()[0]
        result=json.loads(original);result['subtitles']=[sidecar.name]
        db.execute('UPDATE files SET result=? WHERE path=?',(json.dumps(result),str(video)));db.commit()
        try:
            info=api('/api/playback/info?'+q)
            assert any(x['id']=='file:0' for x in info['subtitles'])
            text=request('/api/playback/subtitle?'+q+'&subtitle=file:0&start=3')[0].decode()
            assert 'Živjo čšž' in text and '00:00:03.000' in text
        finally:
            db.execute('UPDATE files SET result=? WHERE path=?',(original,str(video)));db.commit()
    with case('session self-revocation rejection and logout invalidate cookies'):
        sessions=api('/api/auth/sessions',opener=user)['items'];current=next(x for x in sessions if x['current'])
        api('/api/auth/sessions',{'id':current['id']},status=400,opener=user)
        api('/api/auth/sessions',{},status=400,opener=user)
        api('/api/auth/logout',{},opener=user)
        api('/api/auth/me',status=401,opener=user)
    with case('expired sessions are rejected'):
        expired=login('reg-admin@example.test','regression password 123')
        sid=next(x['id'] for x in api('/api/auth/sessions',opener=expired)['items'] if x['current'])
        db.execute('UPDATE auth_sessions SET expires_at=0 WHERE token_hash=?',(sid,));db.commit()
        api('/api/auth/me',status=401,opener=expired)
    with case('removed features have no routes or new database tables'):
        for path in ['/api/playback/markers','/api/admin/playback/markers','/api/playback/report','/api/admin/playback/reports']:
            request(path,status=404);request(path,{},status=404)
        names={row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        assert 'playback_markers' not in names and 'playback_reports' not in names
    return completed
