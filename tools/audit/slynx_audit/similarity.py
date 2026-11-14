"""Similarity check: our code against the reference sources (code we may study but must not copy).

1. Normalise: comments, whitespace and #include-style lines are dropped; identifiers become a placeholder (V) and
   primitive type names another (T); keywords, operators and literal values are kept. Renaming variables or
   reformatting therefore does not hide a copy.
2. Fingerprint: hashes of every K-token window (k-grams), winnowed with a window of W hashes (Schleimer, Wilkerson
   and Aiken, 2003, the method behind MOSS). Any shared run of at least K + W - 1 tokens shares a fingerprint.
3. Fingerprints present in more than COMMON_LIMIT reference files are idioms (loops, getters) and are ignored.
4. A file sharing many fingerprints with one reference file fails; fewer is a warning. Matching regions are listed.

Cross-language (C/C++ reference, Rust code) structure matches are weak, so shared *distinctive literals* (long
numeric constants, precise floats, long strings) are reported separately as warnings, never as failures: many are
facts, like format magic numbers, which the specs are allowed to carry.

Reference paths come from --refs, the SLYNX_AUDIT_REFS environment variable (os.pathsep-separated) or
`git config --get-all audit.reference` (relative paths are relative to the repository).
"""
from __future__ import annotations

import os
import pickle
import re
import subprocess
import sys
import zlib
from array import array
from collections import defaultdict, deque

from .common import FAIL, INFO, SKIP, WARN, CheckResult

K = 15                   # tokens per k-gram
W = 8                    # winnowing window (in k-grams)
COMMON_LIMIT = 10        # fingerprint in more reference files than this = idiom, ignored
FAIL_SHARED = 25         # fail: at least this many shared fingerprints with one reference file ...
FAIL_FRACTION = 0.30     # ... or at least this fraction of the file's fingerprints,
FAIL_MIN_SHARED = 8      #     when that is at least this many
WARN_SHARED = 8          # warn: at least this many ...
WARN_FRACTION = 0.15     # ... or this fraction,
WARN_MIN_SHARED = 4      #     when that is at least this many
REGION_GAP = 6           # lines: fingerprints closer than this belong to one matching region
MAX_REF_SIZE = 4 * 1024 * 1024
PARAMS = (K, W, 3)       # cache key; bump the last number when normalisation changes

CODE_EXT = {'.c', '.cc', '.cpp', '.cxx', '.h', '.hh', '.hpp', '.hxx', '.inl', '.inc', '.rs',
            '.glsl', '.vert', '.frag', '.comp', '.hlsl'}
REF_SKIP_DIRS = {'.git', 'build', 'out', 'target', '.vs', 'x64', 'node_modules', '__pycache__'}

_TOKEN = re.compile(r'''
 (?P<nl>\n)
|(?P<ws>[ \t\r\f\v\\]+)
|(?P<com>//[^\n]*|/\*.*?(?:\*/|\Z))
|(?P<pp>^[ \t]*\#[ \t]*(?:include|import|pragma|if|ifdef|ifndef|elif|else|endif|error|warning|line|undef)\b[^\n]*)
|(?P<rstr>b?r(?P<hashes>\#*)".*?"(?P=hashes))
|(?P<str>(?:u8|[bLuUc])?"(?:\\.|[^"\\])*")
|(?P<chr>b?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^'\\\n])')
|(?P<num>0[xX][0-9a-fA-F_']+(?:[uUlLzi]\w*)?|0[bB][01_]+\w*|0o[0-7_]+\w*
        |(?:\d[\d_']*(?:\.(?![.a-zA-Z_])[\d_]*)?|\.\d[\d_]*)(?:[eE][+-]?\d[\d_]*)?\w*)
|(?P<id>[A-Za-z_]\w*)
|(?P<op>>>=|<<=|\.\.\.|\.\.=|::|->|=>|\+\+|--|&&|\|\||<<|>>|[<>=!+\-*/%&|^]=|\.\.|\S)
''', re.X | re.S | re.M)

KEYWORDS = set('''
if else while for do return break continue switch case default goto struct union enum class typedef const static
extern inline volatile sizeof true false this new delete template typename namespace using public private protected
virtual override operator friend constexpr nullptr NULL try catch throw
fn let mut impl trait pub use mod match loop where as ref move unsafe self Self super crate dyn type in Some None Ok
Err
'''.split())
TYPES = set('''
void bool char short int long float double unsigned signed auto size_t ssize_t uintptr_t intptr_t ptrdiff_t
uint8_t uint16_t uint32_t uint64_t int8_t int16_t int32_t int64_t u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64
usize isize uchar ushort uint ulong byte BYTE WORD DWORD QWORD UINT INT FLOAT BOOL vec2 vec3 vec4 mat3 mat4
'''.split())

