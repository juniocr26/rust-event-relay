#!/usr/bin/env python3
"""Acceptance from candidate Git-visible files; no ignored files, host artifacts or secrets."""
import hashlib,json,os,pathlib,secrets,shutil,socket,subprocess,tempfile,time,uuid
ROOT=pathlib.Path(__file__).resolve().parent.parent

def main():
 owner='relay-clean-'+uuid.uuid4().hex[:12]
 with tempfile.TemporaryDirectory(prefix=owner+'-') as directory:
  checkout=pathlib.Path(directory)/'checkout';checkout.mkdir()
  files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=ROOT).decode().split('\0')
  hashes={}
  for name in sorted(set(files)-{''}):
   source=ROOT/name
   if not source.is_file():continue
   target=checkout/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
   hashes[name]=hashlib.sha256(source.read_bytes()).hexdigest()
  assert not any((checkout/p).exists() for p in ['.env','.demo.env','target','.cargo-cache','frontend/node_modules','.dockerized-postgres'])
  ports=[]
  for _ in range(4):
   with socket.socket() as probe:
    probe.bind(('127.0.0.1',0));ports.append(probe.getsockname()[1])
  assert len(set(ports))==4
  env=dict(os.environ,COMPOSE_PROJECT_NAME=owner)
  # Do not inherit credentials/configuration from the host environment.
  for key in list(env):
   if key.startswith(('POSTGRES_','RABBITMQ_','DEMO_','HOST_','RELAY_','CONSUMER_','REPLAY_')):env.pop(key)
  content=(checkout/'.env.example').read_text()
  for line in content.splitlines():
   if line and not line.startswith('#') and '=' in line:env.pop(line.split('=',1)[0],None)
  values={'POSTGRES_USER':'relay_clean','POSTGRES_PASSWORD':secrets.token_hex(32),'RABBITMQ_DEFAULT_USER':'relay_clean','RABBITMQ_DEFAULT_PASS':secrets.token_hex(32),'HOST_HTTP_PORT':ports[0],'HOST_FRONTEND_PORT':ports[1],'POSTGRES_HOST_PORT':ports[2],'RABBITMQ_MANAGEMENT_PORT':ports[3]}
  lines=[line.split('=',1)[0]+'='+str(values[line.split('=',1)[0]]) if '=' in line and line.split('=',1)[0] in values else line for line in content.splitlines()]
  path=checkout/'.env';path.write_text('\n'.join(lines)+'\n');path.chmod(0o600)
  cmd=['docker','compose']
  def run(args,**kw):return subprocess.run(args,cwd=checkout,env=env,check=True,timeout=1800,**kw)
  def compose(args,**kw):return run(cmd+args,**kw)
  def sql(query):return compose(['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" -At -v ON_ERROR_STOP=1'],input=query,text=True,capture_output=True).stdout.strip()
  try:
   run(['python3','scripts/start-local.py'])
   assert sql('SELECT count(*) FROM public._sqlx_migrations WHERE success;')=='9'
   # Browser image contributes only browser/system tools, never candidate dependencies/artifacts.
   run(['docker','build','-f','docker/Dockerfile.browser','-t',owner+'-browser','.'],stdout=subprocess.DEVNULL)
   # Rotate via the exact supported container command, capture privately, never report the secret.
   rotated=compose(['exec','-T','app','python3','/app/scripts/demo-token.py','rotate'],capture_output=True,text=True).stdout
   assert any(line.startswith('Activated; previous token rejected.') for line in rotated.splitlines())
   token=next(line.split('=',1)[1] for line in path.read_text().splitlines() if line.startswith('DEMO_TOKEN='))
   network=owner+'_default'
   browser=['docker','run','--rm','--network',network,'--mount','type=bind,src='+str(checkout/'frontend')+',dst=/app/frontend','-w','/app/frontend','--entrypoint','sh',owner+'-browser','-c']
   # A local test proxy preserves the documented localhost Origin, including isolated port.
   proxy="require('net').createServer(s=>{const r=require('net').connect(5173,'frontend');s.pipe(r).pipe(s);s.on('error',()=>r.destroy());r.on('error',()=>s.destroy())}).listen("+str(ports[1])+",'0.0.0.0')"
   # Keep installation sequential before starting the proxy and browser.
   script="npm ci && npm run type-check && npm run build && printf '%s' \"$CLEAN_TEST_TOKEN\" > /tmp/demo61-browser-token && (node -e "+__import__('shlex').quote(proxy)+" & npx playwright test --config playwright.live.config.ts --grep 'real browser →|real (dedupe|concurrent|recovery):|exact loopback|configuration proxy failure')"
   browser=browser[:2]+['--env','CLEAN_TEST_TOKEN','--env','PLAYWRIGHT_BASE_URL']+browser[2:]
   test_env=dict(env,CLEAN_TEST_TOKEN=token,PLAYWRIGHT_BASE_URL='http://localhost:'+str(ports[1]))
   compose(['exec','-T','frontend','npm','run','build'])
   run(['docker','compose','exec','-T','-e','CARGO_TARGET_DIR=/app/target/tests','app','cargo','test','--locked','--lib','--jobs','1'])
   result=subprocess.run(browser+[script],cwd=checkout,env=test_env,capture_output=True,text=True,timeout=300)
   print(result.stdout.replace(token,'[redacted]'));print(result.stderr.replace(token,'[redacted]'))
   if result.returncode:raise RuntimeError('Clean-install browser checks failed; sanitized diagnostics above.')
   for asset in (checkout/'frontend/dist').rglob('*'):
    if asset.is_file():
     data=asset.read_bytes()
     assert all(value.encode() not in data for value in [token,values['POSTGRES_PASSWORD'],values['RABBITMQ_DEFAULT_PASS']])
   total=sql('SELECT count(*) FROM demo.runs;')
   assert int(total)>0
   assert sql("SELECT count(*) FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id JOIN consumer.receipts c ON c.event_id=r.event_id AND c.consumer_name='inventory.stock.v1' JOIN consumer.stock s ON (s.warehouse_id,s.sku)=(r.warehouse_id,r.sku) WHERE o.status='processed' AND c.stock_before=10 AND c.quantity=5 AND c.stock_after=15 AND s.units=15;")==total
   assert sql("SELECT count(*) FROM demo.actions a WHERE state<>'confirmed' OR (SELECT count(*) FROM demo.consumer_observations o WHERE o.action_id=a.action_id AND o.outcome='Duplicate')<a.copies;")=='0'
   print('Independent clean-install SQL verifies every run and correlated action; no secrets in production assets.')
   before=sql("SELECT jsonb_build_object('runs',(SELECT count(*) FROM demo.runs),'actions',(SELECT count(*) FROM demo.actions),'receipts',(SELECT count(*) FROM consumer.receipts),'stock',(SELECT sum(units) FROM consumer.stock));")
   compose(['stop','app','frontend']);compose(['start','app','frontend'])
   for _ in range(60):
    # The entrypoint prepares binaries before the Supervisor socket exists.
    # Poll transient connection/status codes; retain the original bounded budget.
    status=subprocess.run(cmd+['exec','-T','app','supervisorctl','status'],cwd=checkout,env=env,capture_output=True,text=True,timeout=10).stdout
    if status.count('RUNNING')==3:break
    time.sleep(1)
   assert status.count('RUNNING')==3
   assert sql("SELECT jsonb_build_object('runs',(SELECT count(*) FROM demo.runs),'actions',(SELECT count(*) FROM demo.actions),'receipts',(SELECT count(*) FROM consumer.receipts),'stock',(SELECT sum(units) FROM consumer.stock));")==before
   compose(['up','-d','--force-recreate','--no-deps','--wait','app'])
   compose(['exec','-T','app','python3','-c',"import runpy,pathlib; c=runpy.run_path('/app/scripts/demo-token.py'); assert c['status'](c['read_token'](pathlib.Path('/app/.env')))==200; print('Persisted token authorizes after app recreation.')"])
   assert sql("SELECT jsonb_build_object('runs',(SELECT count(*) FROM demo.runs),'actions',(SELECT count(*) FROM demo.actions),'receipts',(SELECT count(*) FROM consumer.receipts),'stock',(SELECT sum(units) FROM consumer.stock));")==before
   report={'candidate_sha256':hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest(),'files':len(hashes),'migrations':9,'persistence':json.loads(before),'result':'passed'}
   pathlib.Path('/tmp/relay-clean-install-result.json').write_text(json.dumps(report,indent=2))
   print(json.dumps(report))
  finally:
   compose(['down','--volumes','--remove-orphans'])
   subprocess.run(['docker','image','rm',owner+'-browser',owner+'-app'],check=False,capture_output=True)
   print('Removed exact disposable clean-install resources; main data preserved.')
if __name__=='__main__':main()
