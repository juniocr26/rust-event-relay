#!/usr/bin/python3
"""Fallback cleanup for exactly one runner namespace, including interruption."""
import base64
import json
import os
import re
import sys
import urllib.request
from urllib.parse import quote
import psycopg2
from psycopg2 import sql


def cleanup(run_id):
    if not re.fullmatch('[0-9a-f]{32}',run_id): raise ValueError('invalid cleanup namespace')
    errors=[]
    try:
        connection=psycopg2.connect(host=os.environ['POSTGRES_HOST'],port=os.environ['POSTGRES_PORT'],user=os.environ['POSTGRES_USER'],password=os.environ['POSTGRES_PASSWORD'],dbname=os.environ['POSTGRES_DB'],connect_timeout=5)
        connection.autocommit=True
        with connection.cursor() as cur:
            cur.execute('SELECT datname FROM pg_database WHERE starts_with(datname,%s)',('relay_it_'+run_id+'_',))
            for (name,) in cur.fetchall():
                # Child processes are already reaped. Never terminate unrelated DB connections.
                cur.execute(sql.SQL('DROP DATABASE {}').format(sql.Identifier(name)))
        connection.close()
    except Exception: errors.append('PostgreSQL cleanup failed for relay_it_'+run_id+'_')
    try:
        auth=base64.b64encode((os.environ['RABBITMQ_DEFAULT_USER']+':'+os.environ['RABBITMQ_DEFAULT_PASS']).encode()).decode()
        base='http://'+os.environ.get('RABBITMQ_HOST','rabbitmq')+':15672/api/'
        vhost=quote(os.environ['RABBITMQ_DEFAULT_VHOST'],safe='')
        def api(path,method='GET'):
            request=urllib.request.Request(base+path,method=method,headers={'Authorization':'Basic '+auth})
            with urllib.request.urlopen(request,timeout=5) as response: return json.load(response) if method=='GET' else None
        for kind in ('queues','exchanges'):
            for item in api(kind+'/'+vhost):
                if item['name'].startswith('relay.it.'+run_id+'.'):
                    api(kind+'/'+vhost+'/'+quote(item['name'],safe=''),'DELETE')
    except Exception: errors.append('RabbitMQ cleanup failed for relay.it.'+run_id+'.')
    if errors: raise RuntimeError('; '.join(errors))

if __name__=='__main__': cleanup(sys.argv[1])