_codes: dict = {}


def _code(tok: str) -> int:
    c = _codes.get(tok)
    if c is None:
        c = _codes[tok] = zlib.crc32(tok.encode('utf-8', 'replace')) + 1
    return c


_NUM_BODY = re.compile(r'(\d*\.?\d*(?:e[+-]?\d+)?)(.*)')


def canon_number(text: str):
    """('i', int) or ('f', float) for a numeric literal in C, C++ or Rust syntax, or None."""
    t = text.replace('_', '').replace("'", '').lower()
    try:
        if t.startswith('0x'):
            return 'i', int(re.match(r'0x[0-9a-f]+', t).group(), 16)
        if t.startswith('0b'):
            return 'i', int(re.match(r'0b[01]+', t).group(), 2)
        if t.startswith('0o'):
            return 'i', int(re.match(r'0o[0-7]+', t).group()[2:], 8)
        body, suffix = _NUM_BODY.match(t).groups()
        if '.' in body or 'e' in body or suffix.startswith('f'):
            return 'f', float(body)
        if len(body) > 1 and body.startswith('0') and body.isdigit() and set(body) <= set('01234567'):
            return 'i', int(body, 8)  # C octal
        return 'i', int(body)
    except (ValueError, AttributeError):
        return None


def distinctive(kind: str, value) -> bool:
    """Is this literal unusual enough that sharing it with a reference file is worth a look?"""
    if kind == 'i':
        v = abs(value)
        if v < 0x10000:
            return False
        hx, dec = format(v, 'x').rstrip('0'), str(v).rstrip('0')
        return len(hx) >= 4 and len(set(hx)) >= 3 and len(dec) >= 4 and len(set(dec)) >= 3
    if kind == 'f':
        mantissa = repr(abs(value)).split('e')[0].replace('.', '').strip('0')
        return len(mantissa) >= 5
    if kind == 's':
        s = value.strip()
        return len(s) >= 12 and sum(c.isalpha() for c in s) >= 4
    return False


def literal_label(key: str) -> str:
    kind, _, val = key.partition(':')
    if kind == 'i':
        v = int(val)
        return f'{v} (0x{v:X})'
    if kind == 's':
        return '"' + (val if len(val) <= 60 else val[:57] + '...') + '"'
    return val


def normalise(text: str):
    """(codes, lines, literals): token codes, the line of each token, and distinctive literals as
    {key: first line}."""
    codes, lines, lits = [], [], {}
    line = 1
    for m in _TOKEN.finditer(text):
        kind = m.lastgroup
        s = m.group()
        if kind == 'nl':
            line += 1
            continue
        if kind in ('ws', 'pp', 'hashes'):
            continue
        if kind == 'com':
            line += s.count('\n')
            continue
        if kind == 'id':
            tok = s if s in KEYWORDS else 'T' if s in TYPES else 'V'
        elif kind == 'num':
            n = canon_number(s)
            if n is None:
                tok = 'N?'
            else:
                tok = f'{n[0]}:{n[1]!r}'
                if distinctive(*n):
                    lits.setdefault(f'{n[0]}:{n[1]!r}', line)
        elif kind in ('str', 'rstr'):
            body = s[s.index('"') + 1:s.rindex('"')]
            tok = 's:' + body
            if distinctive('s', body):
                lits.setdefault(tok, line)
            line_inc = s.count('\n')
            codes.append(_code(tok))
            lines.append(line)
            line += line_inc
            continue
        elif kind == 'chr':
            tok = 'c:' + s[s.index("'") + 1:-1]
        else:
            tok = s
        codes.append(_code(tok))
        lines.append(line)
    return codes, lines, lits


_MOD = (1 << 61) - 1
_BASE = 1000003


def kgram_hashes(codes: list, k: int = K) -> list:
    n = len(codes)
    if n < k:
        return []
    top = pow(_BASE, k - 1, _MOD)
    h = 0
    for c in codes[:k]:
        h = (h * _BASE + c) % _MOD
    out = [h]
    for i in range(k, n):
        h = ((h - codes[i - k] * top) * _BASE + codes[i]) % _MOD
        out.append(h)
    return out


def winnow(hashes: list, w: int = W) -> list:
    """Indices of the selected hashes: the rightmost minimum of every window of w, each recorded once."""
    if not hashes:
        return []
    if len(hashes) < w:
        return [min(range(len(hashes)), key=lambda i: (hashes[i], -i))]
    picked, window, last = [], deque(), -1
    for i, h in enumerate(hashes):
        while window and hashes[window[-1]] >= h:
            window.pop()
        window.append(i)
        if window[0] <= i - w:
            window.popleft()
        if i >= w - 1 and window[0] != last:
            last = window[0]
            picked.append(last)
    return picked


