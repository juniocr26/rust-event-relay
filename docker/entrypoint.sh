#!/bin/sh
set -eu
mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"
exec "$@"
