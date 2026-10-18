#!/usr/bin/python3
"""Run existing real scenario specs against invocation-owned DB/topology/processes."""
import base64
import json
import os
from pathlib import Path
import secrets
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from urllib.parse import quote
import uuid
import psycopg2
from psycopg2 import sql

ROOT = Path(__file__).resolve().parents[1]
children = []


def interrupt(signum, frame):
    raise KeyboardInterrupt


def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def main():
    name = 'relay_browser_' + uuid.uuid4().hex
    prefix = 'relay.browser.' + uuid.uuid4().hex
    token = secrets.token_hex(32)
    reservations = [socket.socket() for _ in range(5)]
    for reservation in reservations: reservation.bind(('127.0.0.1', 0))
    http, vite, hm, rm, cm = [reservation.getsockname()[1] for reservation in reservations]
    for reservation in reservations: reservation.close()
    environment = {k: v for k, v in os.environ.items() if k in ('PATH', 'HOME', 'CARGO_TARGET_DIR', 'PLAYWRIGHT_BROWSERS_PATH', 'npm_config_cache') or k.startswith('POSTGRES_') or k.startswith('RABBITMQ_DEFAULT_') or k in ('RABBITMQ_HOST', 'RABBITMQ_PORT')}
    environment.update(POSTGRES_DB=name, DEMO_ENABLED='true', DEMO_TOKEN=token,
        DEMO_ORIGIN=f'http://localhost:{vite}', HTTP_ADDR=f'127.0.0.1:{http}',
        HTTP_METRICS_ADDR=f'127.0.0.1:{hm}', RELAY_METRICS_ADDR=f'127.0.0.1:{rm}', CONSUMER_METRICS_ADDR=f'127.0.0.1:{cm}',
        RABBITMQ_EXCHANGE=prefix+'.events', RABBITMQ_QUEUE=prefix+'.queue', RABBITMQ_ROUTING_KEY='event',
        CONSUMER_SOURCE_EXCHANGE=prefix+'.events', CONSUMER_SOURCE_QUEUE=prefix+'.queue', CONSUMER_SOURCE_ROUTING_KEY='event',
        CONSUMER_QUARANTINE_EXCHANGE=prefix+'.quarantine', CONSUMER_QUARANTINE_QUEUE=prefix+'.quarantine.queue', CONSUMER_QUARANTINE_ROUTING_KEY='inventory',
        DEMO_API_TARGET=f'http://127.0.0.1:{http}', PLAYWRIGHT_BASE_URL=f'http://127.0.0.1:{vite}')
    admin = psycopg2.connect(host=os.environ['POSTGRES_HOST'], port=os.environ['POSTGRES_PORT'], user=os.environ['POSTGRES_USER'], password=os.environ['POSTGRES_PASSWORD'], dbname=os.environ['POSTGRES_DB'], connect_timeout=5)
    admin.autocommit = True
    frontend = Path(sys.argv[1])
    token_path = Path('/tmp/demo61-browser-token')
    errors = []
    with tempfile.TemporaryDirectory(prefix='relay-browser-') as directory:
        logs = []
        with admin.cursor() as cur:
            cur.execute(sql.SQL('CREATE DATABASE {}').format(sql.Identifier(name)))
        try:
            subprocess.run(['sqlx', 'migrate', 'run', '--no-dotenv'], cwd=ROOT/'backend', env=environment, check=True, capture_output=True)
            target = Path(environment['CARGO_TARGET_DIR'])/'debug'
            private = Path(directory)
            (private / 'scripts').mkdir()
            for script in ('demo-token.py', 'service-check.py'):
                shutil.copy2(ROOT / 'scripts' / script, private / 'scripts' / script)
            (private / '.env').write_text('DEMO_TOKEN=' + token + '\n')
            config = (ROOT / 'docker/supervisor/normal.conf').read_text()
            config = config.replace('/tmp/relay-supervisor', str(private / 'supervisor'))
            config = config.replace('directory=/app/backend', 'directory=' + directory)
            config = config.replace('command=/usr/local/bin/relay-http', 'command=/usr/bin/python3 ' + str(private / 'scripts/demo-token.py') + ' launch')
            config = config.replace('command=/usr/local/bin/relay-worker', 'command=' + str(target / 'relay'))
            config = config.replace('command=/usr/local/bin/relay-consumer', 'command=' + str(target / 'consumer'))
            role = None
            lines = []
            for line in config.splitlines():
                if line.startswith('[program:'): role = line.removeprefix('[program:').removesuffix(']')
                if role and line.startswith('stdout_logfile='): line = 'stdout_logfile=' + str(private / (role + '.out'))
                if role and line.startswith('stderr_logfile='): line = 'stderr_logfile=' + str(private / (role + '.err'))
                lines.append(line)
            config = '\n'.join(lines) + '\n'
            (private / 'supervisor').mkdir()
            conf = private / 'supervisord.conf'
            conf.write_text(config)
            environment['SUPERVISOR_CONFIG'] = str(conf)
            commands = [(['/usr/bin/supervisord', '-c', str(conf)], private),
                        (['npm','run','dev','--','--mode','acceptance','--host','127.0.0.1','--port',str(vite),'--strictPort'], frontend)]
            for command, working in commands:
                log = tempfile.TemporaryFile(mode='w+')
                logs.append(log)
                children.append(subprocess.Popen(command, cwd=working, env=environment, stdout=log, stderr=log))
            for attempt in range(60):
                try:
                    for endpoint in [f'http://127.0.0.1:{p}/ready' for p in (hm,rm,cm)]+[f'http://127.0.0.1:{vite}/']:
                        urllib.request.urlopen(endpoint, timeout=2).close()
                    break
                except OSError:
                    supervisor_status = subprocess.run(['supervisorctl','-c',str(conf),'status'], capture_output=True, text=True, timeout=5).stdout
                    if any(p.poll() is not None for p in children) or 'FATAL' in supervisor_status or attempt==59:
                        raise RuntimeError('Isolated browser services failed readiness')
                    time.sleep(1)
            token_path.write_text(token); token_path.chmod(0o600)
            def browser(*selection):
                command = ['npx','--no-install','playwright','test','--config','playwright.live.config.ts', *selection]
                result = subprocess.run(command, cwd=frontend, env=environment, capture_output=True, text=True)
                print((result.stdout + result.stderr).replace(token, '[redacted]'))
                if result.returncode: raise RuntimeError('Real scenario browser checks failed; sanitized diagnostics above')
            def ctl(action, role):
                subprocess.run(['supervisorctl','-c',str(conf),action,role],check=True,capture_output=True,timeout=40)
            def database(query, params=()):
                db = psycopg2.connect(host=os.environ['POSTGRES_HOST'],port=os.environ['POSTGRES_PORT'],user=os.environ['POSTGRES_USER'],password=os.environ['POSTGRES_PASSWORD'],dbname=name)
                try:
                    with db.cursor() as cur:
                        cur.execute(query,params)
                        rows = cur.fetchall() if cur.description else []
                    db.commit()
                    return rows
                finally: db.close()
            ctl('stop', 'consumer')
            browser('--grep', 'paused test consumer')
            pending = json.loads(Path('/tmp/demo61-pending-evidence.json').read_text())
            ctl('start', 'consumer')
            for attempt in range(20):
                if database('SELECT stock_after FROM consumer.receipts WHERE event_id=%s', (pending['event_id'],)) == [(15,)]: break
                if attempt == 19: raise RuntimeError('Pending browser fixture did not recover after named consumer start')
                time.sleep(1)
            browser('live-delivery.spec.ts', 'real-scenarios.spec.ts', '--grep-invert', 'unknown action inspection')
            database("UPDATE demo.actions SET state='unknown' WHERE action_id=(SELECT action_id FROM demo.actions ORDER BY accepted_at DESC LIMIT 1)")
            browser('--grep', 'unknown action inspection')
            ctl('stop', 'http')
            browser('--grep', 'dependency unavailable')
            ctl('start', 'http')
            def business_counts():
                db = psycopg2.connect(host=os.environ['POSTGRES_HOST'], port=os.environ['POSTGRES_PORT'], user=os.environ['POSTGRES_USER'], password=os.environ['POSTGRES_PASSWORD'], dbname=name)
                try:
                    with db.cursor() as cur:
                        cur.execute('SELECT (SELECT count(*) FROM demo.runs),(SELECT count(*) FROM demo.actions),(SELECT count(*) FROM relay.outbox_events),(SELECT count(*) FROM consumer.receipts)')
                        return cur.fetchone()
                finally: db.close()
            original_counts = business_counts()
            # Exercise real token activation and service-check failure on this manager only.
            rotate = ['python3', '-B', str(private / 'scripts/demo-token.py'), 'rotate']
            def rotation(env):
                result = subprocess.run(rotate, env=env, capture_output=True, text=True, timeout=50)
                output = re.sub(r'[0-9a-fA-F]{64}', '[redacted]', result.stdout + result.stderr)
                print(output)
                return result, output
            successful, _ = rotation(environment)
            if successful.returncode: raise RuntimeError('Isolated token activation failed')
            failed, diagnostics = rotation({**environment, 'RELAY_METRICS_ADDR': f'127.0.0.1:{port()}'})
            if failed.returncode == 0 or 'Rotation already applied' not in diagnostics:
                raise RuntimeError('Expected activated-token/service-failure boundary was not reported')
            subprocess.run(['supervisorctl','-c',str(conf),'stop','consumer'], check=True, capture_output=True)
            stopped, _ = rotation(environment)
            if stopped.returncode: raise RuntimeError('Rotation did not preserve intentional consumer stop')
            current = subprocess.run(['supervisorctl','-c',str(conf),'status'],capture_output=True,text=True).stdout
            if 'STOPPED' not in current or current.count('RUNNING') != 2:
                raise RuntimeError('Intentional consumer stop was changed')
            if business_counts() != original_counts: raise RuntimeError('Token rotation created business records')
            print('Three isolated Supervisor roles, token activation, applied-before-failure diagnostics and intentional stop verified.')
        finally:
            # Prevent a second SIGINT from bypassing owned-resource cleanup.
            signal.signal(signal.SIGINT, signal.SIG_IGN)
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            for child in children:
                if child.poll() is None:
                    child.terminate()
            for child in children:
                try: child.wait(timeout=30)
                except subprocess.TimeoutExpired: child.kill(); child.wait()
            token_path.unlink(missing_ok=True)
            if sys.exc_info()[0]:
                for path in sorted(Path(directory).glob('*.err')) + sorted(Path(directory).glob('*.out')):
                    text = path.read_text()[-3000:]
                    for secret in (token, os.environ['POSTGRES_PASSWORD'], os.environ['RABBITMQ_DEFAULT_PASS']): text = text.replace(secret, '[redacted]')
                    print(path.name + ':\n' + re.sub(r'[0-9a-fA-F]{64}', '[redacted]', text), file=sys.stderr)
            for log in logs:
                log.seek(0)
                # Application diagnostics already redact sources; additionally hide token/passwords.
                text = log.read()
                for secret in (token, os.environ['POSTGRES_PASSWORD'], os.environ['RABBITMQ_DEFAULT_PASS']): text=text.replace(secret,'[redacted]')
                if sys.exc_info()[0]: print(re.sub(r'[0-9a-fA-F]{64}', '[redacted]', text[-4000:]), file=sys.stderr)
                log.close()
            try:
                auth = base64.b64encode((os.environ['RABBITMQ_DEFAULT_USER']+':'+os.environ['RABBITMQ_DEFAULT_PASS']).encode()).decode()
                base = 'http://'+os.environ.get('RABBITMQ_HOST','rabbitmq')+':15672/api/'
                vhost = quote(os.environ['RABBITMQ_DEFAULT_VHOST'],safe='')
                def api(path, method='GET'):
                    request=urllib.request.Request(base+path,method=method,headers={'Authorization':'Basic '+auth})
                    with urllib.request.urlopen(request,timeout=5) as response:
                        return json.load(response) if method=='GET' else None
                # Demo actions use relay.demo.<action UUID>. Collect only IDs from this DB.
                own=psycopg2.connect(host=os.environ['POSTGRES_HOST'],port=os.environ['POSTGRES_PORT'],user=os.environ['POSTGRES_USER'],password=os.environ['POSTGRES_PASSWORD'],dbname=name)
                with own.cursor() as cur:
                    cur.execute('SELECT action_id FROM demo.actions'); actions=[str(r[0]) for r in cur.fetchall()]
                own.close()
                for kind in ('queues','exchanges'):
                    for item in api(kind+'/'+vhost):
                        resource=item['name']
                        if resource.startswith(prefix+'.') or any(a in resource for a in actions):
                            api(kind+'/'+vhost+'/'+quote(resource,safe=''),'DELETE')
            except Exception:
                errors.append('Broker cleanup failed: inspect exact prefix '+prefix+' and action IDs from '+name)
            try:
                with admin.cursor() as cur: cur.execute(sql.SQL('DROP DATABASE {}').format(sql.Identifier(name)))
            except Exception: errors.append('Database cleanup failed: '+name)
            admin.close()
            if errors: raise RuntimeError('; '.join(errors))
            print('Isolated browser processes, database and broker resources removed.')


if __name__=='__main__':
    signal.signal(signal.SIGINT,interrupt); signal.signal(signal.SIGTERM,interrupt)
    main()
