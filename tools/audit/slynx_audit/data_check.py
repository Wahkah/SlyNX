"""Data and keys check: no disc data, extracted game files, packaged builds, keys or large binaries.

Looks at every file git would publish (tracked plus untracked, not ignored) and, in a repository with commits,
at every file in the history too: a file deleted from the working tree is still pushed if a commit contains it.
"""
from __future__ import annotations

import os
import re
import subprocess

from .common import FAIL, WARN, CheckResult, is_binary, is_git_repo

MAX_SIZE = 5 * 1024 * 1024
HEAD_BYTES = 0x8010          # enough to see an ISO9660 volume descriptor at 0x8001

BLOCKED_EXT = {
    # disc images and containers
    '.iso', '.bin', '.cue', '.chd', '.img', '.mdf', '.mds', '.nrg', '.cso', '.zso', '.ecm',
    # Sly worlds, scenes, packs and other extracted game files
    '.brx', '.s2w', '.s3w', '.s2v', '.wac', '.wal', '.pak', '.wad', '.irx',
    # game audio and video
    '.vag', '.vb', '.vh', '.wav', '.flac', '.ogg', '.mp3', '.m2v', '.pss', '.mpg', '.mp4', '.bik', '.adpcm', '.at9',
    # executables and packaged builds
    '.elf', '.exe', '.dll', '.so', '.dylib', '.nro', '.nso', '.nsp', '.nca', '.npdm', '.nacp', '.xci', '.nsz',
    '.xcz', '.romfs',
    # archives can hide any of the above
    '.zip', '.7z', '.rar', '.tar', '.gz', '.xz', '.zst', '.bz2',
    # keys and save data
    '.keys', '.ps2', '.mcd', '.sav',
}
BLOCKED_NAMES = {'prod.keys', 'title.keys', 'dev.keys', 'keys.txt', 'system.cnf'}
PS2_EXECUTABLE_NAME = re.compile(r'^(SCUS|SLUS|SCES|SLES|SCPS|SLPS|SCKA|SLPM|SCAJ)[_-]\d{3}\.\d{2}(\.elf)?$', re.I)
REVIEW_EXT = {'.png', '.jpg', '.jpeg', '.bmp', '.tga', '.dds', '.gif', '.webp', '.ico', '.ttf', '.otf'}

# '<name containing key/kek/seed> = <32 or 64 hex>' (prod.keys style) and '<32 hex> = <32 hex>' (title.keys style)
KEY_LINE = re.compile(
    rb'^[ \t]*(?:\w*(?:key|kek|seed)\w*[ \t]*[=:][ \t]*[0-9a-fA-F]{32}(?:[0-9a-fA-F]{32})?'
    rb'|[0-9a-fA-F]{32}[ \t]*=[ \t]*[0-9a-fA-F]{32})[ \t]*\r?$', re.M | re.I)
PRIVATE_KEY = re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH |DSA |ENCRYPTED )?PRIVATE KEY-----')
TOKEN = re.compile(rb'\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{60,})\b')


def content_problem(data: bytes) -> str | None:
    """Why these bytes (the start of a file, or all of it for text) must not be published, or None."""
    if len(data) >= 0x8006 and data[0x8001:0x8006] == b'CD001':
        return 'ISO9660 disc image'
    if data[:4] == b'PCBR':
        return 'Sly 1 world (PCBR)'
    if b'FK$X' in data[:64]:
        return 'Sly 2/3 world (FK$X)'
    if data[0x10:0x14] == b'NRO0':
        return 'Switch NRO'
    if data[:4] in (b'PFS0', b'HFS0', b'NSO0', b'META'):
        return f'Switch package or executable ({data[:4].decode()})'
    if data[:4] == b'\x7fELF' and len(data) > 0x13 and data[0x12] == 8:
        return 'PS2 (MIPS) executable'
    if data[:4] == b'PK\x03\x04' or data[:6] == b'7z\xbc\xaf\x27\x1c' or data[:4] == b'Rar!':
        return 'archive (could contain anything)'
    if KEY_LINE.search(data):
        return 'console key lines'
    if PRIVATE_KEY.search(data):
        return 'private key'
    if TOKEN.search(data):
        return 'access token'
    return None


