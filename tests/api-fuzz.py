#!/usr/bin/env python3
"""Throw malformed requests at every route of a disposable server.

Passes when no request returns HTTP 500, the connection never drops, and the server
still answers afterwards. 502/503 are legitimate (no network, no FFmpeg)."""
import http.cookiejar, json, os, pathlib, re, socket, subprocess, sys, tempfile, time, urllib.error, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
# Opens a native dialog or ends the session the fuzzer itself depends on.
SKIP = {'/api/auth/logout', '/api/scan'}
SAMPLES = {'id': '1', 'file': 'index.m3u8', 'name': 'x'}
BODIES = [
    b'', b'null', b'[]', b'{}', b'"text"', b'0', b'true', b'{"path":null}', b'{"path":123}', b'{"path":"\\u0000"}',
    b'{"id":-1,"kind":"movie"}', b'{"id":9223372036854775808}', b'{"enabled":"yes"}', b'{"key":"x","value":"y"}',
    b'{"path":"' + b'A' * 100_000 + b'"}', b'{"a":', b'\xff\xfe\x00', b'[' * 5000 + b']' * 5000,
    b'{"position":1e999,"duration":-1,"path":"/x"}', b'{"paths":["/a"],"action":"reset"}',
    '{"name":"Žluťoučký kůň \U0001F600","pin":"١٢٣٤","confirm_pin":"١٢٣٤"}'.encode(),
]
QUERIES = ['', '?path=', '?path=%00', '?path=' + 'A' * 10_000, '?id=-1', '?id=abc', '?q=%FF%FE', '?time=NaN', '?start=1e999',
           '?kind=movie&id=99999999999999999999', '?subtitle=embedded:99999999999', '?audio=-1', '?quality=%00']


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def main():
    binary = os.environ.get('PLEXV2_TEST_BINARY', str(ROOT / 'target/debug/smart-media-scanner'))
    routes = sorted({m.replace('{id}', SAMPLES['id']).replace('{file}', SAMPLES['file']).replace('{name}', SAMPLES['name'])
                     for m in re.findall(r'\.route\(\s*"(/[^"]*)"', (ROOT / 'src/main.rs').read_text())})
    with tempfile.TemporaryDirectory(prefix='plexv2-fuzz-') as tmp:
        port = free_port()
        url = f'http://127.0.0.1:{port}'
        env = {**os.environ, 'SMART_MEDIA_DATA': tmp, 'SMART_MEDIA_HOST': '127.0.0.1', 'SMART_MEDIA_PORT': str(port),
               'TMDB_TOKEN': '', 'OPENSUBTITLES_API_KEY': '', 'SMART_MEDIA_COOKIE_SECURE': '0', 'SMART_MEDIA_WIKIDATA': 'off'}
        proc = subprocess.Popen([binary], cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        failures, sent = [], 0

        def send(method, path, body=None):
            req = urllib.request.Request(url + path, data=body, method=method,
                                         headers={'Content-Type': 'application/json'} if body is not None else {})
            try:
                with opener.open(req, timeout=20) as response:
                    response.read(1_000_000)
                    return response.status
            except urllib.error.HTTPError as error:
                error.read(1_000_000)
                return error.code
        try:
            for _ in range(100):
                try:
                    send('GET', '/api/auth/setup-status')
                    break
                except OSError:
                    time.sleep(.1)
            else:
                raise AssertionError('server did not start')
            credentials = json.dumps({'email': 'fuzz@example.test', 'password': 'fuzzing password 123',
                                      'confirm_password': 'fuzzing password 123'}).encode()
            assert send('POST', '/api/auth/setup', credentials) == 200
            # The setup route is intentionally public; fuzz it after the owner exists as well.
            for path in routes:
                if path in SKIP:
                    continue
                cases = [('GET', path + q, None) for q in QUERIES] + [('POST', path, b) for b in BODIES] + [('POST', path + '?x=%00', None)]
                for method, target, body in cases:
                    sent += 1
                    try:
                        status = send(method, target, body)
                    except Exception as error:  # connection reset or timeout means the handler crashed or hung
                        failures.append((method, target[:80], repr(error)[:80]))
                        continue
                    if status == 500:
                        failures.append((method, target[:80], status, (body or b'')[:40]))
                    if proc.poll() is not None:
                        raise AssertionError(f'server exited after {method} {target[:80]}')
            assert send('GET', '/api/auth/me') == 200, 'server stopped answering'
            assert not failures, failures[:20]
            print(f'PASS: {sent} malformed requests across {len(routes) - len(SKIP & set(routes))} routes produced no 500s, hangs or crashes')
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()


if __name__ == '__main__':
    sys.exit(main())
