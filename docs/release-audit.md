# Release audit

`tools/audit/audit.py` checks that nothing which must stay private reaches the public repository: game data, keys,
packaged builds, code copied from the reference projects, and third-party code without its notice or with a license
we have not approved. Run it before every push; the pre-push hook runs it for you.

```sh
python tools/audit/audit.py              # every check, exit code 0 = OK to push
python tools/audit/audit.py --only data  # one or more checks: data,similarity,notices,licenses,extra
python tools/audit/audit.py --verbose    # also list allow-listed findings and their reasons
python tools/audit/audit.py --strict     # a skipped check (no references, no cargo-deny) fails the audit
```

Needs Python 3.11 or newer and git. Exit codes: `0` nothing blocks the push, `1` something does, `2` the audit
could not run (bad arguments or a broken allow-list).

## What it looks at

"What would be published" is every file git would push: tracked files plus untracked files that are not ignored
(`.gitignore` and `.git/info/exclude`), i.e. what `git add .` would take. The data check also reads **every file in
the git history**, because a file deleted from the working tree is still pushed if an earlier commit has it.

## The checks

### data: game data, keys and binaries (hard fail)

Blocks a file when:

| Rule | Examples |
|---|---|
| Extension or name | disc images (`.iso .bin .cue .chd .img` ...), Sly worlds and game files (`.brx .s2w .s3w .s2v .wac .wal .pak` ...), game audio/video (`.vag .wav .flac .ogg .m2v .pss` ...), executables and packages (`.elf .exe .nro .nso .nsp .nca .xci .nacp` ...), archives (`.zip .7z .tar .gz` ...), keys and saves (`prod.keys`, `title.keys`, `.keys`, `.sav`), PS2 executable names (`SCUS_123.45`) |
| Content | ISO9660 volume (`CD001` at 0x8001), Sly 1 world (`PCBR`), Sly 2/3 world (`FK$X` in the first 64 bytes), Switch NRO/NSO/PFS0/HFS0/NPDM headers, PS2 (MIPS) ELF, zip/7z/rar archives |
| Secrets | console key lines (`<name with key/kek/seed> = <32 or 64 hex digits>`, `<32 hex> = <32 hex>`), private keys (`-----BEGIN ... PRIVATE KEY-----`), GitHub tokens |
| Size | any file over 5 MB |

Content checks see through renamed files. Key lines are searched in the whole file, not just its start.

Reported for review, not failed: images and fonts (could be game art or a logo) and any other binary file.

### similarity: copied reference code (hard fail on structure, report on literals)

Compares our code (`.rs .c .h .cpp .hpp .inl .glsl` ...) with the reference sources we may study but must not copy.

1. **Normalise.** Comments, whitespace and `#include`-style lines are dropped. Every identifier becomes one
   placeholder and every primitive type name another, so renaming variables does not hide a copy. Keywords,
   operators and literal values are kept.
2. **Fingerprint** with k-gram winnowing (the method behind MOSS): hashes of every 15-token window, keeping the
   smallest of every 8 consecutive hashes. Any shared run of 22 or more normalised tokens is guaranteed to share a
   fingerprint.
3. **Ignore idioms:** a fingerprint found in more than 10 reference files (a `for` loop, a getter) is not evidence.
4. **Score** each pair of (our file, reference file) by shared fingerprints:

| Result | Threshold |
|---|---|
| FAIL | 25 or more shared fingerprints (roughly 100+ tokens of identical structure), or 30% or more of the file's fingerprints when that is at least 8 |
| REVIEW | 8 or more, or 15% or more when that is at least 4 |

Matching C/C++ structure against Rust is weak (the languages look different even for the same logic), so the check
also reports **shared distinctive literals**: integers of at least 0x10000 with several different digits, floats with
five or more significant digits, and strings of 12 or more characters. These are never a failure on their own: a
format magic number or a constant from a spec is a fact we are allowed to use. Each one should be traceable to a
spec; if it is, allow-list it (below).

The reference folders are not part of the repository. Give them with `--refs DIR ...`, the `SLYNX_AUDIT_REFS`
environment variable (folders separated by `;` on Windows, `:` elsewhere), or once per clone with
`git config --add audit.reference <folder>` (relative paths are relative to the repository). Without any, the check
is skipped. Reference fingerprints are cached in the user cache folder (`%LOCALAPPDATA%\slynx-audit` or
`~/.cache/slynx-audit`; override with `SLYNX_AUDIT_CACHE`), so later runs only re-read changed files.

