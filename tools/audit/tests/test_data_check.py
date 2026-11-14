"""Data and keys check, on synthetic files only (fake headers built here, never real game data)."""
import os

from conftest import git, needs_git, write

from slynx_audit import data_check as dc
from slynx_audit.common import FAIL, WARN, publishable_files


def fake_iso():
    data = bytearray(0x8010)
    data[0x8000] = 1
    data[0x8001:0x8006] = b'CD' + b'001'
    return bytes(data)


def fake_nro():
    data = bytearray(0x80)
    data[0x10:0x14] = b'NR' + b'O0'
    return bytes(data)


def fake_ps2_elf():
    data = bytearray(0x40)
    data[:4] = b'\x7fELF'
    data[0x12] = 8  # EM_MIPS
    return bytes(data)


def hexkey(n):
    return ''.join('0123456789abcdef'[(i * 7) % 16] for i in range(n))


def run_tree(root):
    return dc.run(str(root), publishable_files(str(root)))


@needs_git
def test_blocks_disc_data_packages_and_keys_by_name_and_content(repo):
    write(repo, 'crates/ok.rs', 'fn main() {}\n')
    write(repo, 'level.brx', b'PC' + b'BR' + bytes(12))                 # by extension
    write(repo, 'renamed.dat', b'PC' + b'BR' + bytes(12))               # by content
    write(repo, 'w.dat', bytes(8) + b'FK' + b'$X' + bytes(8))
    write(repo, 'disc.dat', fake_iso())
    write(repo, 'game.dat', fake_nro())
    write(repo, 'pkg.dat', b'PF' + b'S0' + bytes(16))
    write(repo, 'boot.dat', fake_ps2_elf())
    write(repo, 'SCUS_123.45', b'x')
    write(repo, 'notes.txt', 'some notes\nheader_key = ' + hexkey(64) + '\n')
    write(repo, 'titles.txt', hexkey(32) + ' = ' + hexkey(32) + '\n')
    write(repo, 'prod.keys', 'x')
    write(repo, 'id.txt', '-----BEGIN ' + 'PRIVATE KEY-----\nabc\n')
    write(repo, 'bundle.dat', b'PK\x03\x04' + bytes(20))
    write(repo, 'big.dat', bytes(dc.MAX_SIZE + 1))
    write(repo, 'build/app.nro', b'x')
    write(repo, 'shot.png', b'\x89PNG\r\n')
    write(repo, 'blob.dat', b'\x01\x02\x00\x03')
    r = run_tree(repo)
    blocked = {f.path for f in r.findings if f.level == FAIL}
    review = {f.path for f in r.findings if f.level == WARN}
    assert blocked == {'level.brx', 'renamed.dat', 'w.dat', 'disc.dat', 'game.dat', 'pkg.dat', 'boot.dat',
                       'SCUS_123.45', 'notes.txt', 'titles.txt', 'prod.keys', 'id.txt', 'bundle.dat', 'big.dat',
                       'build/app.nro'}
    assert review == {'shot.png', 'blob.dat'}
    assert r.status == FAIL


@needs_git
def test_clean_tree_passes_and_ignored_files_are_not_scanned(repo):
    write(repo, '.gitignore', '*.iso\n/extracted/\n')
    write(repo, 'README.md', '# hi\n')
    write(repo, 'crates/a/src/lib.rs', 'pub fn f() -> u32 { 0x1234 }\n')
    write(repo, 'my.iso', fake_iso())
    write(repo, 'extracted/w.dat', b'PC' + b'BR')
    r = run_tree(repo)
    assert r.status == 'PASS', r.findings
    assert 'my.iso' not in publishable_files(str(repo))


def test_key_pattern_ignores_code_that_builds_or_describes_keys():
    for src in (b"keys.write_text(f'header_key = {secrets.token_hex(32)}\\n')",
                b'let header_key = [0u8; 32];',
                b'header_key = <32 hex digits>',
                b'// the key area key is 16 bytes'):
        assert dc.content_problem(src) is None, src


def test_key_line_anywhere_in_a_text_file():
    text = ('x\n' * 5000 + 'master_key_00 = ' + hexkey(32) + '\r\n').encode()
    assert dc.content_problem(text) == 'console key lines'


@needs_git
def test_files_in_git_history_are_checked_even_when_deleted(repo):
    write(repo, 'README.md', 'hi\n')
    write(repo, 'oops.dat', fake_iso())
    git(repo, 'add', '.')
    git(repo, 'commit', '-q', '-m', 'add')
    os.remove(repo / 'oops.dat')
    git(repo, 'commit', '-q', '-am', 'remove')
    r = run_tree(repo)
    assert [(f.path, f.level) for f in r.findings] == [('oops.dat', FAIL)]
    assert 'git history' in r.findings[0].message


def test_without_git_every_file_is_listed(tmp_path, monkeypatch):
    monkeypatch.setattr('shutil.which', lambda name: None)
    write(tmp_path, 'a/b.txt', 'x')
    write(tmp_path, '.git/config', 'x')
    assert publishable_files(str(tmp_path)) == ['a/b.txt']
