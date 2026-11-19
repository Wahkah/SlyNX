# Clean-room protocol

Status: draft, 2026-11-13, pending the owner's sign-off. Not legal advice; see the **[lawyer]** marks.

## Why

Copyright protects expression (code, comments, the particular way a program is written), not facts, ideas, file
formats or methods (17 U.S.C. section 102(b); EU Software Directive 2009/24/EC, recital 11 and article 1(2)). Courts
have accepted reverse engineering to learn unprotected facts for interoperability (*Sega v. Accolade*, 9th Cir. 1992;
*Sony v. Connectix*, 9th Cir. 2000; EU Directive article 6 for decompilation). A clean room is how we show that
SlyNX's code was written from facts, not copied: the people who read protected code never write SlyNX code, and the
people who write SlyNX code never read protected code. Written specs are the only thing that crosses between them.

The protocol only works if it is documented while it happens. A record that is reconstructed later is worth much
less.

## 1. Roles and what each may read

| Role | May read | May not read | May write |
|---|---|---|---|
| **Reference side** | The reference projects (ProjectCane, SafeHouse, sly2-decomp, Sly-2-Recomp, sly-rs and others listed in `specs/SOURCES.md`); the game's executable and its disassembly; the game's data files; public docs; SlyNX specs and code | (nothing restricted) | Specs in `specs/`, test vectors, format notes. **Never** SlyNX code in `crates/` |
| **Implementation side** | SlyNX specs and code; public documentation (Rust, libnx, deko3d, PS2 hardware manuals, open standards); the game **running** (on a PS2 or in an emulator), including screenshots, video, audio and frame dumps; the output of SlyNX's own tools on a disc they own | Any reference project's source, notes or issue threads with code; the game's executable or its disassembly; decompiler output; anyone's code from a Sly game port | Code in `crates/`, tools, tests |
| **Code review** | Everything (to check for copying) | | Review comments. A reviewer who reads reference code is from then on reference side and does not write SlyNX code |
| **Release audit** | Everything | | Audit reports, automated checks. Not SlyNX code |
| **Legal review** | Everything | | `docs/legal/`, notices |

Rules that follow from the table:

1. **A person is on one side per subsystem, and the reference side is permanent.** Someone who has read reference
   code for a subsystem (say, the renderer of Sly 1) cannot later implement that subsystem. They can implement an
   unrelated subsystem only if the legal review agrees and the record says so. When in doubt, they stay on the
   reference side.
2. **Prior exposure counts.** Anyone who worked on a ProjectCane- or SafeHouse-based build (porting, debugging,
   optimising its code) is reference side for every subsystem they touched in it. Their lessons can still reach the
   implementation side, but only as a spec written in the normal way (section 2), not by them writing the code.
3. **The owner** directs both sides and approves releases. If the owner has read reference code for a subsystem, the
   owner does not write code for it.
4. **The roster** (who is on which side, per subsystem, and since when) is kept by the owner and updated before
   anyone starts work. A spec or change that does not match the roster is rejected.
5. **No shared scratch space.** Reference-side notes, dumps and checkouts live outside the SlyNX tree, in
   separate folders that are never inside or next to the repository checkout. Implementation-side tools and editors are not pointed at those folders,
   and search tools run inside `SlyNX/` only.
6. **No back channel.** Questions from the implementation side go to the reference side in writing (a spec issue);
   answers come back as a spec change. No pasting snippets in chat, no "just look at line 400".

## 2. What a spec may and may not contain

Specs live in `specs/`, one Markdown file per topic (for example `specs/sly1/formats/brx.md`,
`specs/sly1/behaviour/guard-alert.md`).

**A spec may contain (facts):**

- File and data formats: field order, sizes, offsets, types, endianness, meanings, valid ranges, compression and
  encoding schemes, written as tables or prose.
- Behaviour, described from the outside: what happens, in what order, under which conditions; state machines as
  diagrams or tables of states, events and transitions; timings in frames or seconds.
- Constants and tuning values (speeds, gravity, radii, timer lengths), each with where it was measured or read.
- Formulas in mathematical notation (`v' = v + g * dt`), not in a programming language.
- Hardware facts (PS2 GS/VU behaviour that the data depends on) and how to map them to modern graphics.
- Test vectors: inputs and expected outputs, kept small. Prefer hashes, sizes, offsets and a few decoded values over
  raw bytes copied out of the game (see [DISTRIBUTION.md](DISTRIBUTION.md)).
- Names, where a name is needed to talk about a thing: names that appear in the game's own data (file names, string
  tables) and plain descriptive names chosen by the spec writer.

