import os
import shutil
import subprocess
import sys

import pytest

AUDIT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if AUDIT_DIR not in sys.path:
    sys.path.insert(0, AUDIT_DIR)

HAS_GIT = shutil.which('git') is not None
needs_git = pytest.mark.skipif(not HAS_GIT, reason='git not installed')


def git(repo, *args):
    return subprocess.run(['git', '-C', str(repo), '-c', 'user.name=Test', '-c', 'user.email=test@example.com',
                           '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', *args],
                          capture_output=True, check=True, text=True).stdout


def write(root, rel, data):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, str):
        path.write_text(data, encoding='utf-8', newline='\n')
    else:
        path.write_bytes(data)
    return path


@pytest.fixture
def repo(tmp_path, monkeypatch):
    """An empty git repository, with the audit's environment variables cleared."""
    for var in ('SLYNX_AUDIT_REFS', 'SLYNX_AUDIT_EXTRA'):
        monkeypatch.delenv(var, raising=False)
    monkeypatch.setenv('SLYNX_AUDIT_CACHE', str(tmp_path / 'cache'))
    r = tmp_path / 'repo'
    r.mkdir()
    if HAS_GIT:
        git(r, 'init', '-q')
    return r
