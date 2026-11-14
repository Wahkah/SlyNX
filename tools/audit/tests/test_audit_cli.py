"""The audit entry point: exit codes, the allow-list and extra check modules."""
import io
import os
import subprocess
import sys

import pytest
from conftest import AUDIT_DIR, git, needs_git, write

import audit
from slynx_audit.common import AllowListError, load_allowlist

ISO_HEAD = bytes(0x8001) + b'CD' + b'001' + bytes(16)


def run(repo, *args):
    out = io.StringIO()
    old = sys.stdout
    sys.stdout = out
    try:
        code = audit.main(['--repo', str(repo), '--only', 'data,notices,extra', *args])
    finally:
        sys.stdout = old
    return code, out.getvalue()


@needs_git
def test_clean_repo_passes(repo):
    write(repo, 'README.md', 'hello\n')
    code, out = run(repo)
    assert code == 0, out
    assert 'audit passed' in out


@needs_git
def test_blocked_file_fails(repo):
    write(repo, 'disc.dat', ISO_HEAD)
    code, out = run(repo)
    assert code == 1
    assert 'FAIL' in out and 'disc.dat' in out and 'AUDIT FAILED: data' in out


@needs_git
def test_allowlist_covers_a_reviewed_finding_with_reason(repo):
    write(repo, 'docs/diagram.png', b'\x89PNG\r\n')
    write(repo, 'tools/audit/allowlist.toml',
          '[[allow]]\ncheck = "data"\npath = "docs/*.png"\nreason = "Our own diagram, drawn for the docs."\n')
    code, out = run(repo)
    assert code == 0
    assert 'REVIEW' not in out and '1 allow-listed' in out
    code, out = run(repo, '--verbose')
    assert 'ALLOWED' in out and 'Our own diagram' in out


@needs_git
def test_allowlist_cannot_hide_a_failure_without_matching_and_reports_stale_entries(repo):
    write(repo, 'disc.dat', ISO_HEAD)
    write(repo, 'tools/audit/allowlist.toml',
          '[[allow]]\ncheck = "data"\npath = "other/*"\nreason = "Something else entirely."\n')
    code, out = run(repo)
    assert code == 1
    assert 'matches nothing' in out


@pytest.mark.parametrize('entry', [
    'check = "data"\npath = "x"\n',                              # no reason
    'check = "data"\npath = "x"\nreason = "ok"\n',               # reason too short
    'check = "nope"\npath = "x"\nreason = "A long enough reason."\n',
    'check = "data"\nreason = "A long enough reason."\n',        # no path
    'check = "data"\npath = "x"\nreason = "A long enough reason."\nwhy = 1\n',
])
def test_allowlist_entries_need_check_path_and_reason(tmp_path, entry):
    p = write(tmp_path, 'allow.toml', '[[allow]]\n' + entry)
    with pytest.raises(AllowListError):
        load_allowlist(str(tmp_path), str(p))


@needs_git
def test_bad_allowlist_is_exit_2(repo):
    write(repo, 'tools/audit/allowlist.toml', '[[allow]]\ncheck = "data"\npath = "x"\n')
    assert run(repo)[0] == 2


@needs_git
def test_extra_check_module_is_run(repo, tmp_path, monkeypatch):
    mod = write(tmp_path, 'extra.py',
                'from slynx_audit.common import CheckResult\n'
                'def run(repo, files):\n'
                '    r = CheckResult("extra", summary="custom")\n'
                '    if "bad.txt" in files:\n'
                '        r.add("FAIL", "bad.txt", "custom rule")\n'
                '    return r.finish()\n')
    monkeypatch.setenv('SLYNX_AUDIT_EXTRA', str(mod))
    write(repo, 'ok.txt', 'x')
    assert run(repo)[0] == 0
    write(repo, 'bad.txt', 'x')
    code, out = run(repo)
    assert code == 1 and 'custom rule' in out


@needs_git
def test_extra_check_from_git_config_and_crash_fails(repo, tmp_path):
    mod = write(tmp_path, 'crash.py', 'def run(repo, files):\n    raise RuntimeError("boom")\n')
    git(repo, 'config', 'audit.extraCheck', str(mod))
    code, out = run(repo)
    assert code == 1 and 'crashed' in out


@needs_git
def test_strict_fails_on_skipped_checks(repo):
    write(repo, 'README.md', 'x\n')
    args = ['--repo', str(repo), '--only', 'similarity']
    assert audit.main(args) == 0
    assert audit.main(args + ['--strict']) == 1


@needs_git
def test_script_runs_as_a_program(repo):
    write(repo, 'README.md', 'x\n')
    proc = subprocess.run([sys.executable, os.path.join(AUDIT_DIR, 'audit.py'), '--repo', str(repo),
                           '--only', 'data'], capture_output=True, text=True)
    assert proc.returncode == 0, proc.stdout + proc.stderr


def test_unknown_check_is_a_usage_error():
    with pytest.raises(SystemExit) as e:
        audit.main(['--only', 'nonsense'])
    assert e.value.code == 2
