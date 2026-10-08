#!/usr/bin/env python3
"""Read-only host management validation. Never prints credentials or API response bodies."""
import base64
import json
import subprocess
import sys
import urllib.error
import urllib.request

try:
    config = json.loads(subprocess.check_output(["docker", "compose", "config", "--format", "json"]))
    env = config["services"]["rabbitmq"]["environment"]
    publication = subprocess.check_output(["docker", "compose", "port", "rabbitmq", "15672"], text=True).strip()
    if not publication.startswith("127.0.0.1:"):
        sys.exit("Management must be published on host loopback.")
    port = publication.split(":")[1]
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
    print("Authenticated management identity, tag and scoped project-vhost permissions verified.")
    print("HTTP/API checks passed. Browser form login was not performed by this script.")
except (AssertionError, KeyError, StopIteration, ValueError):
    sys.exit("Management identity/permissions mismatch; inspect existing broker configuration. No data was reset.")
except urllib.error.HTTPError as error:
    sys.exit(f"Management API returned HTTP {error.code}. Existing-volume credentials may differ from .env; no data was reset.")
except (urllib.error.URLError, OSError, subprocess.CalledProcessError):
    sys.exit("Management endpoint unavailable. Check Docker health and host port conflicts; default remains 15672.")
