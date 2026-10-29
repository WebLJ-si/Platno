#!/usr/bin/env python3
"""End-to-end check of error/heartbeat forwarding to a collector, entirely on localhost.

A stub collector listens on 127.0.0.1 (the only plain-HTTP endpoint the server accepts), a beta
server forwards to it, and every captured payload is validated with the real collector's schema.
Nothing is sent to any external host."""
import http.cookiejar, http.server, json, os, pathlib, socket, subprocess, sys, tempfile, threading, time, urllib.error, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = os.environ.get('PLEXV2_TEST_BINARY', str(ROOT / 'target/debug/smart-media-scanner'))
CREDENTIALS = {'email': 'owner@example.test', 'password': 'telemetry password 123', 'confirm_password': 'telemetry password 123'}
GOOD = {'schema': 1, 'sessionId': '0123456789abcdef0123456789abcdef', 'kind': 'browser_error', 'code': 'read-null', 'scene': 'library',
        'uptimeSeconds': 42, 'release': '0.1.0', 'frames': [{'asset': 'app', 'line': 7, 'column': 3}],
        'context': {'runtime': 'browser', 'browser': 'Chrome'}, 'replay': [{'kind': 'click', 'scene': 'library', 'offsetMs': 5, 'target': 'button'}]}


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class Collector(http.server.BaseHTTPRequestHandler):
    received = []

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        Collector.received.append((self.path, json.loads(body)))
        self.send_response(202)
        self.end_headers()

    def log_message(self, *args):
        pass


class Server:
    def __init__(self, endpoint, **extra):
        self.tmp = tempfile.TemporaryDirectory(prefix='plexv2-telemetry-')
        self.port = free_port()
        self.url = f'http://127.0.0.1:{self.port}'
        env = {**os.environ, 'SMART_MEDIA_DATA': self.tmp.name, 'SMART_MEDIA_HOST': '127.0.0.1', 'SMART_MEDIA_PORT': str(self.port),
               'SMART_MEDIA_TELEMETRY_ENDPOINT': endpoint, 'SMART_MEDIA_COOKIE_SECURE': '0', 'TMDB_TOKEN': '', **extra}
        self.proc = subprocess.Popen([BINARY], cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        for _ in range(100):
            try:
                self.call('GET', '/api/auth/setup-status')
                break
            except OSError:
                time.sleep(.1)
        else:
            raise AssertionError('server did not start')
        assert self.call('POST', '/api/auth/setup', CREDENTIALS)[0] == 200

    def call(self, method, path, body=None):
        request = urllib.request.Request(self.url + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json'})
        try:
            with self.client.open(request, timeout=20) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as error:
            return error.code, error.read()

    def close(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
        self.tmp.cleanup()


def wait_for(predicate, what, seconds=15):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if predicate():
            return
        time.sleep(.1)
    raise AssertionError(f'timed out waiting for {what}; received={[r[1].get("kind") for r in Collector.received]}')


def validate_with_collector_schema(payloads):
    schema = ROOT / 'telemetry-server/lib/schema.mjs'
    if not (ROOT / 'telemetry-server/node_modules/zod').exists():
        print('NOTE: telemetry-server dependencies missing; payload schema validation skipped')
        return
    script = ("import {parseReport} from %s;let all=JSON.parse(process.argv[1]);for(const p of all)parseReport(p);" % json.dumps(schema.as_uri()))
    result = subprocess.run(['node', '--input-type=module', '-e', script, json.dumps(payloads)], capture_output=True, text=True)
    assert result.returncode == 0, f'collector schema rejected a payload: {result.stderr[-600:]}'


def main():
    collector = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Collector)
    threading.Thread(target=collector.serve_forever, daemon=True).start()
    endpoint = f'http://127.0.0.1:{collector.server_address[1]}/api/ingest'
    servers = []
    try:
        beta = Server(endpoint, SMART_MEDIA_RELEASE_CHANNEL='beta')
        servers.append(beta)
        config = json.loads(beta.call('GET', '/api/telemetry/config')[1])
        assert config['available'] and config['enabled'] and config['channel'] == 'beta', config
        wait_for(lambda: any(p['kind'] == 'heartbeat' for _, p in Collector.received), 'startup heartbeat')

        assert beta.call('POST', '/api/telemetry/report', GOOD)[0] == 202
        wait_for(lambda: any(p['kind'] == 'browser_error' for _, p in Collector.received), 'forwarded browser error')
        forwarded = next(p for _, p in Collector.received if p['kind'] == 'browser_error')
        assert forwarded['code'] == 'read-null' and 'serverUptimeSeconds' in forwarded and forwarded['release'] == '0.1.0', forwarded

        before = len(Collector.received)
        for private in [{**GOOD, 'message': 'private movie title'}, {**GOOD, 'code': 'secret-token-value'},
                        {**GOOD, 'context': {'email': 'user@example.test'}}, {**GOOD, 'scene': '/Users/me/movie.mkv'}]:
            assert beta.call('POST', '/api/telemetry/report', private)[0] == 400, private
        time.sleep(1)
        assert len(Collector.received) == before, 'rejected reports must never be forwarded'
        assert all(path == '/api/ingest' for path, _ in Collector.received)
        validate_with_collector_schema([p for _, p in Collector.received])

        assert json.loads(beta.call('POST', '/api/owner/telemetry', {'enabled': False})[1])['enabled'] is False
        assert beta.call('POST', '/api/telemetry/report', GOOD)[0] == 204
        time.sleep(1)
        assert len(Collector.received) == before, 'owner opt-out must stop forwarding'
        assert json.loads(beta.call('POST', '/api/owner/telemetry', {'enabled': True})[1])['enabled'] is True
        assert beta.call('POST', '/api/telemetry/report', {**GOOD, 'kind': 'rejection', 'code': 'unknown'})[0] == 202
        wait_for(lambda: len(Collector.received) > before, 'forwarding after re-enabling')

        before = len(Collector.received)
        stable = Server(endpoint)
        servers.append(stable)
        assert json.loads(stable.call('GET', '/api/telemetry/config')[1])['available'] is False
        assert stable.call('POST', '/api/telemetry/report', GOOD)[0] == 204
        off = Server(endpoint, SMART_MEDIA_RELEASE_CHANNEL='beta', SMART_MEDIA_TELEMETRY='off')
        servers.append(off)
        assert json.loads(off.call('GET', '/api/telemetry/config')[1])['enabled'] is False
        time.sleep(2)
        assert len(Collector.received) == before, 'stable builds and SMART_MEDIA_TELEMETRY=off must send nothing'
        print('PASS: heartbeat and browser error forwarded to collector, schema-valid, private fields rejected, owner opt-out, stable and TELEMETRY=off send nothing')
    finally:
        for server in servers:
            server.close()
        collector.shutdown()


if __name__ == '__main__':
    sys.exit(main())
