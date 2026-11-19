# Contamination log

Kept under the clean-room protocol ([CLEAN_ROOM.md](CLEAN_ROOM.md), section 5). Every incident stays in this file
after it is resolved. Newest first.

## 2026-11-13: reference-derived names in implementation code

**Status:** remediation in progress. Nothing has been published.

**Found by:** the code review of the first implementation phase, 2026-11-13.

**What was found.** Identifiers and terms that come from the reference projects' naming, not from the specs'
neutral terms, in implementation code and its docs:

- helper and type names `GEOM`, `BSPC`, `TBSP` (geometry and collision-tree records);
- the terms "three-way", "pre-lit", "murk", "projected volume", "gleam", "warp", and "background / foreground /
  world map" (texture, lighting, effect and world-kind terms).

Affected areas: `crates/formats`, `crates/render`, `crates/viewer` and the implementation docs (`docs/formats.md`).

**What was not found.** The code review found no code, pseudo-code or comments from a reference project in the
renamed areas. The leak is limited to names. (The separate similarity findings in `crates/formats/src/iso.rs` and
`crates/render/src/png.rs`, against sly-rs, are not part of this incident and are tracked by the code review.)

**How it got in.** Early drafts of the specs, written before the naming rule (CLEAN_ROOM.md section 2, added
2026-11-13), gave the reference projects' short names as aliases (for example "Alias SO") and used the reference's
names for record kinds and render passes. The implementers read those specs as intended and used the names they
found there. The implementers did not read reference source. This is a spec-side leak, and the protocol gap (no rule
against reference names in specs) has since been closed.

**Assessment.** Short names and words are not protected by copyright on their own (37 CFR 202.1(a)), so the legal
exposure is low. They still matter: they show a path from the references into our code, they make our code look
derived from the references, and they defeat the audit's name check. The implementers stay on the implementation
side; nobody's role changes, because no reference expression was read.

**Remediation.**

1. Specs: the reference aliases and the reference's kind and pass names are removed from the specs, which now use
   neutral names of their own (done, 2026-11-13).
2. Code and docs: the affected identifiers are renamed to the current specs' neutral terms in `crates/formats`,
   `crates/render`, `crates/viewer` and the docs (in progress, 2026-11-13).
3. Audit: the release audit checks names as well as code fingerprints, including the names listed above.
4. Review: the code review checks the renamed code against the current specs before the incident is closed.

**Open points.**

- At the time of writing, the tree still has `GEOM`, `BSPC` and `TBSP` in `crates/formats/src/stream.rs`,
  "three-way" in `crates/formats/src/textures.rs`, `docs/formats.md` and `specs/sly1/effects.md`, "gleam" in
  `crates/formats/src/geometry.rs`, and "warp" and "world map" in code and in several specs.
- Plain English words that describe what a player sees in the game (a map screen, a warp between places) may stay
  in the specs if the spec writer would use them having never seen a reference. A term used only because a
  reference uses it is replaced. The spec review decides per term and records the choice here.

**Code review, 2026-11-14.** `GEOM`, `BSPC`, `TBSP`, "three-way", "pre-lit", "murk", "gleam" and "projected
volume" no longer appear in `crates/formats`, `crates/render`, `crates/viewer` or the docs. Plain-word decisions:
"warp" (a point that moves the player to another place) and "world map" (the map screen) are kept as plain English
for what the player sees. Still open before closing: the new draft `specs/sly1/rendering.md` uses names that match
a reference project's field names (`uShadow`, `uMidtone`, `selfIllum`); the spec review replaces them with
descriptive names before that spec is published. The engine module name `geom` (geometric primitives) is a generic
abbreviation and was not taken from a reference record name; renaming it is optional.

**Closed when:** none of the names above remain in code or docs, except plain words approved under the open point
above; the audit's name check passes; the code review has signed off. Record the closing date and the commit here.
