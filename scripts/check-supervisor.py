#!/usr/bin/env python3
"""Explicit local-development control regression after authorized schema rollout.
Stops/starts managed children and injects unexpected exits; never publishes,
reads message bodies, purges queues, changes credentials or runs migrations.
"""
import base64
import json
import os
import pathlib
import stat
import subprocess
import time
import urllib.error
import urllib.request
from urllib.parse import quote


def ctl(command, *args):
    result = subprocess.run([command, *args], capture_output=True, text=True, timeout=40)
    # supervisorctl status returns 3 for intentionally stopped children.
    assert result.returncode == 0 or (args[0] == "status" and result.returncode == 3), result.stdout
    return result.stdout


def health():
    try:
        with urllib.request.urlopen("http://127.0.0.1:8080/health", timeout=1) as response:
            return response.status == 200 and response.read() == b"ok\n"
    except (urllib.error.URLError, OSError):
        return False


def ready(previous_pid=None, target="http"):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        status = subprocess.run(["supervisorctl", "status", target], capture_output=True, text=True, timeout=2)
        pid = int(ctl("supervisorctl", "pid", target).strip())
        if "RUNNING" in status.stdout and (target != "http" or health()) and (previous_pid is None or previous_pid != pid):
            return pid
        time.sleep(0.05)
    raise AssertionError(f"{target} child did not become ready within deadline")


def broker_snapshot():
    """Authenticated non-consuming observation; do not infer payload persistence."""
    user = os.environ["RABBITMQ_DEFAULT_USER"]
    password = os.environ["RABBITMQ_DEFAULT_PASS"]
    vhost = quote(os.environ.get("RABBITMQ_DEFAULT_VHOST", "relay"), safe="")
    queue = quote(os.environ.get("RABBITMQ_QUEUE", "relay.local"), safe="")
    token = base64.b64encode(f"{user}:{password}".encode()).decode()
    request = urllib.request.Request(f"http://rabbitmq:15672/api/queues/{vhost}/{queue}",
        headers={"Authorization": "Basic " + token})
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            q = json.load(response)
    except (urllib.error.URLError, OSError, ValueError):
        raise AssertionError("Authenticated broker management unavailable") from None
    assert q["durable"]
    return {key: q.get(key) for key in ["name", "vhost", "durable", "messages_ready", "messages_unacknowledged"]}


def stopped(target):
    # Exceed startsecs to prove an intentional stop does not autorestart.
    time.sleep(2)
    assert "STOPPED" in ctl("supervisorctl", "status", target)
    if target == "relay":
        assert health(), "Stopping relay must preserve HTTP"
        assert broker_snapshot() == broker_before, "Broker topology/counts changed while relay stopped"


socket = pathlib.Path("/tmp/relay-supervisor/control.sock")
assert stat.S_IMODE(socket.stat().st_mode) == 0o600
assert socket.stat().st_uid == os.getuid()
manager_pid = pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text()
ready()
ready(target="relay")
broker_before = broker_snapshot()
for command in ["supervisorctl", "supervisor"]:
    assert "http" in ctl(command, "status")
    for target in ["http", "relay"]:
        ctl(command, "stop", target)
        if target == "http":
            assert not health()
        # supervisord remains reachable while a managed child is stopped.
        stopped(target)
        assert pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text() == manager_pid
        ctl(command, "start", target)
        ready(target=target)
    old = ready()
    ctl(command, "restart", "http")
    ready(old)
    console = subprocess.run([command], input="status\nstop http\nstatus\nstart http\nrestart http\nstop relay\nstatus\nstart relay\nrestart relay\nstatus\nquit\n", capture_output=True, text=True, timeout=40)
    assert console.returncode == 0
    assert "STOPPED" in console.stdout and "RUNNING" in console.stdout
    assert not any(marker in console.stdout for marker in ["ERROR", "unknown", "no such", "not recognized"])
    ready()
    ready(target="relay")
    print(f"{command}: direct and interactive http/relay control passed")
for target in ["http", "relay"]:
    old = ready(target=target)
    os.kill(old, 9)
    ready(old, target=target)
    assert pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text() == manager_pid
    print(f"Unexpected {target} exit restarted; manager PID/private socket preserved.")
assert health()
assert "RUNNING" in ctl("supervisorctl", "status", "relay")
print("Both children RUNNING; relay stop preserved authenticated broker management, durable queue and observed message counts.")
print("No demonstration messages published; empty queues cannot prove nonempty payload persistence.")
