#!/usr/bin/python3
"""Sequential developer verification inside app. Owns no production resources."""
import fcntl
import base64
from urllib.parse import quote
import hashlib
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
CACHE = Path('/tmp/relay-tests')
CHILD = None
PASSED = []
PHASE = 'prerequisites'
SECRETS = [v for k, v in os.environ.items() if any(x in k for x in ('PASSWORD', 'PASS', 'TOKEN', 'OPERATORS')) and v]
if (ROOT / '.env').exists():
    SECRETS += [line.split('=', 1)[1].strip().strip('\"\'') for line in (ROOT / '.env').read_text().splitlines() if '=' in line and any(x in line.split('=', 1)[0] for x in ('PASSWORD', 'PASS', 'TOKEN', 'OPERATORS'))]

SECRETS += [os.environ[k] for k in ('POSTGRES_USER', 'RABBITMQ_DEFAULT_USER') if os.environ.get(k)]
SECRETS += [quote(value, safe='') for value in list(SECRETS) if value]
if os.environ.get('RABBITMQ_DEFAULT_USER') and os.environ.get('RABBITMQ_DEFAULT_PASS'):
    SECRETS.append(base64.b64encode((os.environ['RABBITMQ_DEFAULT_USER'] + ':' + os.environ['RABBITMQ_DEFAULT_PASS']).encode()).decode())


def sanitize(text):
    for value in sorted(filter(None, SECRETS), key=len, reverse=True):
        text = text.replace(value, '[redacted]')
    return text


def stop_child():
    global CHILD
    if CHILD is None:
        return
    # Supervisor children may own separate process groups. Capture the owned
    # descendant groups before delivering an interrupt to their parent.
    owned = {CHILD.pid}
    parents = {}
    for path in Path('/proc').glob('[0-9]*/status'):
        try:
            parent = next(line.split()[1] for line in path.read_text().splitlines() if line.startswith('PPid:'))
            parents[int(path.parent.name)] = int(parent)
        except (OSError, StopIteration):
            pass
    while True:
        found = {pid for pid, parent in parents.items() if parent in owned}
        if found <= owned:
            break
        owned |= found
    groups = {CHILD.pid}
    for pid in owned:
        try:
            group = os.getpgid(pid)
            if group != os.getpgrp(): groups.add(group)
        except ProcessLookupError:
            pass
    for group in groups:
        try: os.killpg(group, signal.SIGINT)
        except ProcessLookupError: pass
    try:
        CHILD.wait(timeout=30)
    except subprocess.TimeoutExpired:
        for group in groups:
            try: os.killpg(group, signal.SIGKILL)
            except ProcessLookupError: pass
        CHILD.wait()
    # Reap process groups whose manager exited before its children did.
    for group in groups:
        try: os.killpg(group, signal.SIGTERM)
        except ProcessLookupError: pass
    CHILD = None


def run(name, args, cwd=None, env=None):
    global CHILD, PHASE
    PHASE = name
    print(f'\n=== {name} ===', flush=True)
    # Private disk spool avoids pipe deadlocks and preserves the final failure context.
    with tempfile.TemporaryFile(mode='w+') as log:
        CHILD = subprocess.Popen(args, cwd=cwd or ROOT / 'backend', env=env or ENV,
                                 stdout=log, stderr=log, start_new_session=True)
        try:
            code = CHILD.wait()
        finally:
            stop_child()
        log.seek(0)
        output = sanitize(log.read())
        print(output[-24000:], end='', flush=True)
        if code:
            diagnostic = CACHE / 'last-failure.log'
            diagnostic.write_text(f'{name}: exit {code}\n' + output[-24000:])
            diagnostic.chmod(0o600)
            print(f'Sanitized diagnostics: {diagnostic}', flush=True)
            raise RuntimeError(f'{name}: exit {code}; later phases did not run')
    PASSED.append(name)


def interrupt(signum, frame):
    raise KeyboardInterrupt


