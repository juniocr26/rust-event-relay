#!/usr/bin/env python3
"""Read-only host management validation. Never prints credentials or API response bodies."""
import argparse
import base64
import json
import subprocess
import sys
import urllib.error
import urllib.request
from urllib.parse import quote

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--running-credentials", action="store_true", help="Use existing broker container initialization credentials without modifying .env")
parser.add_argument("--require-topology", action="store_true", help="Require the configured durable publisher exchange, queue and binding")
args = parser.parse_args()

try:
    config = json.loads(subprocess.check_output(["docker", "compose", "config", "--format", "json"]))
    env = config["services"]["rabbitmq"]["environment"]
    if args.running_credentials:
        container = subprocess.check_output(["docker", "compose", "ps", "-q", "rabbitmq"], text=True).strip()
        running = dict(item.split("=", 1) for item in json.loads(subprocess.check_output([
            "docker", "inspect", "--format", "{{json .Config.Env}}", container
        ])))
        env = {key: running[key] for key in ["RABBITMQ_DEFAULT_USER", "RABBITMQ_DEFAULT_PASS", "RABBITMQ_DEFAULT_VHOST"]}
        print("Using existing running broker initialization credentials; .env remains unchanged.")
    publication = subprocess.check_output(["docker", "compose", "port", "rabbitmq", "15672"], text=True).strip()
    if not publication.startswith("127.0.0.1:"):
        sys.exit("Management must be published on host loopback.")
    port = publication.split(":")[1]
    assert port == "15672", "Expected management port 15672"
    base = f"http://localhost:{port}"
    with urllib.request.urlopen(base + "/", timeout=5) as response:
        assert response.status == 200 and b"RabbitMQ" in response.read()
    print(f"Host management login page reachable at {base}/")
    token = base64.b64encode((env["RABBITMQ_DEFAULT_USER"] + ":" + env["RABBITMQ_DEFAULT_PASS"]).encode()).decode()
    def api(path):
        request = urllib.request.Request(base + path, headers={"Authorization": "Basic " + token})
        with urllib.request.urlopen(request, timeout=5) as response:
            return json.load(response)
    who = api("/api/whoami")
    assert who["name"] == env["RABBITMQ_DEFAULT_USER"]
    assert "management" in who["tags"] or "administrator" in who["tags"]
    assert any(item["name"] == env["RABBITMQ_DEFAULT_VHOST"] for item in api("/api/vhosts"))
    # Listing permission records requires an administrator tag; keep this dev user
    # management-only and inspect its scoped permissions through the local node CLI.
    permissions = json.loads(subprocess.check_output([
        "docker", "compose", "exec", "-T", "rabbitmq", "sh", "-c",
        'rabbitmqctl -q list_user_permissions "$RABBITMQ_DEFAULT_USER" --formatter json'
    ]))
    own = next(p for p in permissions if p["vhost"] == env["RABBITMQ_DEFAULT_VHOST"])
    assert all(own[key] == "^relay[.].*" for key in ["configure", "write", "read"])
    vhost = quote(env["RABBITMQ_DEFAULT_VHOST"], safe="")
    topology = {resource: api(f"/api/{resource}/{vhost}") for resource in ["exchanges", "queues", "bindings"]}
    assert all(isinstance(items, list) for items in topology.values())
    if args.require_topology:
        app_env = config["services"]["app"]["environment"]
        assert app_env["RABBITMQ_DEFAULT_VHOST"] == env["RABBITMQ_DEFAULT_VHOST"]
        exchange = app_env["RABBITMQ_EXCHANGE"]
        queue = app_env["RABBITMQ_QUEUE"]
        routing_key = app_env["RABBITMQ_ROUTING_KEY"]
        ex = next(e for e in topology["exchanges"] if e["name"] == exchange)
        q = next(q for q in topology["queues"] if q["name"] == queue)
        assert ex["durable"] and ex["type"] == "direct" and q["durable"]
        assert any(b["source"] == exchange and b["destination"] == queue and b["destination_type"] == "queue" and b["routing_key"] == routing_key for b in topology["bindings"])
        print("Configured durable direct exchange, durable queue and binding verified in the configured virtual host.")
        if all(isinstance(q.get(key), int) for key in ["messages_ready", "messages_unacknowledged"]):
            print(f"Observed queue counts: ready={q['messages_ready']}, unacknowledged={q['messages_unacknowledged']}; no messages consumed or published.")
    else:
        print("Authenticated topology listings verified; absent application topology is allowed before publisher connection.")
    print("Authenticated management identity, tag and scoped project-vhost permissions verified.")
    print("HTTP/API checks passed. Browser form login was not performed by this script.")
except (AssertionError, KeyError, StopIteration, ValueError):
    sys.exit("Management identity/permissions mismatch; inspect existing broker configuration. No data was reset.")
except urllib.error.HTTPError as error:
    sys.exit(f"Management API returned HTTP {error.code}. Existing-volume credentials may differ from .env; no data was reset.")
except (urllib.error.URLError, OSError, subprocess.CalledProcessError):
    sys.exit("Management endpoint unavailable. Check Docker health and host port conflicts; default remains 15672.")
