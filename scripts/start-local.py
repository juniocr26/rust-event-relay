#!/usr/bin/env python3
"""Detect a conflicting management port before starting Compose; never change the port."""
import json
import socket
import subprocess
import sys

config = json.loads(subprocess.check_output(["docker", "compose", "config", "--format", "json"]))
port = int(config["services"]["rabbitmq"]["ports"][0]["published"])
running = subprocess.check_output(["docker", "compose", "ps", "-q", "rabbitmq"], text=True).strip()
if running:
    current = subprocess.check_output(["docker", "compose", "port", "rabbitmq", "15672"], text=True).strip()
    if current == f"127.0.0.1:{port}":
        subprocess.run(["docker", "compose", "up", "-d", "--wait"], check=True)
        sys.exit(0)
try:
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", port))
except OSError:
    sys.exit(f"Host management port {port} is occupied. Free it before starting; the requested port was not changed.")
subprocess.run(["docker", "compose", "up", "-d", "--wait"], check=True)
