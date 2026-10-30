#!/usr/bin/env python3
"""End-to-end test of the owner-started self-update against a stub release server on localhost.

A copy of the real program is started from a temporary folder, finds a fake "9.9.9" release, and
replaces itself. Checks the checksum guard, the smoke test, rollback safety, restart in place,
the Docker / read-only / disabled variants and permissions. Nothing is downloaded from the internet."""
import hashlib, http.cookiejar, http.server, json, os, pathlib, platform, shutil, socket, stat, subprocess, sys, tempfile, threading, time, urllib.error, urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = pathlib.Path(os.environ.get('PLEXV2_TEST_BINARY', str(ROOT / 'target/release/smart-media-scanner')))
PASSWORD = 'self update password 1'
OS = {'linux': 'linux', 'darwin': 'macos', 'win32': 'windows'}[sys.platform]
ARCH = {'x86_64': 'x86_64', 'amd64': 'x86_64', 'arm64': 'aarch64', 'aarch64': 'aarch64'}[platform.machine().lower()]
SUFFIX = {'linux': f'linux-{ARCH}', 'macos': 'macos-universal', 'windows': 'windows-x86_64'}[OS]
ASSET = f'platno-9.9.9-{SUFFIX}'
PROGRAM = BINARY.read_bytes()
CURRENT_VERSION = subprocess.check_output([str(BINARY), '--version'], text=True).strip().split()[-1]


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


class Release(http.server.BaseHTTPRequestHandler):
    mode = 'good'  # good | badsum | garbage
    port = 0

    def payload(self):
        return b'this is not a program' * 4 if Release.mode == 'garbage' else PROGRAM

    def do_GET(self):
        if self.path == '/repos/test/releases/latest':
            base = f'http://127.0.0.1:{Release.port}/download'
            body = json.dumps({'tag_name': 'v9.9.9', 'name': 'Platno 9.9.9', 'body': 'Fake release for the update test.', 'published_at': '2026-11-01T10:00:00Z', 'html_url': f'http://127.0.0.1:{Release.port}/page',
                               'assets': [{'name': ASSET, 'size': len(self.payload()), 'browser_download_url': f'{base}/{ASSET}'}, {'name': 'SHA256SUMS', 'size': 100, 'browser_download_url': f'{base}/SHA256SUMS'}]}).encode()
        elif self.path == f'/download/{ASSET}':
            body = self.payload()
        elif self.path == '/download/SHA256SUMS':
            digest = hashlib.sha256(self.payload()).hexdigest()
            body = f"{'0' * 64 if Release.mode == 'badsum' else digest}  {ASSET}\n".encode()
        else:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