**A spec may not contain (expression):**

- Code from any reference project or from the game: no function bodies, no snippets, no struct or class definitions
  in a programming language, no copied comments.
- Pseudo-code that follows a reference function line by line, or that reproduces its control flow, its local
  variables, or its order of operations where that order is a choice of the original author rather than a fact of
  the behaviour.
- The reference's (or the game's) internal architecture as a blueprint: class hierarchies, module layout, function lists and call
  graphs, in-memory struct layouts of the engine (as opposed to on-disc formats). Describe what the game does, not how
  the reference code is organised. Exception: an in-memory layout that the game's data or save files depend on, which
  is then a data format.
- Names taken from a reference project or a decompilation's symbols (class, function, field or type names), whether
  as headings, aliases or cross-reference columns. Identify things by the game's own identifiers (numeric IDs, file
  names, strings stored in the data) and give them descriptive names of our own. Layout, format and section names in
  specs are ours too. A reference-side writer who needs a mapping to the reference's names keeps it outside the
  repository.
- The engine's class hierarchy (which class derives from which). Say which on-disc layout a class uses, and describe
  shared behaviour where it is observed.
- Large tables copied out of the game's executable. If the port needs a table that exists as data in the
  executable, the spec describes where it is and how to read it, and the port reads it from the player's own disc
  (see "Tables stored in the executable" below).
- Links that would lead the implementation side into reference code ("see `foo.cpp` in ProjectCane").

Test for borderline cases: could someone who has never seen the reference produce this sentence by watching the game
and examining its data? If yes, it is a fact. If it only makes sense as a description of someone's code, leave it out.

**Tables that only exist in the game's code.** Some tables (for example the value size of each option ID) are built
by the game's code and cannot be read from any data file. The individual entries are facts the data format depends
on. Copying the whole table from a reference is not allowed, though. A spec may hold such a table under these
conditions:

1. **Only what the data needs.** List only the entries that occur in the game's data on the supported discs. Entries
   that occur nowhere in the data are left out, and the loader treats them as corrupt input.
2. **Every entry confirmed by the game itself**, independently of the reference: for example, by a walk of every
   file that would fail if the entry were wrong. A reference may supply a first guess, recorded in `sources:`, but an
   entry that cannot be confirmed this way stays out.
3. **Our own presentation.** The layout of the table (codes, grid, order, notes) is ours, not the reference's.
4. **Say what is confirmed and what is not.** Where the game confirms only part of an entry (say, its size but not
   whether 4 bytes are a float or an integer), the spec says the rest is a working assumption to be settled by the
   spec that uses it.

**Tables stored in the executable.** When a table exists as data in the game's executable (for example an
animation lookup list, a per-class option map, friction tables, the movie list), the spec does not copy it. It
describes it by location, and the port reads it from the player's own disc, when packing or at run time. This is
the preferred way to handle any table the game stores, and it is allowed under these conditions:

1. **Tied to a known executable.** The spec names the executable (file name and SHA-256) the address belongs to,
   and gives the virtual address and the file offset, the record layout, the record count, and how to resolve any
   pointers in the records. The port checks the executable's hash (or an equivalent signature) before reading, and
   refuses or reports an unsupported disc on a mismatch; it never guesses an address. Addresses for other regions or
   versions are separate spec entries, each with its own hash.
2. **Check values, not contents.** The spec may give a few entries (the first one or two records, a count, a
   checksum of the table) so the reader can be tested. It does not reproduce the table.
3. **Read as data only.** The port uses these tables as data: it reads records and follows pointers to other data.
   It never loads, translates or runs the game's code. (Running or recompiling the game's code would be a different
   kind of project and needs its own legal review.)
4. **Stays on the player's machine.** A table read this way is game data: it may be in the player's own build, but
   never in the repository, test fixtures, CI artifacts, logs or published builds (see
   [DISTRIBUTION.md](DISTRIBUTION.md)). Tests that need it read the disc from a path given at run time.
5. **Sources recorded** as usual: how the location was found (reference side) and how it was confirmed against the
   disc (for example, every lookup result matches an object that exists in the worlds).

An address, a record layout and a count are facts about the executable; they carry no expression from the game or
from a reference. Reading the table from the player's disc means none of the game's data is copied into SlyNX.

**Minimum necessary.** In general, a spec carries only what the port needs and what the game confirms. Material
whose only source is a reference project, and that the port does not need, stays out even if it is a fact.

## 3. How sources are recorded

Every spec starts with a front-matter block:

