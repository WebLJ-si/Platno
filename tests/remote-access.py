#!/usr/bin/env python3
"""Isolated HTTPS/auth/lifecycle tests. Never discovers or modifies a real router."""
import http.client
import http.cookiejar
import json
import os
import pathlib
import socket
import ssl
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = os.environ.get('PLEXV2_TEST_BINARY', str(ROOT / 'target/debug/smart-media-scanner'))
HOSTNAME = 'media.example.test'
PASSWORD = 'remote integration password 123'

def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]

with tempfile.TemporaryDirectory(prefix='platno-remote-test-') as tmp:
    base = pathlib.Path(tmp)
    http_port, tls_port = free_port(), free_port()
    cert, key = base/'cert.pem', base/'key.pem'
    subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', f'/CN={HOSTNAME}', '-addext', f'subjectAltName=DNS:{HOSTNAME}', '-keyout', str(key), '-out', str(cert)], check=True, capture_output=True)
    context = ssl.create_default_context(cafile=str(cert))
    env = {**os.environ, 'SMART_MEDIA_DATA': str(base/'data'), 'SMART_MEDIA_HOST': '127.0.0.1', 'SMART_MEDIA_PORT': str(http_port), 'SMART_MEDIA_COOKIE_SECURE': '0', 'SMART_MEDIA_REMOTE_HOSTNAME': HOSTNAME, 'SMART_MEDIA_REMOTE_IP': '127.0.0.1', 'SMART_MEDIA_REMOTE_PORT': str(tls_port), 'SMART_MEDIA_TLS_CERT': str(cert), 'SMART_MEDIA_TLS_KEY': str(key)}
    log = open(base/'server.log', 'w')
    jar = http.cookiejar.CookieJar()
    client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
    proc = None

    def start():
        global proc
        proc = subprocess.Popen([BINARY], cwd=ROOT, env=env, stdout=log, stderr=log)
        for _ in range(120):
            try:
                local('/api/auth/setup-status')
                # The remote manager initializes after the HTTP socket binds.
                time.sleep(.1)
                return
            except (OSError, urllib.error.URLError):
                time.sleep(.1)
        raise AssertionError('server did not start')

    def stop():
        if proc and proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=20)

    def local(path, body=None, status=200, headers=None, opener=None):
        req = urllib.request.Request(f'http://127.0.0.1:{http_port}'+path, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type':'application/json', **(headers or {})})
        try:
            with (opener or client).open(req, timeout=30) as response:
                code, raw = response.status, response.read()
        except urllib.error.HTTPError as response:
            code, raw = response.code, response.read()
        assert code == status, (path, code, status, raw)
        return json.loads(raw) if raw else None

    class TLSConnection(http.client.HTTPSConnection):
        def connect(self):
            self.sock = context.wrap_socket(socket.create_connection(('127.0.0.1', tls_port), timeout=10), server_hostname=HOSTNAME)

    def remote(path, body=None, status=200, headers=None):
        conn = TLSConnection(HOSTNAME, tls_port, context=context)
        conn.request('GET' if body is None else 'POST', path, body=None if body is None else json.dumps(body), headers={'Host':f'{HOSTNAME}:{tls_port}', 'Content-Type':'application/json', **(headers or {})})
        response = conn.getresponse()
        raw, result_headers = response.read(), response.headers
        assert response.status == status, (path,response.status,status,raw)
        conn.close()
        return raw,result_headers

    def closed():
        for _ in range(40):
            try:
                with socket.create_connection(('127.0.0.1', tls_port), timeout=.2): pass
            except OSError: return
            time.sleep(.1)
        raise AssertionError('HTTPS listener remained open')

    try:
        start()
        closed()
        anonymous = urllib.request.build_opener()
        local('/api/owner/remote-access',status=401,opener=anonymous)
        local('/api/auth/setup',{'email':'owner@example.test','password':PASSWORD,'confirm_password':PASSWORD})
        state = local('/api/owner/remote-access')
        assert not state['enabled'] and state['configured'] and state['reachable'] is None
        local('/api/owner/remote-access',{'enabled':True,'automatic':False},status=403,headers={'Origin':'https://attacker.test'})
        local('/api/owner/remote-access',{'enabled':True,'automatic':False},status=403,headers={'Sec-Fetch-Site':'cross-site'})
        assert not local('/api/owner/remote-access')['enabled']
        state = local('/api/owner/remote-access',{'enabled':True,'automatic':False})
        assert state['enabled'] and not state['mapped'] and state['url']==f'https://{HOSTNAME}:{tls_port}'
        # TLS trust and hostname verification are active; no insecure SSL context.
        remote('/api/files',status=401)
        remote('/api/auth/setup',{},status=403)
        remote('/api/auth/setup-status',status=421,headers={'Host':'unexpected.example.test'})
        raw,headers = remote('/api/auth/login',{'email':'owner@example.test','password':PASSWORD})
        assert 'Secure' in headers['Set-Cookie'] and 'HttpOnly' in headers['Set-Cookie'] and 'SameSite=Strict' in headers['Set-Cookie']
        assert headers['Strict-Transport-Security']=='max-age=86400'
        cookie = headers['Set-Cookie'].split(';')[0]
        remote('/api/owner/remote-access',status=200,headers={'Cookie':cookie})
        remote('/api/owner/remote-access',{'enabled':False},status=403,headers={'Cookie':cookie,'Origin':'https://attacker.test'})
        local('/api/admin/accounts',{'email':'user@example.test','password':PASSWORD,'confirm_password':PASSWORD,'role':'user'})
        raw,headers = remote('/api/auth/login',{'email':'user@example.test','password':PASSWORD})
        remote('/api/owner/remote-access',status=403,headers={'Cookie':headers['Set-Cookie'].split(';')[0]})
        # A remote owner can disable without aborting their own response.
        remote('/api/owner/remote-access',{'enabled':False},headers={'Cookie':cookie,'Origin':f'https://{HOSTNAME}:{tls_port}'})
        closed()
        local('/api/owner/remote-access',{'enabled':True,'automatic':False})
        stop()
        closed()
        start()
        for _ in range(60):
            state = local('/api/owner/remote-access')
            if state['enabled']: break
            time.sleep(.1)
        assert state['enabled'], state
        remote('/api/auth/setup-status')
        local('/api/owner/remote-access',{'enabled':False})
        closed()
        # Loopback is explicitly ineligible for UPnP, so this cannot touch a router.
        local('/api/owner/remote-access',{'enabled':True,'automatic':True},status=400)
        closed()
        assert not local('/api/owner/remote-access')['enabled']
        cert.rename(base/'missing.pem')
        local('/api/owner/remote-access',{'enabled':True,'automatic':False},status=400)
        closed()
        stop()
        start()
        assert not local('/api/owner/remote-access')['enabled']
        print('PASS: HTTPS trust/hostname, secure cookies, owner-only API, setup block, CSRF, remote disable, restart persistence, shutdown cleanup, failed mapping and certificate setup')
    except BaseException:
        log.flush()
        print((base/'server.log').read_text())
        raise
    finally:
        stop()
        log.close()
