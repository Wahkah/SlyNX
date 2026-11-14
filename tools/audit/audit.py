"""SlyNX release audit. Run before every push:

    python tools/audit/audit.py [--repo PATH] [--only CHECKS] [--refs DIR ...] [--strict] [--verbose]

Checks (see docs/release-audit.md):
  data        no disc data, game files, packaged builds, keys or large binaries (working tree and git history)
  similarity  our code against the reference sources (structure fingerprints; shared literals are reported)
  notices     every crate in Cargo.lock is listed in THIRD_PARTY_NOTICES.md
  licenses    cargo deny check licenses (skipped when cargo-deny is not installed)
  extra       optional extra check modules named in SLYNX_AUDIT_EXTRA or `git config audit.extraCheck`

Exit code: 0 = nothing blocks the push, 1 = something does (or, with --strict, a check was skipped),
2 = the audit itself could not run (bad allow-list, bad arguments).
"""
from __future__ import annotations

import argparse
import importlib.util
import os
import subprocess
import sys

sys.dont_write_bytecode = True  # no __pycache__ folders left in the tree to be published
HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from slynx_audit import data_check, licenses, notices, similarity  # noqa: E402
from slynx_audit.common import (FAIL, INFO, PASS, SKIP, WARN, AllowListError, CheckResult,  # noqa: E402
                                apply_allowlist, load_allowlist, publishable_files)

CHECKS = ('data', 'similarity', 'notices', 'licenses', 'extra')


def extra_modules(repo: str) -> list:
    """Paths of optional extra check modules (each defines run(repo, files) -> CheckResult)."""
    raw = [p for p in os.environ.get('SLYNX_AUDIT_EXTRA', '').split(os.pathsep) if p.strip()]
    if not raw:
        try:
            out = subprocess.run(['git', '-C', repo, 'config', '--get-all', 'audit.extraCheck'],
                                 capture_output=True, text=True).stdout
            raw = [p.strip() for p in out.splitlines() if p.strip()]
        except OSError:
            pass
    return [p if os.path.isabs(p) else os.path.join(repo, p) for p in raw]


def run_extra(repo: str, files: list) -> list:
    results = []
    for i, path in enumerate(extra_modules(repo)):
        if not os.path.exists(path):
            r = CheckResult('extra', FAIL)
            r.add(FAIL, '', f'extra check module not found: {path}')
            results.append(r)
            continue
        try:
            spec = importlib.util.spec_from_file_location(f'slynx_audit_extra_{i}', path)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            r = mod.run(repo, files)
        except Exception as e:  # an extra check that crashes must not pass silently
            r = CheckResult('extra')
            r.add(FAIL, '', f'extra check {os.path.basename(path)} crashed: {e!r}')
            r.finish()
        results.append(r)
    return results


def run_all(repo: str, only=None, refs=None, use_cache=True, allow_path=None) -> list:
    only = set(only or CHECKS)
    files = publishable_files(repo)
    results = []
    if 'data' in only:
        results.append(data_check.run(repo, files))
    if 'similarity' in only:
        results.append(similarity.run(repo, files, refs=refs, use_cache=use_cache))
    if 'notices' in only:
        results.append(notices.run(repo, files))
    if 'licenses' in only:
        results.append(licenses.run(repo, files))
    if 'extra' in only:
        results.extend(run_extra(repo, files))

    entries = load_allowlist(repo, allow_path)
    for r in results:
        apply_allowlist(r, entries)
        if r.status != SKIP:
            r.finish()
    stale = CheckResult('allow-list')
    for e in entries:
        if e.used == 0 and e.check in only:
            stale.add(WARN, e.path, f'entry for {e.check} matches nothing (remove it?): {e.reason}')
    if stale.findings:
        results.append(stale.finish())
    return results


def print_report(results: list, verbose: bool = False, out=None) -> None:
    out = out or sys.stdout
    order = {FAIL: 0, WARN: 1, INFO: 2}
    for r in results:
        print(f'== {r.name}: {r.status}' + (f' ({r.summary})' if r.summary else ''), file=out)
        for f in sorted(r.findings, key=lambda f: (bool(f.allowed_by), order.get(f.level, 3), f.check, f.path)):
            if f.allowed_by and not verbose:
                continue
            tag = 'ALLOWED' if f.allowed_by else {'FAIL': 'FAIL', 'WARN': 'REVIEW', 'INFO': 'INFO'}[f.level]
            label = f' [{f.check}]' if f.check and f.check != r.name else ''
            print(f'  {tag:<7}{label} {f.path + ": " if f.path else ""}{f.message}', file=out)
            for d in f.details:
                print(f'           {d}', file=out)
            if f.allowed_by:
                print(f'           reason: {f.allowed_by}', file=out)
        allowed = sum(1 for f in r.findings if f.allowed_by)
        if allowed and not verbose:
            print(f'  ({allowed} allow-listed; --verbose shows them)', file=out)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description='SlyNX release audit')
    ap.add_argument('--repo', default=os.path.normpath(os.path.join(HERE, '..', '..')))
    ap.add_argument('--only', help='comma-separated: ' + ','.join(CHECKS))
    ap.add_argument('--refs', nargs='+', metavar='DIR', help='reference source folders for the similarity check')
    ap.add_argument('--allowlist', help='allow-list file (default tools/audit/allowlist.toml)')
    ap.add_argument('--no-cache', action='store_true', help='re-fingerprint every reference file')
    ap.add_argument('--strict', action='store_true', help='a skipped check fails the audit')
    ap.add_argument('--verbose', '-v', action='store_true', help='also show allow-listed findings')
    args = ap.parse_args(argv)
    only = [c.strip() for c in args.only.split(',')] if args.only else None
    if only and set(only) - set(CHECKS):
        ap.error(f'unknown check(s): {", ".join(sorted(set(only) - set(CHECKS)))}')
    repo = os.path.abspath(args.repo)
    try:
        results = run_all(repo, only, args.refs, not args.no_cache, args.allowlist)
    except AllowListError as e:
        print(f'audit: allow-list error: {e}', file=sys.stderr)
        return 2
    print_report(results, args.verbose)
    failed = [r.name for r in results if r.status == FAIL or (args.strict and r.status == SKIP)]
    skipped = [r.name for r in results if r.status == SKIP]
    if failed:
        print(f'== AUDIT FAILED: {", ".join(failed)}')
        return 1
    print('== audit passed' + (f' (skipped: {", ".join(skipped)})' if skipped else ''))
    return 0


if __name__ == '__main__':
    sys.exit(main())
