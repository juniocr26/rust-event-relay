#!/usr/bin/env python3
"""Run inside app: real lifecycle checks for the managed HTTP child only."""
import os
import pathlib
import stat
import subprocess
import time
import urllib.error
import urllib.request


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


def ready(previous_pid=None):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        status = subprocess.run(["supervisorctl", "status", "http"], capture_output=True, text=True, timeout=2)
        pid = int(ctl("supervisorctl", "pid", "http").strip())
        if "RUNNING" in status.stdout and health() and (previous_pid is None or previous_pid != pid):
            return pid
        time.sleep(0.05)
    raise AssertionError("HTTP child did not become healthy within deadline")


socket = pathlib.Path("/tmp/relay-supervisor/control.sock")
assert stat.S_IMODE(socket.stat().st_mode) == 0o600
assert socket.stat().st_uid == os.getuid()
manager_pid = pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text()
ready()
for command in ["supervisorctl", "supervisor"]:
    assert "http" in ctl(command, "status")
    for target in ["all", "http"]:
        ctl(command, "stop", target)
        assert not health()
        # supervisord remains reachable while every managed child is stopped.
        assert "STOPPED" in ctl(command, "status", "http")
        assert pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text() == manager_pid
        ctl(command, "start", target)
        ready()
    old = ready()
    ctl(command, "restart", "http")
    ready(old)
    console = subprocess.run([command], input="status\nstop all\nstatus\nstart all\nstop http\nstart http\nrestart http\nstatus\nquit\n", capture_output=True, text=True, timeout=40)
    assert console.returncode == 0
    assert "STOPPED" in console.stdout and "RUNNING" in console.stdout
    assert not any(marker in console.stdout for marker in ["ERROR", "unknown", "no such", "not recognized"])
    ready()
    print(f"{command}: direct and interactive status, collective and named control passed")
old = ready()
os.kill(old, 9)
ready(old)
assert pathlib.Path("/tmp/relay-supervisor/supervisord.pid").read_text() == manager_pid
print("Unexpected child exit restarted; HTTP recovered; supervisord PID and private socket preserved.")
