#!/usr/bin/env python3
"""Private .env is the sole main-app demo credential authority."""
import os
import fcntl
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time
import urllib.request
import urllib.error

ROOT = Path(__file__).resolve().parents[1]

def read_token(path):
    values = re.findall(r'^DEMO_TOKEN=(.*)$', path.read_text(), re.M)
    if len(values) != 1:
        raise ValueError('Expected exactly one DEMO_TOKEN in private .env (redacted).')
    token = values[0].strip().strip('\"\'')
    if not re.fullmatch('[0-9a-fA-F]{64}', token):
        raise ValueError('DEMO_TOKEN must contain 64 hexadecimal characters (redacted).')
    return token

def replace_token(path, token):
    content = path.read_text()
    if len(re.findall(r'^DEMO_TOKEN=', content, re.M)) != 1:
        raise ValueError('Expected exactly one DEMO_TOKEN; no configuration changed.')
    # In-place write preserves the bind-mounted inode, ownership and permissions.
    with path.open('r+') as file:
        file.write(re.sub(r'^DEMO_TOKEN=.*$', 'DEMO_TOKEN=' + token, content, flags=re.M))
        file.truncate()
        file.flush()
        os.fsync(file.fileno())

def status(token):
    request = urllib.request.Request('http://127.0.0.1:8080/demo/config', headers={
        'Authorization': 'Bearer ' + token, 'X-Demo-Client': 'demo61'})
    try:
        with urllib.request.urlopen(request, timeout=2) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code

def main():
    path = ROOT / '.env'
    if sys.argv[1:] == ['launch']:
        os.environ['DEMO_TOKEN'] = read_token(path)
        os.execv('/app/target/debug/reliable-event-relay', ['reliable-event-relay'])
    if sys.argv[1:] != ['rotate']:
        raise ValueError('Usage: python3 /app/scripts/demo-token.py rotate')
    # Serialize explicit rotations; HTTP launch reads the completed file without locking.
    with path.open('r') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        rotate(path)

def rotate(path):
    old = read_token(path)
    token = secrets.token_hex(32)
    replace_token(path, token)
    result = subprocess.run(['supervisorctl', 'restart', 'http'], capture_output=True)
    if result.returncode:
        raise ValueError('Token saved in .env; HTTP restart failed. Run supervisorctl start http; retrieve token privately from .env.')
    for _ in range(30):
        try:
            if status(token) == 200 and status(old) == 401:
                print('Activated; previous token rejected. Keep this token private. Enter it in Local demo token:')
                print(token)
                return
        except (OSError, urllib.error.URLError):
            pass
        time.sleep(1)
    raise ValueError('Token saved; activation verification failed. Inspect named http process; retrieve token privately from .env.')

if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
    except Exception:
        print('Demo token command failed (details redacted). Check private .env and supervisorctl status; activation may be pending.', file=sys.stderr)
        sys.exit(1)
