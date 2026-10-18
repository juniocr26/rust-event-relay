#!/usr/bin/python3
"""Named Supervisor recovery and read-only readiness/topology verification."""
import base64
import json
import os
import subprocess
import time
import urllib.request
from urllib.parse import quote


def supervisor(*args):
    config = ['-c', os.environ['SUPERVISOR_CONFIG']] if os.environ.get('SUPERVISOR_CONFIG') else []
    return ['supervisorctl', *config, *args]


def states():
    result = subprocess.run(supervisor('status'), capture_output=True, text=True, timeout=10)
    rows = {parts[0]: parts[1] for line in result.stdout.splitlines() if len(parts := line.split()) >= 2}
    if set(rows) != {'http', 'relay', 'consumer'}:
        raise ValueError('Cannot read the three named Supervisor processes; inspect supervisorctl status')
    return rows


def control(action, program):
    result = subprocess.run(supervisor(action, program), capture_output=True, timeout=40)
    if result.returncode:
        raise ValueError(f'{program}: {action} failed; inspect docker compose logs --tail 60 app (sources redacted by application)')


def activate_http():
    before = states()
    if before['http'] == 'STOPPED':
        raise ValueError('HTTP intentionally STOPPED; preserved. Token saved but activation pending; start http explicitly when ready')
    for program, state in before.items():
        if state in ('FATAL', 'EXITED'):
            control('start', program)
        elif state == 'BACKOFF':
            raise ValueError(f'{program} is BACKOFF; diagnose startup before rotating again')
    if before['http'] == 'RUNNING':
        control('restart', 'http')
    return before


def verify(before):
    # Stability is observed on the same manager; never restart all or unstop operators.
    for attempt in range(3):
        current = states()
        for program, previous in before.items():
            expected = 'STOPPED' if previous == 'STOPPED' else 'RUNNING'
            if current[program] != expected:
                raise ValueError(f'{program}: expected {expected}, observed {current[program]}; inspect named process logs')
            if expected == 'RUNNING':
                key = {'http':'HTTP_METRICS_ADDR','relay':'RELAY_METRICS_ADDR','consumer':'CONSUMER_METRICS_ADDR'}[program]
                try:
                    with urllib.request.urlopen('http://' + os.environ[key] + '/ready', timeout=3) as response:
                        if response.status != 200: raise ValueError('readiness rejected')
                except Exception:
                    raise ValueError(f'{program}: readiness failed; inspect migrations, dependency readiness and named process logs') from None
        if attempt < 2: time.sleep(1)
    topology()
    stopped = [p for p,s in before.items() if s == 'STOPPED']
    print('Supervisor stability and readiness verified; intentional stops preserved: '+(', '.join(stopped) or 'none')+'. RabbitMQ topology checked separately without publishing.')


def topology():
    token = base64.b64encode((os.environ['RABBITMQ_DEFAULT_USER']+':'+os.environ['RABBITMQ_DEFAULT_PASS']).encode()).decode()
    base = 'http://' + os.environ.get('RABBITMQ_HOST','rabbitmq') + ':15672/api/'
    vhost = quote(os.environ['RABBITMQ_DEFAULT_VHOST'], safe='')
    def api(resource):
        request=urllib.request.Request(base+resource+'/'+vhost, headers={'Authorization':'Basic '+token})
        with urllib.request.urlopen(request,timeout=5) as response: return json.load(response)
    try:
        exchanges,queues,bindings=[api(k) for k in ('exchanges','queues','bindings')]
        for ek,qk,rk in [('RABBITMQ_EXCHANGE','RABBITMQ_QUEUE','RABBITMQ_ROUTING_KEY'), ('CONSUMER_SOURCE_EXCHANGE','CONSUMER_SOURCE_QUEUE','CONSUMER_SOURCE_ROUTING_KEY'), ('CONSUMER_QUARANTINE_EXCHANGE','CONSUMER_QUARANTINE_QUEUE','CONSUMER_QUARANTINE_ROUTING_KEY')]:
            exchange,queue,routing=[os.environ[k] for k in (ek,qk,rk)]
            assert any(e['name']==exchange and e['durable'] and e['type']=='direct' for e in exchanges)
            assert any(q['name']==queue and q['durable'] for q in queues)
            assert any(b['source']==exchange and b['destination']==queue and b['destination_type']=='queue' and b['routing_key']==routing for b in bindings)
    except Exception:
        raise ValueError('RabbitMQ topology verification failed; inspect configured vhost permissions, exchange, queue and bindings. No messages published or purged.') from None
