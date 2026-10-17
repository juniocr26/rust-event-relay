#!/usr/bin/env python3
"""Run bounded opt-in evidence against owned disposable service instances.

No shared service exec, stop, credentials or published ports. See milestone-5-3.
"""
import argparse
import base64
import os
import urllib.request
import json
import pathlib
import shutil
import subprocess
import time
import uuid

ROOT = pathlib.Path(__file__).resolve().parent.parent


def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, timeout=30, **kwargs)


def output(args):
    return run(args, capture_output=True, text=True).stdout.strip()


def observed_campaign():
    """Bounded management observer inside the test-owned runner namespace."""
    process = subprocess.Popen(["cargo", "test", "--locked", "--test", "capacity_measurements", "--", "--ignored", "--nocapture", "--test-threads=1"])
    start = time.monotonic()
    samples = failed = max_ready = max_unacked = 0
    token = base64.b64encode((os.environ["RABBITMQ_DEFAULT_USER"] + ":" + os.environ["RABBITMQ_DEFAULT_PASS"]).encode()).decode()
    try:
        while process.poll() is None:
            if time.monotonic() - start > 600:
                raise TimeoutError("Finite campaign exceeded 600s")
            try:
                request = urllib.request.Request("http://measure-rabbit:15672/api/queues/relay", headers={"Authorization": "Basic " + token})
                with urllib.request.urlopen(request, timeout=1) as response:
                    body = response.read(131073)
                assert len(body) <= 131072, "Bounded management response exceeded"
                queues = json.loads(body)
                if queues and all(isinstance(q.get("messages_ready"), int) and isinstance(q.get("messages_unacknowledged"), int) for q in queues):
                    samples += 1
                    max_ready = max(max_ready, sum(q["messages_ready"] for q in queues))
                    max_unacked = max(max_unacked, sum(q["messages_unacknowledged"] for q in queues))
            except (OSError, ValueError):
                failed += 1
            time.sleep(0.2)
        print("BROKER_OBSERVER", json.dumps({"samples": samples, "failed": failed, "max_ready": max_ready if samples else None, "max_unacknowledged": max_unacked if samples else None, "scope": "dedicated broker, all fixture queues; cached management statistics"}), flush=True)
        assert process.returncode == 0, "Capacity campaign failed"
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("suite", choices=["campaign", "verification", "observe-campaign"])
    args = parser.parse_args()
    if args.suite == "observe-campaign":
        observed_campaign()
        return
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3, "Need 3 GiB free before finite run"
    image = output(["docker", "compose", "images", "-q", "app"])
    assert image and "\n" not in image, "Build the reviewed app image first"
    owner = "relay-m53-" + uuid.uuid4().hex
    network = owner + "-net"
    pg, rabbit, runner = (owner + suffix for suffix in ["-pg", "-rabbit", "-runner"])
    created = []
    network_created = False
    try:
        run(["docker", "network", "create", "--label", "relay.measure.owner=" + owner, network], stdout=subprocess.DEVNULL)
        network_created = True
        for name, alias, service_image, memory, tmpfs, env in [
            (pg, "measure-pg", "postgres:18.6-bookworm", "512m", "/var/lib/postgresql:rw,size=256m", {
                "POSTGRES_USER": "relay_test", "POSTGRES_PASSWORD": "disposable_measurement_only", "POSTGRES_DB": "relay_test_admin"}),
            (rabbit, "measure-rabbit", "rabbitmq:4.3.6-management", "768m", "/var/lib/rabbitmq:rw,size=256m", {
                "RABBITMQ_DEFAULT_USER": "relay_test", "RABBITMQ_DEFAULT_PASS": "disposable_measurement_only", "RABBITMQ_DEFAULT_VHOST": "relay", "RABBITMQ_SERVER_ADDITIONAL_ERL_ARGS": "+S 2:2"}),
        ]:
            command = ["docker", "run", "-d", "--name", name, "--label", "relay.measure.owner=" + owner,
                       "--network", network, "--network-alias", alias, "--cpus", "1", "--memory", memory,
                       "--memory-swap", memory, "--tmpfs", tmpfs, "--log-opt", "max-size=2m", "--log-opt", "max-file=1"]
            for key, value in env.items():
                command += ["-e", key + "=" + value]
            run(command + [service_image], stdout=subprocess.DEVNULL)
            created.append(name)
        deadline = time.monotonic() + 100
        for name, probe in [(pg, ["pg_isready", "-U", "relay_test", "-d", "relay_test_admin"]),
                            (rabbit, ["gosu", "rabbitmq", "rabbitmq-diagnostics", "-q", "check_running"])]:
            while True:
                ready = subprocess.run(["docker", "exec", name, *probe], capture_output=True, timeout=15)
                if ready.returncode == 0:
                    break
                assert time.monotonic() < deadline, "Dedicated service startup exceeded 100s"
                time.sleep(1)
        commands = {
            "campaign": ["python3 /app/scripts/measure-isolated.py observe-campaign"],
            "verification": [
                "cargo fmt --check",
                "cargo clippy --locked --all-targets --all-features -- -D warnings",
                "cargo test --locked",
                "cargo test --locked --test demo_runs --test postgres_repository --test delivery_ownership_schema --test postgres_ownership --test postgres_retry --test postgres_discovery --test postgres_replay --test postgres_consumer --test operational_readiness --test rabbitmq_publisher --test integrated_recovery --test consumer_runtime --test worker_smoke -- --ignored --test-threads=1",
                "cargo test --locked --lib infrastructure::rabbitmq::tests:: -- --ignored --test-threads=1",
                "cargo build --locked --bins",
                "RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps",
            ],
        }
        # Fixed commands only, fail at the first failed check. No secrets in output.
        script = "set -eu\n" + "\n".join(commands[args.suite])
        command = ["docker", "run", "--rm", "--name", runner, "--label", "relay.measure.owner=" + owner,
                   "--network", network, "--cpus", "2", "--memory", "2g", "--memory-swap", "2g",
                   "--log-opt", "max-size=2m", "--log-opt", "max-file=1",
                   "--mount", "type=bind,src=" + str(ROOT) + ",dst=/app", "-w", "/app/backend", "--entrypoint", "/bin/sh"]
        env = {"POSTGRES_HOST": "measure-pg", "POSTGRES_PORT": "5432", "POSTGRES_USER": "relay_test",
               "POSTGRES_PASSWORD": "disposable_measurement_only", "POSTGRES_DB": "relay_test_admin",
               "RABBITMQ_HOST": "measure-rabbit", "RABBITMQ_PORT": "5672", "RABBITMQ_DEFAULT_USER": "relay_test",
               "RABBITMQ_DEFAULT_PASS": "disposable_measurement_only", "RABBITMQ_DEFAULT_VHOST": "relay",
               "CARGO_HOME": "/app/.cargo-cache", "CARGO_TARGET_DIR": "/app/target/m52", "REPLAY_OPERATORS": "",
               "RELAY_MAINTENANCE_CANDIDATES": "", "RELAY_DEDICATED_MEASUREMENTS": "1", "RUST_LOG": "error" if args.suite == "campaign" else "info"}
        for key, value in env.items():
            command += ["-e", key + "=" + value]
        created.append(runner)
        print("Owned disposable resources:", owner, "suite:", args.suite, flush=True)
        subprocess.run(command + [image, "-c", script], cwd=ROOT, check=True, timeout=1500)
        # Assert harness cleanup before removing the containing services.
        dbs = output(["docker", "exec", pg, "psql", "-U", "relay_test", "-d", "relay_test_admin", "-Atqc",
                      "SELECT count(*) FROM pg_database WHERE datname LIKE 'relay_it_%'"])
        assert dbs == "0", "Test-owned database remains"
        queues = json.loads(output(["docker", "exec", rabbit, "rabbitmqctl", "-q", "list_queues", "-p", "relay", "name", "--formatter", "json"]))
        assert not queues, "Test-owned broker queue remains"
        print("No fixture databases or queues remain before service removal.", flush=True)
    finally:
        for name in reversed(created):
            result = subprocess.run(["docker", "inspect", "--format", '{{index .Config.Labels "relay.measure.owner"}}', name], capture_output=True, text=True, timeout=15)
            if result.returncode == 0:
                assert result.stdout.strip() == owner, "Ownership mismatch: refusing removal"
                run(["docker", "rm", "-f", "-v", name], stdout=subprocess.DEVNULL)
        if network_created:
            run(["docker", "network", "rm", network], stdout=subprocess.DEVNULL)
        remaining = output(["docker", "ps", "-aq", "--filter", "label=relay.measure.owner=" + owner])
        assert not remaining, "Owned container remains"
        print("Exact owned containers, anonymous volumes and network removed:", owner, flush=True)


if __name__ == "__main__":
    main()