```yaml
---
id: sly1-formats-brx
title: BRX level file format
side: reference
status: draft | reviewed | approved
sources:
  - kind: disc            # disc | executable | observation | reference-project | public-doc
    what: <disc serial and region>, file <name>.BRX, sha256 <hash>
    used_for: header layout, section table
  - kind: reference-project
    what: ProjectCane @ f77825d6, read for the order of the header fields
    used_for: confirmation only; values re-checked against the disc
  - kind: observation
    what: PCSX2 run, frame dumps of level load
    used_for: when textures appear
reviewed_by: <role / roster id>
date: 2026-11-13
---
```

- `kind: reference-project` entries name the project and the exact commit. The fact must also be checked against the
  game itself where possible, and the spec says so.
- `specs/SOURCES.md` lists every reference project with its URL, the commit we read, its license and the date.
- Implementation commits and pull requests name the spec(s) they implement (`Spec: sly1-formats-brx`). Code with no
  spec behind it must be something that needs none (platform code, generic engine code, tools) and the change says so.
- Spec history is kept in git like code. Specs are never rewritten to hide where a fact came from.

## 4. What reviewers check

**Spec review (before a spec is marked `reviewed`):**

- No code, no line-by-line pseudo-code, no copied comments or identifiers in bulk (section 2).
- Every fact has a source line; reference-project facts are confirmed against the game where possible.
- Test vectors are small and do not reproduce game assets.
- No names from the references; tables built by the game's code meet the four conditions in section 2.

**Code review (every change):**

- The change names its spec, or explains why it needs none.
- The author is implementation side for that subsystem on the roster.
- Nothing looks transcribed: unusual identifier names that match a reference, the same quirky constants in the same
  order, the same comments or string literals, a structure that mirrors a reference function, workarounds for bugs
  the author could not have known about from the spec.
- `unsafe`, performance and correctness as usual.

**Release audit (before every push and release):**

- Automated similarity checks of `crates/` against each reference tree: shared string literals and comments,
  identifier overlap (rare names), constant sequences, and token-level similarity for C/C++ files (the C shim). Rust
  against C++ is hard for tools to compare, so hits are leads for a human, and a clean result does not prove much
  alone.
- Every spec that the release's code names is `reviewed` or `approved`.
- `THIRD_PARTY_NOTICES.md` is up to date, `cargo deny` passes, and no game data, keys or packaged builds are in the
  tree (see [DISTRIBUTION.md](DISTRIBUTION.md)).

## 5. When contamination is found

Contamination means reference-side expression reached SlyNX code or specs: copied or translated code, a spec with
code in it, or an implementer who read reference code for the subsystem they implemented.

1. **Stop.** The change is not merged, or is reverted if already merged. Nothing is pushed until this is resolved.
2. **Record it** in `docs/legal/contamination-log.md` (create on first use): date, what was found, which files,
   which reference, how it got in, who found it. Keep the record even after the fix.
3. **Quarantine.** Remove the affected code and every change built on it. If a spec was the carrier, withdraw the
   spec and have it rewritten by a reference-side writer who follows section 2.
4. **Reassign.** The person exposed becomes reference side for that subsystem on the roster.
5. **Rewrite** the affected code from the corrected spec, by an implementer who never saw the contaminated code or
   the reference. The rewrite is reviewed with the contamination entry in hand.
6. **If it was already public:** remove it in a new commit right away and say so in the log. Rewriting public git
   history does not remove copies that others already fetched or forked, so it is not a substitute for the steps
   above; whether to rewrite history anyway is a decision for the owner. **[lawyer]** if the copied material is
   substantial.
7. **If a takedown notice (DMCA or similar) arrives:** do not argue in public. Comply with the platform's process,
   keep the records, and get a lawyer before filing any counter-notice. **[lawyer]**

## 6. Practical notes

- The reference projects are pinned: record the commit you read. If a reference later adds a license, that does not
  change our process, but the legal review may re-evaluate what can be reused.
- Public standards (MPEG-2, ISO 9660, PNG, SHA-256 and so on) are implemented from the standard documents. The
  tables a standard defines (code tables, quantiser matrices, constants) may be used as the standard gives them.
  Other projects' implementations of the same standard (FFmpeg, libmpeg2, PCSX2's IPU, the codecs inside the
  reference projects) are not read by whoever writes ours: their code carries its own license (often GPL or LGPL),
  and the similarity audit compares against them. Running such a tool as a test oracle (to produce reference output
  to compare with) is fine; reading its source is not.
- Emulator use: running the game in an emulator to observe it is fine for both sides. Reading the emulator's source
  is reference side (most are GPL), but is rarely needed.
- Specs are part of the public repository, so they must be publishable: no game data beyond the small test vectors
  described above.