def fingerprint(text: str):
    """(hashes, start lines, end lines, literals) for one source file."""
    codes, lines, lits = normalise(text)
    hs = kgram_hashes(codes)
    hashes, l1, l2 = array('Q'), array('I'), array('I')
    for i in winnow(hs):
        hashes.append(hs[i])
        l1.append(lines[i])
        l2.append(lines[i + K - 1])
    return hashes, l1, l2, lits


def read_text(path: str) -> str:
    with open(path, 'rb') as f:
        return f.read().decode('utf-8', 'replace')


# ---------------------------------------------------------------- references

def reference_roots(repo: str, cli_refs=None) -> list:
    if cli_refs:
        raw = list(cli_refs)
    elif os.environ.get('SLYNX_AUDIT_REFS'):
        raw = [p for p in os.environ['SLYNX_AUDIT_REFS'].split(os.pathsep) if p.strip()]
    else:
        try:
            out = subprocess.run(['git', '-C', repo, 'config', '--get-all', 'audit.reference'],
                                 capture_output=True, text=True).stdout
        except OSError:
            out = ''
        raw = [p.strip() for p in out.splitlines() if p.strip()]
    return [os.path.normpath(p if os.path.isabs(p) else os.path.join(repo, p)) for p in raw]


def reference_files(root: str) -> list:
    out = []
    for dirpath, dirs, names in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in REF_SKIP_DIRS)
        for n in sorted(names):
            if os.path.splitext(n)[1].lower() in CODE_EXT:
                out.append(os.path.join(dirpath, n))
    return out


def cache_path() -> str:
    base = os.environ.get('SLYNX_AUDIT_CACHE')
    if not base:
        if sys.platform == 'win32' and os.environ.get('LOCALAPPDATA'):
            base = os.path.join(os.environ['LOCALAPPDATA'], 'slynx-audit')
        else:
            base = os.path.join(os.environ.get('XDG_CACHE_HOME') or os.path.expanduser('~/.cache'), 'slynx-audit')
    return os.path.join(base, 'reference-fingerprints.pickle')


def load_cache(path: str) -> dict:
    try:
        with open(path, 'rb') as f:
            data = pickle.load(f)
        return data if data.get('params') == PARAMS else {'params': PARAMS}
    except (OSError, EOFError, pickle.UnpicklingError, AttributeError, ValueError):
        return {'params': PARAMS}


def save_cache(path: str, data: dict) -> None:
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        tmp = path + '.tmp'
        with open(tmp, 'wb') as f:
            pickle.dump(data, f, protocol=pickle.HIGHEST_PROTOCOL)
        os.replace(tmp, path)
    except OSError:
        pass


# ---------------------------------------------------------------- comparison

def regions(matches: list) -> list:
    """Group (our l1, our l2, ref l1, ref l2) matches into contiguous regions:
    (our first, our last, ref first, ref last, fingerprints)."""
    groups = []
    for a1, a2, b1, b2 in sorted(matches):
        if groups and a1 <= groups[-1][1] + REGION_GAP:
            groups[-1][1] = max(groups[-1][1], a2)
            groups[-1][2].append((b1, b2))
        else:
            groups.append([a1, a2, [(b1, b2)]])
    out = []
    for a1, a2, refs in groups:
        # the reference side of a region: its largest cluster of nearby lines (a common k-gram can also
        # match somewhere far away in the reference file)
        refs.sort()
        best, cur = [refs[0]], [refs[0]]
        for r in refs[1:]:
            cur = cur + [r] if r[0] <= max(x[1] for x in cur) + 2 * REGION_GAP else [r]
            if len(cur) > len(best):
                best = cur
        out.append((a1, a2, best[0][0], max(x[1] for x in best), len(refs)))
    return out


def verdict(shared: int, total: int):
    frac = shared / total if total else 0.0
    if shared >= FAIL_SHARED or (frac >= FAIL_FRACTION and shared >= FAIL_MIN_SHARED):
        return FAIL
    if shared >= WARN_SHARED or (frac >= WARN_FRACTION and shared >= WARN_MIN_SHARED):
        return WARN
    return None


