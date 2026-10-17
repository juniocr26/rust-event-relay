#!/usr/bin/env python3
"""Protected backup/restore proof before the authorized main forward migrations."""
import hashlib,json,os,pathlib,subprocess,datetime,sys,uuid
ROOT=pathlib.Path(__file__).resolve().parent.parent
MAIN=['docker','compose']
LEGACY=['docker','compose','--env-file','.demo.env','-p','relay-demo61','-f','compose.demo.yaml']
def run(base,args,data=None):
 r=subprocess.run(base+args,cwd=ROOT,input=data,capture_output=True,timeout=120)
 if r.returncode:raise RuntimeError('storage operation failed; private diagnostics redacted')
 return r.stdout
def sql(base,database,query):
 return run(base,['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U "$POSTGRES_USER" -d "$1" -At -v ON_ERROR_STOP=1','storage',database],query.encode()).decode().strip()
def main():
 os.umask(0o077)
 target=ROOT/'.local-backups'/('consolidation-'+datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ'))
 target.mkdir(parents=True,mode=0o700)
 fingerprints={}
 targets=[(MAIN,'main',os.environ.get('COMPOSE_PROJECT_NAME','rust-event-relay'))]
 if '--apply' not in sys.argv and (ROOT/'compose.demo.yaml').exists(): targets.append((LEGACY,'legacy','relay-demo61'))
 for base,label,expected in targets:
  config=json.loads(run(base,['config','--format','json']))
  assert config['name']==expected
  cid=run(base,['ps','-q','postgres']).decode().strip()
  metadata=json.loads(subprocess.check_output(['docker','inspect',cid]))[0]
  assert metadata['Config']['Labels']['com.docker.compose.project']==expected
  database=config['services']['postgres']['environment']['POSTGRES_DB']
  assert sql(base,database,'SELECT current_database();')==database
  has_history=sql(base,database,"SELECT to_regclass('public._sqlx_migrations') IS NOT NULL;")=='t'
  history=sql(base,database,"SELECT version,encode(checksum,'hex') FROM public._sqlx_migrations ORDER BY version;") if has_history else ''
  for row in history.splitlines():
   version,digest=row.split('|');file=next((ROOT/'backend/migrations').glob(version+'_*.up.sql'))
   assert hashlib.sha384(file.read_bytes()).hexdigest()==digest
  versions=[line.split('|')[0] for line in history.splitlines()]
  expected_versions=[p.name.split('_')[0] for p in sorted((ROOT/'backend/migrations').glob('*.up.sql'))]
  assert versions == expected_versions[:len(versions)], 'non-prefix migration history'
  if '--apply' in sys.argv and versions==expected_versions:
   print('All main migration versions/checksums already current; no migration required.'); return
  data=run(base,['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" pg_dump -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc'])
  (target/(label+'.dump')).write_bytes(data)
  snapshot="SELECT jsonb_build_object('events',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM relay.outbox_events t),'receipts',(SELECT jsonb_agg(to_jsonb(t) ORDER BY event_id) FROM consumer.receipts t),'stock',(SELECT jsonb_agg(to_jsonb(t) ORDER BY warehouse_id,sku) FROM consumer.stock t));"
  if '20261014000000' not in versions: snapshot="SELECT jsonb_build_object('pre_consumer_schema',true);"
  original=sql(base,database,snapshot)
  fingerprints[label]={'database':database,'dump_sha256':hashlib.sha256(data).hexdigest(),'evidence_sha256':hashlib.sha256(original.encode()).hexdigest(),'versions':history,'container':cid}
  (target/(label+'-evidence.json')).write_text(original)
  if label=='main':
   restore='relay_restore_'+uuid.uuid4().hex
   sql(base,database,f'CREATE DATABASE "{restore}";')
   try:
    run(base,['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" pg_restore -h postgres -U "$POSTGRES_USER" -d "$1" --exit-on-error','restore',restore],data)
    assert sql(base,restore,snapshot)==original
    if has_history: assert sql(base,restore,"SELECT version,encode(checksum,'hex') FROM public._sqlx_migrations ORDER BY version;")==history
   finally:sql(base,database,f'DROP DATABASE "{restore}";')
   assert sql(base,database,f"SELECT count(*) FROM pg_database WHERE datname='{restore}';")=='0'
 print('Protected backups and main disposable restore verified.')
 (target/'manifest.json').write_text(json.dumps(fingerprints,indent=2))
 print('Backup directory:',target.relative_to(ROOT))
 if '--apply' in sys.argv:
  run(MAIN,['run','--rm','--no-deps','app','sqlx','migrate','run','--no-dotenv'])
  database=fingerprints['main']['database']
  result=sql(MAIN,database,"SELECT version,encode(checksum,'hex') FROM public._sqlx_migrations ORDER BY version;")
  assert [line.split('|')[0] for line in result.splitlines()]==[p.name.split('_')[0] for p in sorted((ROOT/'backend/migrations').glob('*.up.sql'))]
  for line in result.splitlines():
   version,digest=line.split('|');assert hashlib.sha384(next((ROOT/'backend/migrations').glob(version+'_*.up.sql')).read_bytes()).hexdigest()==digest
  print('Required forward migrations applied; all resulting versions/checksums verified.')
if __name__=='__main__':main()
