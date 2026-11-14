"""Notices check: every third-party crate in a Cargo.lock is listed in THIRD_PARTY_NOTICES.md."""
from __future__ import annotations

import os
import re

from .common import FAIL, CheckResult

NOTICES = 'THIRD_PARTY_NOTICES.md'
_PACKAGE = re.compile(r'^\[\[package\]\]\s*$', re.M)
_FIELD = re.compile(r'^(name|version|source)\s*=\s*"([^"]*)"\s*$', re.M)


def lock_packages(text: str) -> list:
    """(name, version, source) of each [[package]] in a Cargo.lock. Our own workspace crates have no source."""
    out = []
    for block in _PACKAGE.split(text)[1:]:
        block = block.split('\n[', 1)[0]  # stop at the next table ([metadata] in old lock files)
        fields = dict(_FIELD.findall(block))
        if 'name' in fields:
            out.append((fields['name'], fields.get('version', ''), fields.get('source', '')))
    return out


def mentioned(name: str, notices: str) -> bool:
    return re.search(r'(?<![\w-])' + re.escape(name) + r'(?![\w-])', notices, re.I) is not None


def run(repo: str, files: list) -> CheckResult:
    result = CheckResult('notices')
    locks = [f for f in files if f.rsplit('/', 1)[-1] == 'Cargo.lock']
    if not locks:
        result.summary = 'no Cargo.lock: no third-party crates yet'
        return result.finish()
    path = os.path.join(repo, NOTICES)
    notices = ''
    if os.path.exists(path):
        with open(path, encoding='utf-8', errors='replace') as f:
            notices = f.read()
    third_party = {}
    for lock in locks:
        with open(os.path.join(repo, lock), encoding='utf-8', errors='replace') as f:
            for name, version, source in lock_packages(f.read()):
                if source:
                    third_party.setdefault(name, set()).add(version)
    missing = sorted(n for n in third_party if not mentioned(n, notices))
    if third_party and not notices:
        result.add(FAIL, NOTICES, f'missing, but {len(third_party)} third-party crates are locked', key='missing')
    else:
        for name in missing:
            result.add(FAIL, NOTICES, f'crate {name} {", ".join(sorted(third_party[name]))} is not listed', key=name)
    result.summary = (f'{len(third_party)} third-party crates in {", ".join(locks)}; '
                      f'{len(missing)} missing from {NOTICES}')
    return result.finish()