def name_problem(rel: str) -> str | None:
    name = rel.rsplit('/', 1)[-1].lower()
    ext = os.path.splitext(name)[1]
    if name in BLOCKED_NAMES or ext in BLOCKED_EXT:
        return f'blocked file type ({ext or name})'
    if PS2_EXECUTABLE_NAME.match(name):
        return 'PS2 executable name'
    return None


def classify(rel: str, size: int, read) -> tuple:
    """(level, reason) or (None, None). `read(n)` returns the first n bytes of the file."""
    why = name_problem(rel)
    if why:
        return FAIL, why
    if size > MAX_SIZE:
        return FAIL, f'{size / 1e6:.1f} MB (over {MAX_SIZE // (1024 * 1024)} MB)'
    data = read(MAX_SIZE)  # within the limit: read it all, key lines can be anywhere
    why = content_problem(data)
    if why:
        return FAIL, why
    ext = os.path.splitext(rel.lower())[1]
    if ext in REVIEW_EXT:
        return WARN, 'image or font: make sure it is our own work, not game art, box art or a logo'
    if size and is_binary(data):
        return WARN, 'binary file: make sure it is ours and not game data'
    return None, None


def check_tree(repo: str, files: list, result: CheckResult) -> None:
    for rel in files:
        path = os.path.join(repo, rel)
        if not os.path.isfile(path):
            continue

        def read(n, path=path):
            with open(path, 'rb') as f:
                return f.read(n)
        level, why = classify(rel, os.path.getsize(path), read)
        if level:
            result.add(level, rel, why, key=why)


def history_blobs(repo: str) -> list:
    """(blob id, size, path) for every file in every commit reachable from any ref."""
    try:
        objs = subprocess.run(['git', '-C', repo, 'rev-list', '--objects', '--all'],
                              capture_output=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return []
    named = {}
    for line in objs.decode('utf-8', 'replace').splitlines():
        oid, _, path = line.partition(' ')
        if path:
            named.setdefault(oid, path)
    if not named:
        return []
    batch = subprocess.run(['git', '-C', repo, 'cat-file', '--batch-check=%(objectname) %(objecttype) %(objectsize)'],
                           input='\n'.join(named).encode(), capture_output=True, check=True).stdout
    blobs = []
    for line in batch.decode().splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] == 'blob':
            blobs.append((parts[0], int(parts[2]), named[parts[0]]))
    return blobs


def read_blob(repo: str, oid: str, n: int) -> bytes:
    data = subprocess.run(['git', '-C', repo, 'cat-file', 'blob', oid], capture_output=True, check=True).stdout
    return data[:n]


def check_history(repo: str, result: CheckResult, current: set) -> int:
    """Files in past commits. Ones identical to the working tree were already checked."""
    blobs = history_blobs(repo)
    seen = set()
    for oid, size, rel in blobs:
        if oid in seen:
            continue
        seen.add(oid)
        level, why = classify(rel, size, lambda n, oid=oid: read_blob(repo, oid, n))
        if level == FAIL and (rel, why) not in current:
            result.add(FAIL, rel, f'{why} (in git history, blob {oid[:12]})', key=why)
    return len(seen)


def run(repo: str, files: list) -> CheckResult:
    result = CheckResult('data')
    check_tree(repo, files, result)
    current = {(f.path, f.message) for f in result.findings}
    nblobs = check_history(repo, result, current) if is_git_repo(repo) else 0
    result.finish()
    nfail = sum(f.level == FAIL for f in result.findings)
    nrev = sum(f.level == WARN for f in result.findings)
    result.summary = (f'{len(files)} files to publish, {nblobs} files in history: '
                      f'{nfail} blocked, {nrev} to review')
    return result
