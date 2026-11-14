"""Similarity check, with synthetic "reference" code written for these tests."""
import random

from conftest import needs_git, write

from slynx_audit import similarity as sim
from slynx_audit.common import FAIL, WARN, publishable_files

# A made-up C routine standing in for reference code.
REF_C = '''
#include <stdio.h>
/* Moves every particle and bounces it off the floor. */
typedef struct { float x, y, vx, vy; int alive; } particle_t;

static int step_particles(particle_t *list, int count, float dt, float floor_y)
{
    int bounced = 0;
    for (int i = 0; i < count; ++i) {
        particle_t *p = &list[i];
        if (!p->alive) continue;
        p->vy -= 9.81f * dt;
        p->x += p->vx * dt;
        p->y += p->vy * dt;
        if (p->y < floor_y) {
            p->y = floor_y + (floor_y - p->y) * 0.5f;
            p->vy = -p->vy * 0.75f;
            if (p->vy < 0.25f) { p->vy = 0.0f; p->alive = 0; }
            bounced += 1;
        }
        if (p->x > 1000.0f || p->x < -1000.0f) p->alive = 0;
    }
    return bounced;
}

static unsigned mix_seed(unsigned seed, unsigned salt)
{
    seed ^= salt + 0x9E3779B9u + (seed << 6) + (seed >> 2);
    seed = (seed ^ (seed >> 16)) * 0x7FEB352Du;
    seed = (seed ^ (seed >> 15)) * 0x846CA68Bu;
    return seed ^ (seed >> 16);
}
'''

# The same code with every name changed, comments rewritten and the layout changed.
DISGUISED_C = '''// physics helpers
typedef struct { float a, b, c, d; int on; } dot;
static int advance(dot *arr, int n, float t, float ground) {
  int hits = 0;
  for (int k = 0; k < n; ++k) { dot *q = &arr[k]; if (!q->on) continue;
    q->d -= 9.81f * t; q->a += q->c * t; q->b += q->d * t;
    if (q->b < ground) { q->b = ground + (ground - q->b) * 0.5f; q->d = -q->d * 0.75f;
      if (q->d < 0.25f) { q->d = 0.0f; q->on = 0; } hits += 1; }
    if (q->a > 1000.0f || q->a < -1000.0f) q->on = 0; }
  return hits;
}
static unsigned scramble(unsigned s, unsigned k) {
  s ^= k + 0x9E3779B9u + (s << 6) + (s >> 2); s = (s ^ (s >> 16)) * 0x7FEB352Du;
  s = (s ^ (s >> 15)) * 0x846CA68Bu; return s ^ (s >> 16);
}
'''

# Independent Rust code that happens to use the same hash constants (a fact, not a copy).
RUST_WITH_CONSTANTS = '''
/// Finaliser from the format spec.
pub fn finalise(mut h: u32) -> u32 {
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}
'''


OTHER_REF_RS = '''
pub enum Cmd { Move(i32, i32), Wait(u16), Say(String) }

pub fn parse(line: &str) -> Option<Cmd> {
    let mut it = line.split_whitespace();
    match it.next()? {
        "move" => Some(Cmd::Move(it.next()?.parse().ok()?, it.next()?.parse().ok()?)),
        "wait" => it.next()?.parse().ok().map(Cmd::Wait),
        "say" => Some(Cmd::Say(it.collect::<Vec<_>>().join(" "))),
        _ => None,
    }
}
'''


def unrelated_rust(seed):
    rnd = random.Random(seed)
    lines = ['pub struct Table { rows: Vec<u32> }', 'impl Table {']
    for i in range(30):
        op = rnd.choice(['+', '-', '*', '^', '|', '&'])
        lines.append(f'    pub fn f{i}(&self, a: u32) -> u32 {{ let mut t = a {op} {rnd.randrange(1, 99)}; '
                     f'for r in &self.rows {{ t = t.rotate_left({rnd.randrange(1, 31)}) {op} *r; }} t }}')
    lines.append('}')
    return '\n'.join(lines) + '\n'


def test_normalisation_ignores_names_comments_and_layout():
    a, _, _ = sim.normalise(REF_C.split('static unsigned')[0].split('typedef', 1)[1])
    b, _, _ = sim.normalise(DISGUISED_C.split('static unsigned')[0].split('typedef', 1)[1])
    assert a == b


def test_normalisation_keeps_literals_and_structure():
    a, _, _ = sim.normalise('x = 1 + y;')
    b, _, _ = sim.normalise('x = 2 + y;')
    c, _, _ = sim.normalise('x = 1 - y;')
    assert a != b and a != c


def test_numbers_are_canonical_across_languages():
    assert sim.canon_number('0x7FEB352Du') == sim.canon_number('0x7feb_352d') == ('i', 0x7FEB352D)
    assert sim.canon_number('0.75f') == sim.canon_number('0.75_f32') == ('f', 0.75)
    assert sim.canon_number('1e3') == ('f', 1000.0)
    assert sim.canon_number('10u8') == ('i', 10)
    assert sim.canon_number('0755') == ('i', 0o755)


