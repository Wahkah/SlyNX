---
id: sly1-classes-base-object
title: Sly 1 object stream, base layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layout verified by full walks; class counts
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files lo, brx
    used_for: layout and the stock-object rule
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker, CidLoaders.generated.inc)
    used_for: which classes use this layout in the retail format
date: 2026-11-13
---

# Base layout (layout key **base**)

The simplest object body: an option list and nothing else. Objects with this layout have no transform and
nothing to draw.

## Layout

After the 6-byte object header (object-stream.md, section 5.1):

| Order | Field | Type |
|---|---|---|
| 1 | options | option list (object-stream.md, section 4) |

That is the whole body.

## Classes that use exactly this layout

Only classes that occur on the USA disc are listed; any other class ID is corrupt input (object-stream.md 6).

Only classes that occur on the USA disc are listed; any other class ID is corrupt input (object-stream.md 6).

| ID | Neutral name | Records on the disc |
|---:|---|---|
| 135 | freeze group ? | 12 |
| 137 | guard group ? | 5 |
| 140 | object variant ? | 61 |
| 141 | object variant ? | 67 |
| 143 | object variant ? | 60 |
| 144 | rhythm sequence ? | 4 |
| 145 | lightning flash ? | 77 |

These classes hold behaviour configured by options and scripts only. None has geometry; a static renderer can parse
and ignore them.

## Common to every class

These facts hold for every record whatever its layout:

- The header's object ID and script index (object-stream.md, section 5.1) belong to the object.
- If the record is loaded through an object list and its OID is 12 to 42, it is a stock template: it is removed
  from the world after loading and kept in template slot OID − 12 (object-stream.md, section 7.1).
