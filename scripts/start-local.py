#!/usr/bin/env python3
"""One main application startup: validate, build, verified migrations, Compose/Supervisor."""
import json,os,pathlib,re,secrets,socket,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parent.parent
os.chdir(ROOT)
def run(*args):subprocess.run(args,check=True)
def initialize_token():
 path=ROOT/'.env'
 if not path.exists():raise SystemExit('Create private .env from .env.example and configure infrastructure credentials first.')
 content=path.read_text();matches=list(re.finditer(r'^DEMO_TOKEN=(.*)$',content,re.M))
 if len(matches)>1:raise SystemExit('Expected one authoritative DEMO_TOKEN in .env; values redacted.')
 value=matches[-1].group(1).strip().strip('"\'') if matches else ''
 if not value:
  content=re.sub(r'^DEMO_TOKEN=.*\n?','',content,flags=re.M)+'\nDEMO_TOKEN='+secrets.token_hex(32)+'\n'
  path.write_text(content);path.chmod(0o600)
 elif not re.fullmatch('[0-9a-fA-F]{64}',value):raise SystemExit('DEMO_TOKEN must be a private 64-character hex token; value redacted.')
def resume_failed_processes():
 # A checkout can leave an existing manager FATAL before binaries exist.
 status=subprocess.run(['docker','compose','exec','-T','app','supervisorctl','status'],text=True,capture_output=True).stdout
 for line in status.splitlines():
  fields=line.split()
  if len(fields)>1 and fields[1] in ('FATAL','BACKOFF','EXITED'):
   if fields[1]=='BACKOFF':run('docker','compose','exec','-T','app','supervisorctl','stop',fields[0])
   run('docker','compose','exec','-T','app','supervisorctl','start',fields[0])
def main():
 initialize_token()
 config=json.loads(subprocess.check_output(['docker','compose','config','--format','json']))
 required=['POSTGRES_HOST','POSTGRES_PORT','POSTGRES_DB','POSTGRES_USER','POSTGRES_PASSWORD','RABBITMQ_DEFAULT_USER','RABBITMQ_DEFAULT_PASS','RABBITMQ_DEFAULT_VHOST']
 environment=config['services']['app']['environment']
 missing=[key for key in required if not environment.get(key) or environment[key]=='change_me']
 if missing:raise SystemExit('Configure required values in .env (values redacted): '+', '.join(missing))
 # Never move the established ports to work around another application.
 for service,target in [('rabbitmq',15672),('postgres',5432),('frontend',5173),('app',8080)]:
  ports=config['services'][service].get('ports',[])
  for binding in ports:
   port=int(binding['published'])
   running=subprocess.check_output(['docker','compose','ps','-q',service],text=True).strip()
   if running:continue
   try:
    with socket.socket() as probe:probe.bind(('127.0.0.1',port))
   except OSError:raise SystemExit(f'Configured host port {port} is occupied. Free it; no alternate port was selected.')
 run('docker','compose','up','-d','--wait','postgres','rabbitmq')
 run('docker','compose','build','app')
 run('docker','compose','run','--rm','--no-deps','-e','CARGO_TARGET_DIR=/app/target/runtime','app','cargo','build','--locked','--bins')
 run(sys.executable,'scripts/consolidate-storage.py','--apply')
 run('docker','compose','up','-d','app','frontend')
 resume_failed_processes()
 run('docker','compose','up','-d','--wait','app','frontend')
 print('Main application available. See docs/en/operations/application-scenarios.md. Intentional stops under an unchanged manager remain respected.')
if __name__=='__main__':main()
