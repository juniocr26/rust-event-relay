#!/usr/bin/env python3
"""Prepare service artifacts before starting the manager; never modifies the DB."""
import fcntl
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
target = ROOT / 'target/runtime'
target.mkdir(parents=True, exist_ok=True)
with (target / '.prepare.lock').open('w') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    subprocess.run(['cargo', 'build', '--locked', '--bins'], cwd=ROOT / 'backend',
                   env={**os.environ, 'CARGO_TARGET_DIR': str(target)}, check=True)