def run(repo: str, files: list, refs=None, use_cache: bool = True) -> CheckResult:
    result = CheckResult('similarity')
    ours = [f for f in files if os.path.splitext(f)[1].lower() in CODE_EXT]
    roots = reference_roots(repo, refs)
    present = [r for r in roots if os.path.isdir(r)]
    for r in roots:
        if r not in present:
            result.add(WARN, '', f'reference folder not found: {r}', key=r)
    if not present:
        result.status = SKIP
        result.summary = ('no reference sources configured or found (use --refs, SLYNX_AUDIT_REFS or '
                          'git config audit.reference)')
        return result

    # our side
    index = defaultdict(list)          # hash -> [(our file id, l1, l2)]
    lit_index = defaultdict(list)      # literal key -> [(our file id, line)]
    totals = []
    for sid, rel in enumerate(ours):
        hs, l1, l2, lits = fingerprint(read_text(os.path.join(repo, rel)))
        totals.append(len(set(hs)))
        for h, a, b in zip(hs, l1, l2):
            index[h].append((sid, a, b))
        for key, line in lits.items():
            lit_index[key].append((sid, line))

    # reference side, streamed (only hashes we have are kept)
    cpath = cache_path()
    cache = load_cache(cpath) if use_cache else {'params': PARAMS}
    dirty = False
    ref_names, ref_files = [], 0
    doc_freq = defaultdict(set)        # hash -> reference file ids
    ref_pos = {}                       # (ref id, hash) -> (l1, l2)
    lit_refs = defaultdict(list)       # literal key -> [(ref id, line)]
    skipped = 0
    for root in present:
        label = os.path.basename(os.path.dirname(root)) + '/' + os.path.basename(root)
        for path in reference_files(root):
            try:
                st = os.stat(path)
            except OSError:
                continue
            if st.st_size > MAX_REF_SIZE:
                skipped += 1
                continue
            sig = (st.st_size, st.st_mtime_ns)
            entry = cache.get(path)
            if entry is None or entry[0] != sig:
                entry = (sig, *fingerprint(read_text(path)))
                cache[path] = entry
                dirty = True
            _, hs, l1, l2, lits = entry
            rid = len(ref_names)
            ref_names.append(label + '/' + os.path.relpath(path, root).replace(os.sep, '/'))
            ref_files += 1
            for h, a, b in zip(hs, l1, l2):
                if h in index:
                    doc_freq[h].add(rid)
                    ref_pos.setdefault((rid, h), (a, b))
            for key, line in lits.items():
                if key in lit_index:
                    lit_refs[key].append((rid, line))
    if use_cache and dirty:
        save_cache(cpath, cache)

    # pairs
    pair_hashes = defaultdict(set)
    pair_matches = defaultdict(list)
    common = 0
    for h, rids in doc_freq.items():
        if len(rids) > COMMON_LIMIT:
            common += 1
            continue
        for rid in rids:
            b1, b2 = ref_pos[(rid, h)]
            for sid, a1, a2 in index[h]:
                pair_hashes[(sid, rid)].add(h)
                pair_matches[(sid, rid)].append((a1, a2, b1, b2))

    by_file = defaultdict(list)
    for (sid, rid), hs in pair_hashes.items():
        level = verdict(len(hs), totals[sid])
        if level:
            by_file[sid].append((len(hs), rid, level))
    for sid, hits in sorted(by_file.items()):
        hits.sort(reverse=True)
        for shared, rid, level in hits[:3]:
            regs = sorted(regions(pair_matches[(sid, rid)]), key=lambda r: -r[4])[:5]
            details = [f'{ours[sid]}:{a}-{b}  ~  {ref_names[rid]}:{c}-{d}  ({n} fingerprints)'
                       for a, b, c, d, n in sorted(regs)]
            pct = 100.0 * shared / totals[sid] if totals[sid] else 0.0
            result.add(level, ours[sid],
                       f'shares {shared} of {totals[sid]} fingerprints ({pct:.0f}%) with {ref_names[rid]}',
                       key=ref_names[rid], details=details)
        if len(hits) > 3:
            result.add(INFO, ours[sid], f'... and {len(hits) - 3} more reference files over a threshold',
                       key='more')

    # literals: reported, never a failure
    lit_count = 0
    for key, refs_hit in sorted(lit_refs.items()):
        rids = sorted({rid for rid, _ in refs_hit})
        for sid, line in lit_index[key]:
            lit_count += 1
            where = ', '.join(ref_names[r] for r in rids[:3]) + (f' and {len(rids) - 3} more' if len(rids) > 3 else '')
            result.add(WARN, ours[sid], f'line {line}: literal {literal_label(key)} also in {where}',
                       key=key.partition(':')[2], check='literals')

    result.summary = (f'{len(ours)} code files against {ref_files} reference files'
                      f'{f" ({skipped} over {MAX_REF_SIZE >> 20} MB skipped)" if skipped else ""}; '
                      f'k={K}, w={W}; {common} idiom fingerprints ignored; '
                      f'{sum(1 for h in by_file.values() for x in h if x[2] == FAIL)} over the fail threshold, '
                      f'{lit_count} shared literals')
    return result.finish()
