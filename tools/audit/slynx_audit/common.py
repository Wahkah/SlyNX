"""Shared pieces: results, the list of files git would publish, and the allow-list."""
from __future__ import annotations

import fnmatch
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field

try:
    import tomllib
except ImportError:  # Python < 3.11
    tomllib = None

FAIL, WARN, INFO = 'FAIL', 'WARN', 'INFO'
PASS, SKIP = 'PASS', 'SKIP'

SKIP_DIRS = {'.git', '__pycache__', '.pytest_cache', 'target'}

ALLOWLIST_PATH = 'tools/audit/allowlist.toml'
CHECK_NAMES = ('data', 'similarity', 'literals', 'notices', 'licenses', 'extra')


@dataclass
class Finding:
    level: str              # FAIL | WARN | INFO
    path: str               # repo-relative path the finding is about ('' when none)
    message: str
    key: str = ''           # what an allow-list entry's `match` is compared with
    details: list = field(default_factory=list)
    check: str = ''         # allow-list check name; defaults to the CheckResult's name
    allowed_by: str = ''    # reason, when an allow-list entry covers this finding


@dataclass
class CheckResult:
    name: str
    status: str = PASS      # PASS | FAIL | WARN | SKIP
    summary: str = ''
    findings: list = field(default_factory=list)

    def add(self, level, path, message, key='', details=None, check=''):
        self.findings.append(Finding(level, path, message, key, list(details or []), check or self.name))

    def finish(self):
        """Status from the findings that are not allow-listed (SKIP is kept)."""
        if self.status == SKIP:
            return self
        live = [f for f in self.findings if not f.allowed_by]
        if any(f.level == FAIL for f in live):
            self.status = FAIL
        elif any(f.level == WARN for f in live):
            self.status = WARN
        else:
            self.status = PASS
        return self


def _git(repo):
    return ['git', '-C', repo]


def is_git_repo(repo: str) -> bool:
    return shutil.which('git') is not None and os.path.exists(os.path.join(repo, '.git'))


def publishable_files(repo: str) -> list:
    """Repo-relative '/'-separated paths git would publish: tracked files plus untracked files that are not
    ignored (.gitignore and .git/info/exclude). A folder that is not a repository is previewed with a throwaway
    git directory (what `git init && git add .` would take); without git, every file is listed."""
    try:
        if shutil.which('git') is None:
            raise OSError('git not found')
        tmp = None
        if os.path.exists(os.path.join(repo, '.git')):
            cmd = _git(repo)
        else:
            tmp = tempfile.mkdtemp(prefix='slynx_audit_')
            subprocess.run(['git', 'init', '-q', '--bare', tmp], capture_output=True, check=True)
            cmd = ['git', f'--git-dir={tmp}', f'--work-tree={repo}', '-C', repo]
        try:
            out = subprocess.run(cmd + ['ls-files', '-z', '--cached', '--others', '--exclude-standard'],
                                 capture_output=True, check=True).stdout
        finally:
            if tmp:
                shutil.rmtree(tmp, ignore_errors=True)
        files = {p.decode('utf-8', 'replace') for p in out.split(b'\0') if p}
        # tracked files deleted from the working tree are not published by `git add .`
        return sorted(f for f in files if os.path.lexists(os.path.join(repo, f)))
    except (OSError, subprocess.CalledProcessError):
        files = []
        for root, dirs, names in os.walk(repo):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for n in names:
                files.append(os.path.relpath(os.path.join(root, n), repo).replace(os.sep, '/'))
        return sorted(files)


def is_binary(data: bytes) -> bool:
    return b'\0' in data[:8192]


# ---------------------------------------------------------------- allow-list

@dataclass
class AllowEntry:
    check: str
    path: str
    match: str
    reason: str
    used: int = 0


class AllowListError(ValueError):
    pass


def load_allowlist(repo: str, path: str | None = None) -> list:
    """Entries from tools/audit/allowlist.toml. Every entry needs `check`, `path` and a non-empty `reason`."""
    path = path or os.path.join(repo, ALLOWLIST_PATH)
    if not os.path.exists(path):
        return []
    if tomllib is None:
        raise AllowListError('Python 3.11+ is needed to read the allow-list (tomllib)')
    with open(path, 'rb') as f:
        try:
            doc = tomllib.load(f)
        except tomllib.TOMLDecodeError as e:
            raise AllowListError(f'{path}: {e}') from e
    entries = []
    for i, raw in enumerate(doc.get('allow', []), 1):
        where = f'{os.path.basename(path)} entry {i}'
        unknown = set(raw) - {'check', 'path', 'match', 'reason'}
        if unknown:
            raise AllowListError(f'{where}: unknown field(s) {sorted(unknown)}')
        check = raw.get('check', '')
        if check not in CHECK_NAMES:
            raise AllowListError(f'{where}: check must be one of {", ".join(CHECK_NAMES)}')
        if not str(raw.get('path', '')).strip():
            raise AllowListError(f'{where}: path is required (a glob; use "*" for any file)')
        reason = str(raw.get('reason', '')).strip()
        if len(reason) < 10:
            raise AllowListError(f'{where}: a reason (at least 10 characters) is required')
        entries.append(AllowEntry(check, str(raw['path']), str(raw.get('match', '')), reason))
    return entries


def apply_allowlist(result: CheckResult, entries: list) -> None:
    for f in result.findings:
        for e in entries:
            if e.check != (f.check or result.name):
                continue
            if not fnmatch.fnmatchcase(f.path, e.path):
                continue
            if e.match and not fnmatch.fnmatchcase(f.key, e.match):
                continue
            f.allowed_by = e.reason
            e.used += 1
            break
