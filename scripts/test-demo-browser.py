#!/usr/bin/env python3
"""Build a separate test stack from the reviewed demo config; exact project cleanup only.

Browser runs use real APIs and real binaries; retained operator demo volumes are never removed.
"""
import json
import pathlib
import secrets
import subprocess
import tempfile
ROOT = pathlib.Path(__file__).resolve().parent.parent
PROJECT = 'relay-demo61-browser-test'
def main():
    subprocess.run(['docker','build','-f','docker/Dockerfile.browser','-t','relay-demo61-browser','.'],cwd=ROOT,check=True,timeout=600,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    base = ['docker','compose','--env-file','.demo.env','-p','relay-demo61','-f','compose.demo.yaml']
    config = json.loads(subprocess.run(base+['--profile','tools','config','--format','json'],cwd=ROOT,check=True,capture_output=True,text=True).stdout)
    config['name'] = PROJECT
    config['services']['frontend'].update({'image':'relay-demo61-browser','mem_limit':'2g','cpus':2,'pids_limit':512})
    config.pop('networks',None)
    for name,service in config['services'].items():
        service.pop('ports',None); service.pop('networks',None); service.pop('profiles',None)
    credentials = {k:secrets.token_hex(32) for k in ['POSTGRES_PASSWORD','RABBITMQ_DEFAULT_PASS','DEMO_TOKEN']}
    for service in config['services'].values():
        for key,value in credentials.items():
            if key in service.get('environment',{}):service['environment'][key]=value
    # Binaries and dependency caches are reused read-only. SQL/broker/node evidence is test-owned.
    config['volumes']['demo-build']={'external':True,'name':'relay-demo61_demo-build'}
    config['volumes']['demo-cargo']={'external':True,'name':'relay-demo61_demo-cargo'}
    for key in ['demo-postgres','demo-rabbitmq','demo-node']:
        config['volumes'][key]={'name':PROJECT+'_'+key}
    # Only the explicit migration one-off and frontend/browser can write test artifacts.
    for name in ['demo','relay','consumer']:
        config['services'][name]['volumes']=[v for v in config['services'][name]['volumes'] if v.get('target')!='/app']
    with tempfile.TemporaryDirectory(prefix='demo61-browser-') as directory:
        path=pathlib.Path(directory)/'compose.json';path.write_text(json.dumps(config));path.chmod(0o600)
        cmd=['docker','compose','-p',PROJECT,'-f',str(path)]
        def run(args,**kw):return subprocess.run(cmd+args,cwd=ROOT,check=True,timeout=180,**kw)
        try:
            run(['up','-d','--wait','postgres','rabbitmq'])
            # Verify labels and the exact dedicated test database before migration.
            cid=run(['ps','-q','postgres'],capture_output=True,text=True).stdout.strip()
            label=subprocess.run(['docker','inspect','--format','{{index .Config.Labels "com.docker.compose.project"}}',cid],check=True,capture_output=True,text=True).stdout.strip()
            if label!=PROJECT:raise RuntimeError('wrong test project')
            fingerprint=run(['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U demo61 -d relay_demo61 -Atqc "SELECT current_database()"'],capture_output=True,text=True).stdout.strip()
            if fingerprint!='relay_demo61':raise RuntimeError('wrong test database')
            run(['run','--rm','--no-deps','tools','sqlx','migrate','run','--no-dotenv'])
            run(['up','-d','demo','relay','consumer','frontend'])
            # Bounded frontend startup; browser dependencies were baked into a test-only image.
            run(['exec','-T','frontend','sh','-c','for attempt in $(seq 1 60); do test -x node_modules/.bin/playwright && break; sleep 1; done; test -x node_modules/.bin/playwright'])
            run(['exec','-T','frontend','sh','-c','cat > /tmp/demo61-browser-token'],input=credentials['DEMO_TOKEN'],text=True)
            run(['stop','consumer'])
            run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep','paused test consumer'])
            pending=json.loads(run(['exec','-T','frontend','cat','/tmp/demo61-pending-evidence.json'],capture_output=True,text=True).stdout)
            run(['start','consumer'])
            # Consumer restart must finish the durable run even after the modal closed.
            import time
            for attempt in range(20):
                result=run(['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U demo61 -d relay_demo61 -At'],input=f"SELECT stock_after FROM consumer.receipts WHERE event_id='{pending['event_id']}';",capture_output=True,text=True).stdout.strip()
                if result=='15': break
                time.sleep(1)
            assert result=='15', 'closed browser run did not complete durably'
            run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep-invert','dependency unavailable|paused test consumer'])
            evidence=run(['exec','-T','frontend','cat','/tmp/demo61-browser-evidence.json'],capture_output=True,text=True).stdout
            e=json.loads(evidence)
            # Independent SQL verification, no API projection as sole oracle.
            sql=f"BEGIN READ ONLY; SELECT r.run_id,o.status,c.stock_before,c.quantity,c.stock_after,s.units FROM demo.runs r JOIN relay.outbox_events o ON o.id=r.event_id JOIN consumer.receipts c ON c.event_id=r.event_id JOIN consumer.stock s ON s.warehouse_id=r.warehouse_id AND s.sku=r.sku WHERE r.run_id='{e['run_id']}'; COMMIT;"
            result=run(['exec','-T','postgres','sh','-c','PGPASSWORD="$POSTGRES_PASSWORD" psql -X -h postgres -U demo61 -d relay_demo61 -At -v ON_ERROR_STOP=1'],input=sql,text=True,capture_output=True).stdout
            assert '|processed|10|5|15|15' in result
            print('Independent browser-run SQL receipt/stock verified.')
            for width in [320,1920]:
                run(['cp',f'frontend:/tmp/demo61-live-{width}.png',f'/tmp/demo61-live-{width}.png'])
            run(['stop','demo'])
            run(['exec','-T','frontend','npx','playwright','test','--config','playwright.live.config.ts','--grep','dependency unavailable'])
        finally:
            run(['down','--volumes','--remove-orphans'],stdout=subprocess.DEVNULL)
            print('Exact browser-test project removed; retained demo untouched.')
if __name__=='__main__':main()