def main():
    global ENV
    if sys.argv[1:]:
        raise RuntimeError('Usage: tests (no arguments); see testing/strategy.md for host acceptance')
    for executable in ('cargo', 'npm', 'node', 'sqlx', 'psql', 'supervisord', 'git'):
        if not shutil.which(executable):
            raise RuntimeError(f'Missing {executable}; rebuild the app image')
    CACHE.mkdir(mode=0o700, exist_ok=True)
    with (CACHE / 'invocation.lock').open('w') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError('Another tests invocation is running; wait for it to finish')
        required = ('POSTGRES_HOST','POSTGRES_PORT','POSTGRES_USER','POSTGRES_PASSWORD','POSTGRES_DB','RABBITMQ_HOST','RABBITMQ_PORT','RABBITMQ_DEFAULT_USER','RABBITMQ_DEFAULT_PASS','RABBITMQ_DEFAULT_VHOST')
        missing = [key for key in required if not os.environ.get(key)]
        if missing: raise RuntimeError('Missing Compose configuration keys: ' + ', '.join(missing))
        ENV = {**{k:v for k,v in os.environ.items() if not k.startswith(('DEMO_', 'HTTP_', 'RELAY_', 'CONSUMER_', 'REPLAY_', 'RABBITMQ_')) or k in ('RABBITMQ_HOST','RABBITMQ_PORT','RABBITMQ_DEFAULT_USER','RABBITMQ_DEFAULT_PASS','RABBITMQ_DEFAULT_VHOST')}, 'CARGO_TARGET_DIR': str(ROOT / 'target/tests'),
               'RELAY_TEST_RUN_ID': uuid.uuid4().hex,
               'PLAYWRIGHT_BROWSERS_PATH': str(CACHE / 'browsers'), 'npm_config_cache': str(CACHE / 'npm')}
        try:
            run('Integration prerequisites', ['python3', '-B', str(ROOT / 'scripts/test-resource-cleanup.py'), ENV['RELAY_TEST_RUN_ID']], ROOT)
            frontend = CACHE / 'frontend'
            if frontend.exists():
                for old in frontend.iterdir():
                    if old.name not in ('node_modules', '.installed-lock'):
                        if old.is_dir(): shutil.rmtree(old)
                        else: old.unlink()
            shutil.copytree(ROOT / 'frontend', frontend, dirs_exist_ok=True,
                            ignore=shutil.ignore_patterns('node_modules', 'dist', 'test-results', 'playwright-report'))
            digest = hashlib.sha256((frontend / 'package-lock.json').read_bytes() + (frontend / 'package.json').read_bytes()).hexdigest()
            stamp = frontend / '.installed-lock'
            if not (frontend / 'node_modules/.bin/playwright').exists() or not stamp.exists() or stamp.read_text() != digest:
                run('Prepare locked frontend dependencies', ['npm', 'ci'], frontend)
                stamp.write_text(digest)
            run('Prepare Chromium', ['npx', '--no-install', 'playwright', 'install', 'chromium'], frontend)
            run('Rust formatting', ['cargo', 'fmt', '--check'])
            run('Rust lint', ['cargo', 'clippy', '--locked', '--all-targets', '--all-features', '--', '-D', 'warnings'])
            run('Backend tests', ['cargo', 'test', '--locked'])
            run('Test binaries', ['cargo', 'build', '--locked', '--bins'])
            run('Rust documentation', ['cargo', 'doc', '--locked', '--no-deps'], env={**ENV, 'RUSTDOCFLAGS': '-D warnings'})
            run('Documentation links', ['python3', '-B', 'scripts/check-docs.py'], ROOT)
            run('Whitespace', ['git', '-c', f'safe.directory={ROOT}', 'diff', '--check'], ROOT)
            run('Python tooling', ['python3', '-B', '-m', 'unittest', 'discover', '-s', 'scripts', '-p', 'test_*.py'], ROOT)
            run('Frontend types', ['npm', 'run', 'type-check'], frontend)
            run('Frontend build', ['npm', 'run', 'build'], frontend)
            run('Browser experience', ['npm', 'test'], frontend)
            for target in ('postgres_repository', 'delivery_ownership_schema', 'postgres_ownership', 'postgres_retry', 'postgres_discovery', 'postgres_replay', 'postgres_consumer', 'demo_runs', 'operational_readiness', 'rabbitmq_publisher', 'worker_smoke', 'integrated_recovery', 'consumer_runtime'):
                run(f'Real integration: {target}', ['cargo', 'test', '--locked', '--test', target, '--', '--ignored', '--test-threads=1'])
            for test in ('closed_owned_connection_is_unavailable', 'publisher_admission_is_immediate_and_released_after_error_and_cancellation'):
                run(f'Real broker: {test}', ['cargo', 'test', '--locked', '--lib', test, '--', '--ignored'])
            run('Disposable SQLx migrations', ['python3', '-B', str(ROOT / 'scripts/check-migrations.py'), '--container'], ROOT)
            run('Real scenario browsers', ['python3', '-B', str(ROOT / 'scripts/test-container-browser.py'), str(frontend)], ROOT)
        finally:
            stop_child()
            # A second interrupt cannot bypass scoped cleanup.
            signal.signal(signal.SIGINT, signal.SIG_IGN)
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            cleanup = subprocess.run(['python3', '-B', str(ROOT / 'scripts/test-resource-cleanup.py'), ENV['RELAY_TEST_RUN_ID']], env=ENV, capture_output=True, text=True, timeout=45)
            if cleanup.returncode:
                raise RuntimeError('Owned-resource cleanup failed: ' + sanitize(cleanup.stderr[-2000:]))

        print('\nPASS: all routine phases and cleanup completed. Host acceptance and capacity campaigns are separate.', flush=True)


if __name__ == '__main__':
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    os.umask(0o077)
    try:
        main()
    except KeyboardInterrupt:
        stop_child()
        print(f'\nINTERRUPTED during {PHASE}; completed: {", ".join(PASSED)}', file=sys.stderr)
        sys.exit(130)
    except Exception as error:
        stop_child()
        print(f'\nFAIL during {PHASE}: {sanitize(str(error))}\nCompleted: {", ".join(PASSED) or "none"}', file=sys.stderr)
        sys.exit(1)
