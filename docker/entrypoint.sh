#!/bin/sh
set -eu
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"
mkdir -p /tmp/relay-supervisor
chmod 700 /tmp/relay-supervisor
if [ "${1:-}" = /usr/bin/supervisord ]; then
  python3 /app/scripts/prepare-runtime.py
fi
exec "$@"