def test_comments_and_strings_do_not_confuse_the_tokenizer():
    codes, lines, lits = sim.normalise('s = "a // not a comment /* nor this */";\n/* multi\nline */ t = 1;\n')
    assert lines[-1] == 3  # line numbers survive the multi-line comment
    assert len(codes) == 8  # s = "..." ; t = 1 ;


def test_rust_lifetimes_and_raw_strings():
    codes, _, _ = sim.normalise('fn f<\'a>(x: &\'a str) -> &\'a str { let r = r#"raw "q" here"#; x }')
    assert codes  # tokenises without swallowing the rest of the file
    assert sim._code('s:raw "q" here') in codes


def test_winnowing_guarantee():
    """Every run of hashes as long as the window keeps at least one fingerprint."""
    rnd = random.Random(1)
    hs = [rnd.randrange(1 << 30) for _ in range(500)]
    picked = set(sim.winnow(hs, 8))
    for start in range(len(hs) - 8 + 1):
        assert picked & set(range(start, start + 8))
    assert len(picked) < len(hs) / 2


def test_distinctive_literals():
    assert sim.distinctive('i', 0x7FEB352D)
    assert not sim.distinctive('i', 0xFFFFFFFF)
    assert not sim.distinctive('i', 0x10000)
    assert not sim.distinctive('i', 1000000)
    assert not sim.distinctive('i', 255)
    assert sim.distinctive('f', 0.017453292)
    assert not sim.distinctive('f', 0.5)
    assert sim.distinctive('s', 'could not open the world file')
    assert not sim.distinctive('s', '%d %d %d')


def make_refs(tmp_path, files):
    root = tmp_path / 'refs' / 'project'
    for rel, text in files.items():
        write(root, rel, text)
    return str(root)


@needs_git
def test_disguised_copy_fails_with_regions(repo, tmp_path):
    refs = make_refs(tmp_path, {'src/particles.c': REF_C})
    write(repo, 'crates/fx/c/fx.c', DISGUISED_C)
    r = sim.run(str(repo), publishable_files(str(repo)), refs=[refs])
    fails = [f for f in r.findings if f.level == FAIL]
    assert r.status == FAIL
    assert [f.path for f in fails] == ['crates/fx/c/fx.c']
    assert 'project/src/particles.c' in fails[0].message
    assert any('fx.c:' in d and 'particles.c:' in d for d in fails[0].details)


@needs_git
def test_unrelated_code_passes_and_shared_constants_are_only_reported(repo, tmp_path):
    refs = make_refs(tmp_path, {'src/particles.c': REF_C, 'src/other.rs': OTHER_REF_RS})
    write(repo, 'crates/a/src/lib.rs', unrelated_rust(2))
    write(repo, 'crates/a/src/hash.rs', RUST_WITH_CONSTANTS)
    r = sim.run(str(repo), publishable_files(str(repo)), refs=[refs])
    assert r.status == WARN, [(f.level, f.message) for f in r.findings]
    assert not [f for f in r.findings if f.level == FAIL]
    lits = [f for f in r.findings if f.check == 'literals']
    assert {f.path for f in lits} == {'crates/a/src/hash.rs'}
    assert {f.key for f in lits} == {str(0x7FEB352D), str(0x846CA68B)}


@needs_git
def test_idioms_shared_by_many_reference_files_are_ignored(repo, tmp_path):
    files = {f'src/f{i}.c': REF_C.replace('step_particles', f'step{i}') for i in range(sim.COMMON_LIMIT + 1)}
    refs = make_refs(tmp_path, files)
    write(repo, 'crates/fx/c/fx.c', DISGUISED_C)
    r = sim.run(str(repo), publishable_files(str(repo)), refs=[refs])
    assert not [f for f in r.findings if f.level == FAIL and f.check == 'similarity']


@needs_git
def test_no_references_means_skip(repo, tmp_path):
    write(repo, 'a.rs', 'fn main() {}\n')
    r = sim.run(str(repo), publishable_files(str(repo)), refs=[str(tmp_path / 'missing')])
    assert r.status == 'SKIP'
    assert r.findings and 'not found' in r.findings[0].message


@needs_git
def test_references_from_environment_and_cache_reuse(repo, tmp_path, monkeypatch):
    refs = make_refs(tmp_path, {'src/particles.c': REF_C})
    monkeypatch.setenv('SLYNX_AUDIT_REFS', refs)
    write(repo, 'x.c', DISGUISED_C)
    first = sim.run(str(repo), publishable_files(str(repo)))
    calls = []
    real = sim.fingerprint
    monkeypatch.setattr(sim, 'fingerprint', lambda text: calls.append(1) or real(text))
    second = sim.run(str(repo), publishable_files(str(repo)))
    assert first.status == second.status == FAIL
    assert len(calls) == 1  # only our own file; the reference came from the cache


def test_regions_merge_nearby_lines_and_ignore_far_outliers():
    matches = [(10, 12, 100, 102), (12, 14, 102, 104), (13, 15, 900, 901), (40, 41, 300, 301)]
    regs = sim.regions(matches)
    assert regs == [(10, 15, 100, 104, 3), (40, 41, 300, 301, 1)]
