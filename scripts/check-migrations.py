#!/usr/bin/env python3
"""Validate SQLx history and upgrades on an exact disposable database only.

Requires the existing Compose app/PostgreSQL services and CREATE DATABASE.
Never migrates the shared database, prints driver output or reads private rows.
"""
import hashlib
import pathlib
import subprocess
import uuid


ROOT = pathlib.Path(__file__).resolve().parent.parent


def run(args, sql=None):
    result = subprocess.run(
        ["docker", "compose", *args], cwd=ROOT, input=sql,
        text=True, capture_output=True, timeout=60,
    )
    if result.returncode:
        raise RuntimeError("isolated migration operation failed; driver output redacted")
    return result.stdout.strip()


def query(database, sql):
    return run([
        "exec", "-T", "postgres", "sh", "-c",
        'PGPASSWORD="$POSTGRES_PASSWORD" PGCONNECT_TIMEOUT=5 '
        'PGOPTIONS="-c statement_timeout=10000 -c lock_timeout=3000" '
        'psql -X -w -h postgres -U "$POSTGRES_USER" -d "$1" '
        '-v ON_ERROR_STOP=1 -At -F "|"', "migration-check", database,
    ], sql)


def admin(sql):
    return run([
        "exec", "-T", "postgres", "sh", "-c",
        'PGPASSWORD="$POSTGRES_PASSWORD" PGCONNECT_TIMEOUT=5 '
        'PGOPTIONS="-c statement_timeout=10000 -c lock_timeout=3000" '
        'psql -X -w -h postgres -U "$POSTGRES_USER" -d "$POSTGRES_DB" '
        '-v ON_ERROR_STOP=1 -At',
    ], sql)


def validate():
    # The only mutation target is generated here, never supplied by a caller.
    name = "relay_migration_" + uuid.uuid4().hex
    files = sorted((ROOT / "backend/migrations").glob("*.up.sql"))
    expected = {str(int(p.name.split("_", 1)[0])): hashlib.sha384(p.read_bytes()).hexdigest() for p in files}
    assert len(files) == 5, "review acceptance when the migration inventory changes"
    admin(f'CREATE DATABASE "{name}";')
    try:
        def migrate(*args):
            run(["exec", "-T", "-e", f"POSTGRES_DB={name}", "app", "sqlx", "migrate", *args, "--no-dotenv"])

        # Real SQLx CLI ledger from empty schema, then supported earlier schemas.
        for version in list(expected):
            migrate("run", "--target-version", version)
            rows = query(name, "SELECT version,success,encode(checksum,'hex') FROM public._sqlx_migrations ORDER BY version;")
            applied = [row.split("|") for row in rows.splitlines()]
            assert [r[0] for r in applied] == [v for v in expected if int(v) <= int(version)]
            assert all(success == "t" and checksum == expected[v] for v, success, checksum in applied)
        migrate("run")  # No pending migrations; checksum validation still runs.
        assert query(name, "SELECT to_regclass('relay.outbox_events') IS NOT NULL AND to_regclass('relay.replay_requests') IS NOT NULL;") == "t"
        migrate("revert")
        assert query(name, "SELECT to_regclass('relay.replay_requests') IS NULL;") == "t"
        assert query(name, "SELECT count(*) FROM public._sqlx_migrations;") == "4"
        migrate("run")
        assert query(name, "SELECT count(*) FROM public._sqlx_migrations WHERE success;") == "5"
        print("Five ordered SQLx upgrades/checksums, no-op, replay down/re-up passed in isolated database.")
    finally:
        # No test connection remains after each CLI invocation. Never use FORCE.
        admin(f'DROP DATABASE "{name}";')
        assert admin(f"SELECT count(*) FROM pg_database WHERE datname='{name}';") == "0"
        print("Exact isolated migration database removed and absence verified.")


if __name__ == "__main__":
    validate()
