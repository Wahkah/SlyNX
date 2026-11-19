# Sources used by the specs

Required by the clean-room protocol (`docs/legal/CLEAN_ROOM.md`, sections 1 and 3). This file lists every outside
source the reference side has used to write specs: reference projects, public standards and hardware manuals.
Each spec's `sources:` front matter names the exact source and what it was used for; this file is the index.

Rules:

- Add a project or document here **before** a spec cites it, with the commit (or edition and date) that was read.
- If a newer commit is read later, add it as a new line; never overwrite the old one.
- Only the reference side reads the reference projects. The implementation side reads the specs and the public
  standards, never the projects' source.
- A license listed here does not mean anything was copied. Specs carry facts only; see `docs/legal/LICENSING.md` for
  what each license would allow.

Last updated: 2026-11-13.

## Reference projects

| Project | Public URL | Commit read (date) | License | Used for |
|---|---|---|---|---|
| ProjectCane | https://github.com/theclub654/ProjectCane | `f77825d6` (2026-10-22) | none (all rights reserved) | First reading of Sly 1 formats and behaviour: world-file sections and decompression, object records and option lists, geometry, textures, animation, collision, physics, player moves, guards, alarms, effects, audio, dialogue, movies, scripts, disc layout. Every format fact was re-checked on the disc; behaviour facts are marked where they rest on the reference alone. Cited by nearly every spec in `specs/sly1/`. |
| SafeHouse | https://github.com/FridiNaTor1/SafeHouse | `c70803ce` (2026-11-12) | none for its own code (all rights reserved) | Retail (disc) byte layouts where they differ from ProjectCane's converted PC format: object, option, curve, geometry and visibility-map layouts, which classes share a layout, the packed-colour option sizes. Cross-check that a walker reaches the texture block on every world. `object-stream.md`, `option-encodings.md`, `worlds.md`, `classes/*.md`. |
| SlyTools | https://github.com/VelocityRa/SlyTools | `63987211` (2024-11-14) | MIT (Copyright (c) 2020 Nick Renieris) | Confirmation only: the decompressor's 8 KiB ring window (`world-file.md`) and that the WAC/WAL archive pair belongs to other releases (`disc.md`). |
| sly-rs | https://github.com/jadzziaa/sly-rs | `5777345d` (2026-11-05) | GPL-3.0-or-later | Its research notes only: the disc survey (cross-check of image and executable hashes, `disc.md`) and the memory-card file list (`pickups-and-progress.md`). |
| sly2-decomp (OpenThievius) | https://github.com/OpenThievius/sly2-decomp | `33e2bab9` (2026-11-12) | GPL-3.0-or-later (tooling); its `src/` and `nonmatching/` are the game owners' code | Not cited by any spec yet. Reserved for the Sly 2 phase. |
| Sly-2-Recomp | https://github.com/menaman123/Sly-2-Recomp | `b692aa3e` (2026-11-03) | Apache-2.0 (its NativeEE engine submodule: MIT, unverified) | Not cited by any spec yet. Reserved for the Sly 2 phase. |

## Public standards and manuals

| Document | Edition | Used for | Cited by |
|---|---|---|---|
| ECMA-119 (ISO 9660), volume and file structure of CD-ROM | as cited; edition to be recorded | Volume descriptor and directory record layout | `disc.md` |
| Sony "GS User's Manual" (PlayStation 2 Graphics Synthesizer) | as cited; version to be recorded | Pixel storage formats PSMCT32, PSMT8, PSMT4, CLUT storage mode CSM1, alpha blending | `textures.md` |
| PlayStation SPU/SPU2 hardware documentation | as cited; document and version to be recorded | ADPCM block format, ADSR register layout, 48 kHz voice clock | `audio.md` |
| Standard MIDI File 1.1 | 1.1 | Variable-length quantities, channel messages, meta events | `audio.md` |
| ISO/IEC 13818-1 / ITU-T H.222.0, MPEG-2 systems (program stream) | as cited; edition to be recorded (the ITU edition is the one to obtain) | Pack, PES and stream header parsing | `movies.md` |
| ISO/IEC 13818-2 / ITU-T H.262, MPEG-2 video | as cited; edition to be recorded (the ITU edition is the one to obtain) | Video header parsing; the in-house decoder (`docs/legal/MOVIE_PLAYBACK.md`) | `movies.md` |

"As cited" means the spec names the document but not the edition. Record the edition and date here when the
document is next used.

## The game

All format facts are checked against one disc: *Sly Cooper and the Thievius Raccoonus*, USA, SCUS-97198, image
SHA-256 `b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a`; executable `SCUS_971.98` SHA-256
`d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c`. Hashes identify the disc; nothing from it is in
the repository. Observations of the game as played are recorded in each spec as `kind: observation`.
