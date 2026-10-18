#!/usr/bin/env python3
"""Disposable copy of MAIN Compose, including its actual Supervisor composition."""
import fcntl,json,os,pathlib,secrets,subprocess,tempfile,time,uuid
ROOT=pathlib.Path(__file__).resolve().parent.parent
PROJECT='relay-application-test'
def main():
 os.umask(0o077)
 with open('/tmp/relay-application-test.lock','w') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
  campaign()
def campaign():
 subprocess.run(['docker','build','-f','docker/Dockerfile.browser','-t','relay-application-browser','.'],cwd=ROOT,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=600)
 config=json.loads(subprocess.check_output(['docker','compose','config','--format','json'],cwd=ROOT))
 config['name']=PROJECT;config.pop('networks',None)
 secret=secrets.token_hex(32);password=secrets.token_hex(32)
 for service in config['services'].values():
  service.pop('ports',None);service.pop('networks',None);service.pop('build',None)
 app=config['services']['app'];app['image']='rust-event-relay-app'
 app['entrypoint']=['/bin/sh','-ec','mkdir -p /tmp/relay-supervisor; chmod 700 /tmp/relay-supervisor; exec "$@"','test-entrypoint'];app['command']=['/usr/bin/supervisord','-c','/etc/supervisor/supervisord.conf']
 app['environment'].update({'DEMO_ORIGIN':'http://localhost:5173','POSTGRES_USER':'relay_test','POSTGRES_PASSWORD':password,'POSTGRES_DB':'relay_test','RABBITMQ_DEFAULT_USER':'relay_test','RABBITMQ_DEFAULT_PASS':password,'RABBITMQ_DEFAULT_VHOST':'relay','REPLAY_OPERATORS':'','DEMO_ENABLED':'true'})
 app['volumes']=[{'type':'bind','source':str(ROOT),'target':'/app','read_only':True},{'type':'bind','source':str(ROOT/'target'),'target':'/app/target','read_only':True}]
 app.update({'mem_limit':'1g','cpus':2,'pids_limit':256})
 db=config['services']['postgres'];db['environment'].update({'POSTGRES_USER':'relay_test','POSTGRES_PASSWORD':password,'POSTGRES_DB':'relay_test'});db['volumes']=[{'type':'volume','source':'test-postgres','target':'/var/lib/postgresql'}]
 broker=config['services']['rabbitmq'];broker['environment'].update({'RABBITMQ_DEFAULT_USER':'relay_test','RABBITMQ_DEFAULT_PASS':password,'RABBITMQ_DEFAULT_VHOST':'relay','RABBITMQ_SERVER_ADDITIONAL_ERL_ARGS':'+S 2:2'});broker['volumes']=[v for v in broker['volumes'] if v.get('type')=='bind']+[{'type':'volume','source':'test-rabbitmq','target':'/var/lib/rabbitmq'}]
 frontend=config['services']['frontend'];frontend.update({'image':'relay-application-browser','user':'root','mem_limit':'2g','cpus':2,'pids_limit':512});frontend['volumes']=[{'type':'bind','source':str(ROOT/'frontend'),'target':'/app/frontend','read_only':True},{'type':'volume','source':'test-node','target':'/app/frontend/node_modules'}]
 config['volumes']={key:{'name':PROJECT+'_'+key} for key in ['test-postgres','test-rabbitmq','test-node']}
 with tempfile.TemporaryDirectory(prefix='relay-application-test-') as directory:
  private=pathlib.Path(directory)/'.env';private.write_text('DEMO_TOKEN='+secret+'\n');private.chmod(0o600)
  app['volumes'].append({'type':'bind','source':str(private),'target':'/app/.env','read_only':True})
  path=pathlib.Path(directory)/'compose.json';path.write_text(json.dumps(config));path.chmod(0o600)
  command=['docker','compose','-p',PROJECT,'-f',str(path)]
  def run(args,**kw):
   if 'playwright' in args:
    result=subprocess.run(command+args,cwd=ROOT,capture_output=True,text=True,timeout=180,**kw)
    print(result.stdout.replace(secret,'[redacted]'));print(result.stderr.replace(secret,'[redacted]'))
    if result.returncode:raise RuntimeError('Browser check failed; sanitized diagnostics above.')
    return result
   return subprocess.run(command+args,cwd=ROOT,check=True,timeout=180,**kw)
  def sql(query):return run(['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U relay_test -d relay_test -At -v ON_ERROR_STOP=1'],input=query,text=True,capture_output=True).stdout.strip()
  try:
   run(['up','-d','--wait','postgres','rabbitmq'])
   cid=run(['ps','-q','postgres'],capture_output=True,text=True).stdout.strip()
   label=subprocess.check_output(['docker','inspect','--format','{{index .Config.Labels "com.docker.compose.project"}}',cid],text=True).strip()
   assert label==PROJECT and sql('SELECT current_database();')=='relay_test'
   run(['run','--rm','--no-deps','app','sqlx','migrate','run','--no-dotenv'])
   run(['up','-d','--wait','app','frontend'])
   for _ in range(60):
    status=run(['exec','-T','app','supervisorctl','status'],capture_output=True,text=True).stdout
    if status.count('RUNNING')==3:break
    time.sleep(1)
   assert status.count('RUNNING')==3
   run(['exec','-T','frontend','sh','-c','for attempt in $(seq 1 60); do test -x node_modules/.bin/playwright && break; sleep 1; done; test -x node_modules/.bin/playwright'])
   run(['exec','-T','frontend','sh','-c','cat > /tmp/demo61-browser-token'],input=secret,text=True)
   run(['exec','-T','app','supervisorctl','stop','consumer'])
   run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep','paused test consumer'])
   pending=json.loads(run(['exec','-T','frontend','cat','/tmp/demo61-pending-evidence.json'],capture_output=True,text=True).stdout)
   run(['exec','-T','app','supervisorctl','start','consumer'])
   for _ in range(20):
    result=sql(f"SELECT stock_after FROM consumer.receipts WHERE event_id='{uuid.UUID(pending['event_id'])}';")
    if result=='15':break
    time.sleep(1)
   assert result=='15'
   run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep-invert','dependency unavailable|paused test consumer|unknown action inspection'])
   evidence=json.loads(run(['exec','-T','frontend','cat','/tmp/demo61-browser-evidence.json'],capture_output=True,text=True).stdout)
   assert '|processed|10|5|15|15' in sql(f"SELECT r.run_id,o.status,c.stock_before,c.quantity,c.stock_after,s.units FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id JOIN consumer.receipts c ON c.event_id=r.event_id JOIN consumer.stock s ON s.warehouse_id=r.warehouse_id AND s.sku=r.sku WHERE r.run_id='{uuid.UUID(evidence['run_id'])}';")
   assert sql("SELECT count(*) FROM demo.actions a WHERE state='confirmed' AND (SELECT count(*) FROM demo.consumer_observations o WHERE o.action_id=a.action_id AND o.outcome='Duplicate')>=a.copies;")=='7'
   assert sql("SELECT count(*) FROM demo.faults WHERE injected_at IS NOT NULL AND retry_available_at IS NOT NULL;")=='1'
   queues=json.loads(run(['exec','-T','rabbitmq','rabbitmqctl','-q','list_queues','-p','relay','name','consumers','messages_ready','messages_unacknowledged','--formatter','json'],capture_output=True,text=True).stdout)
   action_queues=[q for q in queues if q['name'].startswith('relay.demo.')]
   assert len(action_queues)==7 and all(q['consumers']==0 and q['messages_ready']==0 and q['messages_unacknowledged']==0 for q in action_queues)
   print('Independent SQL verifies normal, duplicate, concurrent duplicate and controlled retry evidence.')
   retained=sql("SELECT (SELECT count(*) FROM demo.runs),(SELECT count(*) FROM demo.actions),(SELECT count(*) FROM consumer.receipts);")
   run(['up','-d','--no-deps','--force-recreate','--wait','app'])
   assert run(['exec','-T','app','supervisorctl','status'],capture_output=True,text=True).stdout.count('RUNNING')==3
   assert sql("SELECT (SELECT count(*) FROM demo.runs),(SELECT count(*) FROM demo.actions),(SELECT count(*) FROM consumer.receipts);")==retained
   print('App container recreation preserves existing real runs/actions/receipts and restores all three roles.')
   # Test-owned fixture: retained unknown state, not a claim of a newly injected crash.
   sql("UPDATE demo.actions SET state='unknown' WHERE action_id=(SELECT action_id FROM demo.actions ORDER BY accepted_at DESC LIMIT 1);")
   run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep','unknown action inspection'])
   run(['cp','frontend:/tmp/demo63-unknown.png','/tmp/relay-main-unknown.png'])

   run(['cp','frontend:/tmp/demo62-historical.png','/tmp/relay-main-historical.png'])
   run(['cp','frontend:/tmp/demo63-stale.png','/tmp/relay-main-stale.png'])
   for width in [320,768,1920]:run(['cp',f'frontend:/tmp/demo63-history-{width}.png',f'/tmp/relay-main-history-{width}.png'])
   for width in [320,1920]:run(['cp',f'frontend:/tmp/demo61-live-{width}.png',f'/tmp/relay-main-live-{width}.png'])
   run(['exec','-T','app','supervisorctl','stop','http'])
   run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep','dependency unavailable'])
   # Intentional stop stays stopped under the existing manager; other roles stay running.
   time.sleep(2)
   stopped=subprocess.run(command+['exec','-T','app','supervisorctl','status'],cwd=ROOT,capture_output=True,text=True).stdout
   assert 'STOPPED' in stopped and stopped.count('RUNNING')==2
   run(['exec','-T','app','supervisorctl','start','http'])
   print('Main Supervisor autostart, intentional stop and named restart verified in disposable environment.')
  except BaseException:
   diagnostics=run(['logs','--tail','60','rabbitmq','app'],capture_output=True,text=True).stdout
   pathlib.Path('/tmp/relay-application-test-diagnostics.txt').write_text(diagnostics)
   print('Test-only diagnostics saved privately to /tmp/relay-application-test-diagnostics.txt')
   raise
  finally:
   run(['down','--volumes','--remove-orphans'],stdout=subprocess.DEVNULL)
   print('Exact test resources removed; main and legacy user evidence untouched.')
if __name__=='__main__':main()
