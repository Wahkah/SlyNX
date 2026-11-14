# What may be distributed

Status: draft, 2026-11-13. Not legal advice; **[lawyer]** marks the open questions.

"Distribute" means anything that leaves the owner's machine: a git push, a GitHub release, an attachment in an issue,
a file sent to a tester, a Discord upload.

## 1. May be published

| What | Conditions |
|---|---|
| SlyNX source code (`crates/`, the C shim, tools, tests) | Clean-room rules followed ([CLEAN_ROOM.md](CLEAN_ROOM.md)); license files and notices present |
| Specs (`specs/`) | Facts only, no code from references; test vectors small (section 4) |
| The packager (source and prebuilt PC binaries) | Contains no game data, no keys and no key material; ships with `THIRD_PARTY_NOTICES.md` and the crate notices; does not bundle hacBrewPack unless its GPL-2.0 obligations are met |
| Builds of our own code: a PC executable, an `.nro` without game data (for example the "hello" test, or the engine with an empty RomFS that loads the player's converted data from the SD card) | No game data, nothing derived from it, no keys; notices included; neutral icon |
| Original project art (logo, default icon) | Made for the project, no Sony or Nintendo marks or characters ([TRADEMARKS.md](TRADEMARKS.md)) |
| Documentation of how to use the packager with **your own** disc | Must not explain how to obtain games, keys or firmware from other people, and must not link to key-dumping tools or download sites (section 5) |
| Hashes of retail files (to identify a supported disc) | Hashes are fine; the files are not |

## 2. May never be published

| What | Why |
|---|---|
| Disc images (ISO, CHD, BIN/CUE) or any file extracted from a disc | Copyright of Sony / Sucker Punch |
| **Converted** game data (re-encoded textures, models, audio, video, levels in our formats) | Still a copy or derivative of the game's assets, whatever format it is in |
| The game's executable, disassembly, decompiler output, or tables copied out of it in bulk | The game's code |
| Any `.nro` whose RomFS (or embedded data) contains game data, converted or not | Same as above; a packaged copy of the game |
| Any `.nsp`/`.nca` made from game data | Same, plus key-signed content (next row) |
| Switch keys (`prod.keys`, `title.keys`, any individual key value, including in source code, tests, CI secrets or docs) | Nintendo's; distributing them raises anti-circumvention issues (17 U.S.C. section 1201, EU InfoSoc Directive article 6) **[lawyer]** |
| PS2 BIOS, Switch firmware or system files | Sony's / Nintendo's; SlyNX needs none of them |
| Builds based on ProjectCane or SafeHouse (for any platform), and patches against those projects | No license ([LICENSING.md](LICENSING.md)); they also embed game logic from the original games |
| Save files from the retail game | Usually harmless, but they are game data; keep them out of the repository |
| Screenshots or videos of the game in the repository | Copyrighted game content; see [TRADEMARKS.md](TRADEMARKS.md) section 5 |
| Official logos, box art, fan art or cover art depicting the characters | Trademarks and copyright of Sony ([TRADEMARKS.md](TRADEMARKS.md)) |

The rule is the same for a "private" share with a friend or tester: sending a build with game data in it is
distribution. Testers make their own builds from their own discs.

## 3. What a player's own build may contain

The packager runs on the player's machine and converts **the player's own disc**. The output may contain:

- The player's game data, converted, in the `.nro` RomFS or next to it on the SD card.
- The player's own `prod.keys`, used locally by the NSP path and never copied into the output folder, logs or crash
  reports.
- An icon and title the player chooses, including their own art. The defaults are the neutral project icon and a
  plain title.

The player's build is for the player's own use. The packager's README and its output screen say so:

> This build contains data from your own copy of the game. It is for your personal use. Do not share it.

Whether making a personal format-shifted copy is lawful depends on the country (the US has no general private-copy
exception, but fair use and 17 U.S.C. section 117 may apply; several EU countries have private-copy exceptions) and on
how the disc was obtained. We do not advise players on this; we only require that they use their own copy.
**[lawyer]**

The packager itself:

- Reads a PS2 disc image the player supplies. PS2 game data on disc is not encrypted, so converting it circumvents
  no access control.
- **NRO path:** needs no keys. This is the primary, lower-risk path.
- **NSP path:** needs the player's own `prod.keys` for hacBrewPack to build the NCA. We never ship keys, never
  explain where to get them beyond "dumped from your own console", never link to dumping tools, and never accept or
  bundle keys in issues or CI. Nintendo's 2024 suit against the Yuzu developers leaned heavily on key handling, so
  this path carries more risk than the NRO path. Consider shipping it later, or not at all, after a lawyer's view.
  **[lawyer]**
- Uses a homebrew title ID, never an official Sony or Nintendo title ID.
- Writes logs and crash reports that contain no game data and no key values.

## 4. Test vectors and fixtures

- Fixtures checked into the repository are made by us (synthetic files in the game's format, written by a test
  helper) or are a few bytes or values with no expressive content (a header, a count, an offset, a hash).
- Tests that need real game data read it from a path given at run time (`SLYNX_DISC=/path/to/disc.iso`) and are
  skipped when it is missing. CI runs without game data; any job that needs a disc runs only on a contributor's own
  machine.
- Image-comparison references produced from the game (screenshots, frame dumps) stay outside the repository.

## 5. Things never to write in tracked files

- Where to download games, ISOs, BIOS, firmware or keys.
- Instructions to bypass a console's protections beyond pointing to the platform's homebrew community docs in
  general terms.
- Any key value, even a partial one, even "for testing".

## 6. Release checklist (the release audit runs this)

1. `python tools/audit/audit.py` passes: no disc data, packages (`.nro`, `.nsp`, `.nca`, `.nso`), keys or large
   binaries in the tree or in the release assets.
2. Release binaries are built from a clean checkout with no game data on the build machine's include paths.
3. `cargo deny check` passes; `THIRD_PARTY_NOTICES.md` matches the dependencies; crate notices are generated.
4. Release notes carry the disclaimer from [TRADEMARKS.md](TRADEMARKS.md).
5. The owner has approved the release.
