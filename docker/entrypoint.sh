#!/bin/sh
set -eu
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"
mkdir -p /tmp/relay-supervisor
chmod 700 /tmp/relay-supervisor
exec "$@"
