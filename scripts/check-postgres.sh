#!/bin/sh
# Read-only local PostgreSQL diagnostics. No credentials are printed.
set -eu
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"

container_id=$(docker compose ps -q postgres)
if [ -z "$container_id" ]; then
    echo 'PostgreSQL container is not running.' >&2
    exit 1
fi
health=$(docker inspect --format '{{.State.Health.Status}}' "$container_id")
if [ "$health" != healthy ]; then
    echo 'PostgreSQL healthcheck is not healthy.' >&2
    exit 1
fi
publication=$(docker compose port postgres 5432)
case "$publication" in
    127.0.0.1:*) host_port=${publication#127.0.0.1:} ;;
    *) echo 'Expected PostgreSQL publication on IPv4 loopback only.' >&2; exit 1 ;;
esac
printf 'PostgreSQL healthy; publication: %s\n' "$publication"

if command -v nc >/dev/null 2>&1; then
    nc -z -w 3 127.0.0.1 "$host_port"
    echo 'Host loopback TCP connection succeeded.'
elif command -v python3 >/dev/null 2>&1; then
    python3 - "$host_port" <<'PYTHON'
import socket
import sys
with socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=3):
    print("Host loopback TCP connection succeeded.")
PYTHON
else
    echo 'Host TCP probe skipped: install nc or Python 3 to enable it.'
fi

# Compare desired Compose settings to running environments without displaying them.
# JSON stays in a pipe, never in command-line arguments or a source file.
{
    docker compose config --format json
    docker inspect --format '{{json .Config.Env}}' "$container_id"
} | docker compose exec -T app python3 -c '
import json
import os
import sys
raw = sys.stdin.read().lstrip()
decoder = json.JSONDecoder()
config, end = decoder.raw_decode(raw)
running, _ = decoder.raw_decode(raw[end:].lstrip())
running = dict(item.split("=", 1) for item in running)
for service, actual in [("postgres", running), ("app", os.environ)]:
    expected = config["services"][service]["environment"]
    keys = ["POSTGRES_USER", "POSTGRES_PASSWORD", "POSTGRES_DB", "POSTGRES_HOST", "POSTGRES_PORT"]
    if any(str(expected[key]) != actual.get(key) for key in keys):
        sys.exit("Running container settings differ from current Compose/.env configuration.")
print("Running PostgreSQL/app settings match current Compose/.env configuration.")
'

# Capture psql's identifying output; never print the configured role or password.
# -X avoids user psql startup files; every query below is read-only.
docker compose exec -T postgres sh -s <<'CONTAINER'
set -eu
export PGPASSWORD="$POSTGRES_PASSWORD"
if ! identity=$(psql -X -h postgres -p "${POSTGRES_PORT:-5432}" \
    -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
    -At -F '|' -c 'SELECT current_user, current_database();' 2>/dev/null); then
    echo 'Authenticated TCP query failed with the configured credentials. No database changes were made.' >&2
    exit 1
fi
if [ "$identity" != "$POSTGRES_USER|$POSTGRES_DB" ]; then
    echo 'Authenticated identity did not match the configured role/database.' >&2
    exit 1
fi
echo 'Authenticated TCP query: role and database match the current Compose configuration.'
history=$(psql -X -h postgres -p "${POSTGRES_PORT:-5432}" \
    -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
    -At -c "SELECT to_regclass('public._sqlx_migrations') IS NOT NULL;" 2>/dev/null)
if [ "$history" = t ]; then
    psql -X -h postgres -p "${POSTGRES_PORT:-5432}" \
        -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 \
        -c 'SELECT version, success FROM public._sqlx_migrations ORDER BY version;' 2>/dev/null
else
    echo 'No SQLx history table exists yet; diagnostics did not create it.'
fi
CONTAINER