class Server:
    def __init__(self, tmp, api, **extra):
        self.dir = pathlib.Path(tmp) / f'app-{len(list(pathlib.Path(tmp).iterdir()))}'
        (self.dir / 'bin').mkdir(parents=True)
        self.exe = self.dir / 'bin' / ('platno.exe' if OS == 'windows' else 'platno')
        shutil.copy2(BINARY, self.exe)
        self.port = free_port()
        self.url = f'http://127.0.0.1:{self.port}'
        env = {**os.environ, 'SMART_MEDIA_DATA': str(self.dir / 'data'), 'SMART_MEDIA_HOST': '127.0.0.1', 'SMART_MEDIA_PORT': str(self.port), 'SMART_MEDIA_UPDATE_API': api,
               'SMART_MEDIA_COOKIE_SECURE': '0', 'SMART_MEDIA_WIKIDATA': 'off', 'TMDB_TOKEN': '', **extra}
        self.proc = subprocess.Popen([str(self.exe)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        wait(lambda: self.try_call('GET', '/api/auth/setup-status') is not None, 'server start')
        self.call('POST', '/api/auth/setup', {'email': 'owner@example.test', 'password': PASSWORD, 'confirm_password': PASSWORD})

    def try_call(self, method, path, body=None):
        try:
            return self.call(method, path, body)
        except (OSError, urllib.error.URLError):
            return None

    def call(self, method, path, body=None, opener=None):
        request = urllib.request.Request(self.url + path, method=method, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
        try:
            with (opener or self.client).open(request, timeout=30) as response:
                raw = response.read()
                return response.status, json.loads(raw or b'null')
        except urllib.error.HTTPError as error:
            raw = error.read()
            return error.code, json.loads(raw or b'null')

    def close(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()


def wait(predicate, what, seconds=30):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if predicate():
            return
        time.sleep(.2)
    raise AssertionError(f'timed out waiting for {what}')


def leftovers(server):
    return [p.name for p in (server.dir / 'bin').iterdir() if p.name.startswith('.platno-')]


def main():
    stub = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Release)
    Release.port = stub.server_address[1]
    threading.Thread(target=stub.serve_forever, daemon=True).start()
    api = f'http://127.0.0.1:{Release.port}/repos/test'
    servers = []
    with tempfile.TemporaryDirectory(prefix='plexv2-update-') as tmp:
        try:
            server = Server(tmp, api)
            servers.append(server)
            status, info = server.call('GET', '/api/owner/update')
            assert status == 200 and info['method'] == 'selfupdate' and info['enabled'] and not info['update_available'] and not info['can_install'], info
            assert info['current'] == CURRENT_VERSION and info['platform_supported'], info
            assert server.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})[0] == 400, 'installing requires a check first'
            # Only the owner may look at or change updates.
            server.call('POST', '/api/admin/accounts', {'email': 'admin@example.test', 'password': PASSWORD, 'confirm_password': PASSWORD, 'role': 'admin'})
            admin = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
            server.call('POST', '/api/auth/login', {'email': 'admin@example.test', 'password': PASSWORD}, opener=admin)
            for method, path in [('GET', '/api/owner/update'), ('POST', '/api/owner/update/check'), ('POST', '/api/owner/update/apply')]:
                assert server.call(method, path, {}, opener=admin)[0] == 403, path
            assert server.call('POST', '/api/owner/update', {})[0] in (404, 405)

            status, info = server.call('POST', '/api/owner/update/check', {})
            assert status == 200 and info['update_available'] and info['can_install'] and info['latest']['version'] == '9.9.9' and info['latest']['asset']['name'] == ASSET, info
            assert server.call('POST', '/api/owner/update/apply', {'version': '9.9.8'})[0] == 400, 'only the version that was found may be installed'
            inode = server.exe.stat().st_ino

            Release.mode = 'badsum'
            status, body = server.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})
            assert status == 502 and 'checksum' in body['error'], body
            Release.mode = 'garbage'
            status, body = server.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})
            assert status == 502 and 'does not run' in body['error'], body
            assert server.exe.stat().st_ino == inode and leftovers(server) == [] and not (server.exe.parent / f'{server.exe.name}.previous').exists(), 'a refused update changes nothing'
            assert server.try_call('GET', '/api/owner/update') is not None, 'the server keeps running after a refused update'

            Release.mode = 'good'
            pid, since = server.proc.pid, server.call('GET', '/api/owner/update')[1]['running_since']
            status, body = server.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})
            assert status == 200 and body['restarting'], body
            wait(lambda: (lambda r: r and r[0] == 200 and r[1]['running_since'] != since)(server.try_call('GET', '/api/owner/update')), 'restart after the update')
            assert server.exe.stat().st_ino != inode, 'the program file was replaced'
            assert server.exe.read_bytes() == PROGRAM and (server.exe.parent / f'{server.exe.name}.previous').is_file() and leftovers(server) == []
            if OS != 'windows':
                assert server.proc.pid == pid and server.proc.poll() is None, 'the server restarts inside the same process'
            assert server.call('POST', '/api/auth/login', {'email': 'owner@example.test', 'password': PASSWORD})[0] == 200, 'accounts survive the update'
            assert not server.call('GET', '/api/owner/update')[1]['update_available'], 'the in-memory check result is gone after a restart'
            server.close()

            docker = Server(tmp, api, SMART_MEDIA_DISTRIBUTION='docker')
            servers.append(docker)
            info = docker.call('GET', '/api/owner/update')[1]
            assert info['method'] == 'docker' and 'docker compose pull' in info['instructions'], info
            docker.call('POST', '/api/owner/update/check', {})
            status, body = docker.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})
            assert status == 409 and 'docker compose pull' in body['error'], body
            docker.close()

            off = Server(tmp, api, SMART_MEDIA_UPDATE_CHECK='off')
            servers.append(off)
            assert off.call('GET', '/api/owner/update')[1]['enabled'] is False and off.call('POST', '/api/owner/update/check', {})[0] == 403
            off.close()

            if OS != 'windows' and os.geteuid() != 0:
                locked = Server(tmp, api)
                servers.append(locked)
                os.chmod(locked.exe.parent, stat.S_IRUSR | stat.S_IXUSR)
                try:
                    assert locked.call('GET', '/api/owner/update')[1]['method'] == 'readonly'
                    locked.call('POST', '/api/owner/update/check', {})
                    assert locked.call('POST', '/api/owner/update/apply', {'version': '9.9.9'})[0] == 409
                finally:
                    os.chmod(locked.exe.parent, stat.S_IRWXU)
                locked.close()
            print('PASS: owner-only update check, checksum and smoke-test guards, in-place restart that keeps accounts, previous version kept, Docker, read-only and disabled variants')
        finally:
            for server in servers:
                server.close()
            stub.shutdown()


if __name__ == '__main__':
    sys.exit(main())
