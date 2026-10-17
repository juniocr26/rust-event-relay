#!/usr/bin/env python3
"""Operate only the fixed retained demo project. Never print secrets or target shared SQL."""
import os
import pathlib
import secrets
import subprocess
import sys
ROOT = pathlib.Path(__file__).resolve().parent.parent
ENV = ROOT / '.demo.env'
COMMAND = ['docker', 'compose', '--env-file', str(ENV), '-p', 'relay-demo61', '-f', 'compose.demo.yaml']
def run(args, **kw):
    return subprocess.run(COMMAND + args, cwd=ROOT, check=True, **kw)
def main():
    action = sys.argv[1] if len(sys.argv) == 2 else 'status'
    if action == 'init':
        if ENV.exists():
            print('Existing .demo.env preserved.'); return
        fd = os.open(ENV, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'w') as f:
            for key in ['DEMO_DB_PASSWORD', 'DEMO_BROKER_PASSWORD', 'DEMO_TOKEN']:
                f.write(f'{key}={secrets.token_hex(32)}\n')
        print('Created private .demo.env. Use DEMO_TOKEN in the local interface; do not share it.')
    elif action == 'build':
        run(['build', 'tools'])
        run(['run', '--rm', '--no-deps', 'tools', 'cargo', 'build', '--locked', '--bins'])
    elif action == 'migrate':
        # Exact container label + database/server fingerprints before SQLx mutation.
        cid = run(['ps', '-q', 'postgres'], capture_output=True, text=True).stdout.strip()
        if not cid: raise RuntimeError('demo PostgreSQL is not running')
        labels = subprocess.run(['docker','inspect','--format','{{index .Config.Labels "com.docker.compose.project"}}/{{index .Config.Labels "com.docker.compose.service"}}',cid],check=True,capture_output=True,text=True).stdout.strip()
        if labels != 'relay-demo61/postgres': raise RuntimeError('wrong Compose target')
        query = ['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U demo61 -d relay_demo61 -At -v ON_ERROR_STOP=1']
        fp = run(query, input="SELECT current_database() || '/' || current_user || '/' || inet_server_port();", capture_output=True,text=True).stdout.strip()
        if fp != 'relay_demo61/demo61/5432': raise RuntimeError('wrong database fingerprint')
        run(['run','--rm','--no-deps','tools','sqlx','migrate','run','--no-dotenv'])
    elif action == 'infra': run(['up','-d','postgres','rabbitmq'])
    elif action == 'start': run(['up','-d','demo','relay','consumer','frontend'])
    elif action == 'stop': run(['stop'])
    elif action == 'status': run(['ps'])
    else: raise SystemExit('Usage: demo-stack.py init|infra|build|migrate|start|stop|status')
if __name__ == '__main__': main()
