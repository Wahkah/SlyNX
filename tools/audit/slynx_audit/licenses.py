"""Dependency licenses: `cargo deny check licenses` against deny.toml, when cargo-deny is installed."""
from __future__ import annotations

import os
import shutil
import subprocess

from .common import FAIL, SKIP, CheckResult, tomllib

INSTALL_HINT = 'cargo install --locked cargo-deny'


def cargo_deny_command() -> list | None:
    exe = shutil.which('cargo-deny')
    if exe:
        return [exe]
    if shutil.which('cargo'):
        try:
            if subprocess.run(['cargo', 'deny', '--version'], capture_output=True, timeout=60).returncode == 0:
                return ['cargo', 'deny']
        except (OSError, subprocess.TimeoutExpired):
            pass
    return None


def has_crates(manifest: str) -> bool:
    """False for a workspace with no members and no root package (nothing for cargo-deny to check)."""
    if tomllib is None:
        return True
    try:
        with open(manifest, 'rb') as f:
            doc = tomllib.load(f)
    except (OSError, ValueError):
        return True
    return 'package' in doc or bool(doc.get('workspace', {}).get('members'))


def run(repo: str, files: list, command=None) -> CheckResult:
    result = CheckResult('licenses')
    manifest = os.path.join(repo, 'Cargo.toml')
    if not os.path.exists(manifest):
        result.status, result.summary = SKIP, 'no Cargo.toml'
        return result
    if not has_crates(manifest):
        result.status, result.summary = SKIP, 'the workspace has no crates yet'
        return result
    cmd = command if command is not None else cargo_deny_command()
    if not cmd:
        result.status, result.summary = SKIP, f'cargo-deny is not installed (install: {INSTALL_HINT})'
        return result
    full = cmd + ['--manifest-path', manifest, 'check', '--config', os.path.join(repo, 'deny.toml'), 'licenses']
    try:
        proc = subprocess.run(full, capture_output=True, text=True, cwd=repo, timeout=900)
    except (OSError, subprocess.TimeoutExpired) as e:
        result.add(FAIL, 'deny.toml', f'cargo deny could not run: {e}', key='error')
        return result.finish()
    if proc.returncode != 0:
        tail = (proc.stdout + proc.stderr).strip().splitlines()[-40:]
        result.add(FAIL, 'deny.toml', 'cargo deny check licenses failed', key='licenses', details=tail)
    result.summary = 'cargo deny check licenses: ' + ('ok' if proc.returncode == 0 else 'failed')
    return result.finish()
