"""Exercise the real loopback API using isolated synthetic data only."""
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'debug' / ('hns-collector.exe' if os.name == 'nt' else 'hns-collector')


def main():
    subprocess.run(['cargo', 'build', '--locked', '-p', 'hns-collector'], cwd=ROOT, check=True)
    with tempfile.TemporaryDirectory(prefix='hns-smoke-') as directory:
        token_path = Path(directory) / 'token'
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        child = subprocess.Popen([str(BINARY), 'serve', '--demo', '--port', str(port), '--token-file', str(token_path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            token = ''
            def request(method, path, data=None, authenticated=True):
                conn = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
                headers = {'Authorization': f'Bearer {token}'} if authenticated else {}
                body = None if data is None else json.dumps(data)
                try:
                    conn.request(method, path, body, headers)
                    response = conn.getresponse()
                    assert response.getheader('Access-Control-Allow-Origin') is None
                    assert response.getheader('Cache-Control') == 'no-store'
                    return response.status, json.loads(response.read())
                finally:
                    conn.close()
            for _ in range(100):
                if child.poll() is not None:
                    raise RuntimeError('Collector exited before becoming ready')
                if token_path.exists():
                    token = token_path.read_text().strip()
                    try:
                        status, data = request('GET', '/v1/snapshot')
                        if status == 200:
                            break
                    except (ConnectionError, OSError):
                        pass
                time.sleep(0.1)
            else:
                raise RuntimeError('Collector startup timed out')
            assert data['mode'] == 'demo' and len(data['devices']) == 6
            assert request('GET', '/v1/snapshot', authenticated=False)[0] == 401
            assert request('GET', '/v1/snapshot?sensor=missing')[0] == 400
            assert request('GET', '/v1/snapshot?sensor=sample&since=253402300799')[1]['observationCount'] == 0
            assert request('GET', '/v1/snapshot?since=invalid')[0] == 400
            device_id = data['devices'][0]['id']
            assert request('POST', '/v1/rename', {'id': device_id, 'name': 'Smoke device'})[0] == 200
            data = request('GET', '/v1/snapshot')[1]
            assert next(d for d in data['devices'] if d['id'] == device_id)['name'] == 'Smoke device'
            alert_id = data['alerts'][0]['id']
            assert request('POST', '/v1/acknowledge', {'id': alert_id})[0] == 200
            data = request('GET', '/v1/snapshot')[1]
            assert next(a for a in data['alerts'] if a['id'] == alert_id)['acknowledged']
            assert request('POST', '/v1/rename', {'id': device_id, 'name': ''})[0] == 400
            assert request('POST', '/v1/rename', {'id': device_id, 'name': 'x' * 5000})[0] == 400
            assert request('GET', '/unknown')[0] == 400
            print('PASS: authenticated API, denied unauthenticated access, sample snapshot, persisted rename/review, invalid input and size limits')
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)


if __name__ == '__main__':
    main()