### notices: third-party notices (hard fail)

Every third-party crate in every `Cargo.lock` (packages with a `source`, i.e. not our own workspace crates) must be
named in `THIRD_PARTY_NOTICES.md`. Passes when there is no `Cargo.lock` yet.

### licenses: dependency licenses (hard fail, or skipped)

Runs `cargo deny check licenses` with `deny.toml` (the license allow-list). Skipped, with a message, when
`cargo-deny` is not installed or the workspace has no crates yet. Install it with
`cargo install --locked cargo-deny`. The allow-list in `deny.toml` is provisional until legal review confirms it.

### extra: optional extra checks

Extra check modules, each a Python file defining `run(repo, files)` that returns a
`slynx_audit.common.CheckResult`, can be added per clone without changing the audit: list them in the
`SLYNX_AUDIT_EXTRA` environment variable or with `git config --add audit.extraCheck <file>`. A module that is
listed but missing, or that crashes, fails the audit.

## Reading a report

```
== data: FAIL (41 files to publish, 12 files in history: 1 blocked, 1 to review)
  FAIL    tests/fixtures/level.dat: Sly 1 world (PCBR)
  REVIEW  docs/img/menu.png: image or font: make sure it is our own work, not game art, box art or a logo
== similarity: FAIL (13 code files against 6880 reference files; k=15, w=8; ...)
  FAIL    crates/fx/src/particles.rs: shares 31 of 80 fingerprints (39%) with project/src/particles.c
           crates/fx/src/particles.rs:14-52  ~  project/src/particles.c:120-161  (27 fingerprints)
  REVIEW  [literals] crates/formats/src/world.rs: line 9: literal 1129332290 (0x434F4C42) also in project/src/world.cpp
== AUDIT FAILED: data, similarity
```

- One `==` line per check: its status (`PASS`, `FAIL`, `WARN` = only things to review, `SKIP`) and a summary.
- `FAIL` lines block the push. `REVIEW` lines do not, but each one needs a look before a release.
- Similarity findings list up to five matching regions: our lines `~` the reference lines. Open both and compare.
  The tool only finds candidates; a person decides whether it is a copy.
- The last line is the verdict; the exit code matches it.

## What to do with a finding

- **Game data, keys, packages:** remove the file, add a `.gitignore` rule, and if it was ever committed, rewrite the
  history before pushing (an ignored file is still in old commits).
- **Structural match:** rewrite the code from the spec, without looking at the reference. If the match is code that
  can only be written one way (a required API call sequence, a three-line shader), allow-list it with that
  explanation.
- **Shared literal:** make sure the value is in a spec, with its `source:` line, then allow-list it.
- **Missing notice:** add the crate, its version and license to `THIRD_PARTY_NOTICES.md`.

## False positives: the allow-list

`tools/audit/allowlist.toml` lists findings that were reviewed and are fine. Each entry needs a reason; the audit
refuses to run (exit 2) on an entry without one, and reports entries that no longer match anything.

```toml
[[allow]]
check  = "literals"                     # data | similarity | literals | notices | licenses | extra
path   = "crates/formats/src/world.rs"  # glob on the finding's path
match  = "1129332290"                   # optional glob on the finding's key (see below)
reason = "World file magic number, from specs/formats/world.md (a format fact, not code)."
```

`match` is compared with: the reason text (`data`), the reference file shown in the report (`similarity`), the
literal's value as printed in decimal (`literals`), the crate name (`notices`). Keep entries narrow: one file, one
reference or one value, never `path = "*"` for a failing check. Changes to the allow-list are reviewed like code.

## The pre-push hook

`tools/hooks/pre-push` runs the audit and stops the push if it fails. It is not installed automatically; once per
clone, from the repository root:

```sh
cp tools/hooks/pre-push .git/hooks/pre-push
chmod +x .git/hooks/pre-push      # not needed on Windows
```

Arguments for the audit can be passed through `SLYNX_AUDIT_ARGS` (for example `SLYNX_AUDIT_ARGS=--strict`). The
hook audits the working tree and the whole history, which covers what is being pushed. Do not bypass it with
`--no-verify`; fix or allow-list the finding instead.

## Tests

```sh
python -m pytest tools/audit/tests
```

The tests build their own fake files (fake ISO headers, fake key lines, made-up "reference" code) in temporary
folders. Never add real game files or real reference code to the tests.
