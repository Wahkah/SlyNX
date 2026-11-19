# The packager (`slynx-pack`)

`slynx-pack` runs on the player's PC. It takes the player's own disc image and the SlyNX program, and writes a
Switch homebrew package (`.nro`) on the player's machine. The program itself contains no game data; the game data
in the package comes only from the player's disc, and the package never leaves their machine. See
[legal/DISTRIBUTION.md](legal/DISTRIBUTION.md), section 3.

> This build contains data from your own copy of the game. It is for your personal use. Do not share it.

(The packager prints this after every package it makes.)

## Use

```sh
slynx-pack <game.iso> --out <dir> [--program <slynx.elf|.nro>] [--icon <256x256.jpg>] [--title <text>]
           [--layout auto|romfs|sd]
slynx-pack --identify <game.iso>              # which disc is this?
slynx-pack --inspect <file.nro>               # check a package, list its RomFS
slynx-pack --write-icon <out.jpg> [--number N]  # the default icon, to look at or edit
```

The package goes to `<dir>/switch/SlyNX/`, laid out like the SD card: copy the `switch` folder to the root of the
card and start `slynx-sly1.nro` ("SlyNX 1") or `slynx-sly2.nro` ("SlyNX 2") from the Homebrew Menu.

| Option | Default | Meaning |
|---|---|---|
| `--out <dir>` | (required) | output folder |
| `--program <file>` | `slynx-game.nro`, then `slynx-game.elf`, next to `slynx-pack` | the SlyNX program: the linked ELF, or an `.nro` of it with no game data (its assets are replaced) |
| `--icon <file>` | the generated SlyNX icon | the player's own 256x256 JPEG |
| `--title <text>` | `SlyNX 1`, `SlyNX 2` (`SlyNX 3` later) | title in the Homebrew Menu |
| `--layout` | `auto` | where the disc image goes (next section) |
| `--nsp --keys <file>` | | NSP output: not supported yet (below) |

Exit status: 0 on success, 1 on an error (message on stderr), 2 on a usage error.

## What it does

1. **Identify the disc.** Read `SYSTEM.CNF` from the ISO 9660 image, take the boot executable named on its `BOOT2`
   line, and hash it (SHA-256). The table in `crates/packager/src/game.rs` maps hashes to releases:

   | Release | Boot file | Executable SHA-256 | Status |
   |---|---|---|---|
   | Sly Cooper and the Thievius Raccoonus (USA) | `SCUS_971.98` | `d4ba3ce3...b859796c` (from `formats::disc::EXECUTABLE_SHA256`) | supported |
   | Sly 2: Band of Thieves (USA) | `SCUS_973.16` | `a511a94b...2fec6a73` (from `formats_sly2::exe::EXECUTABLE_SHA256`) | supported |
   | Sly 3: Honor Among Thieves (USA) | `SCUS_974.64` | from `formats_sly3::exe::EXECUTABLE_SHA256` | supported |

   A known boot file with a different hash is reported as "a different revision"; anything else as an unknown disc.
   Only hashes are stored, never game bytes.
2. **Choose the layout.** Sly 1 keeps its game data in sectors that no directory record points at: the executable
   holds tables of sector numbers and sizes (see `specs/sly1/disc.md`). The simplest correct package is therefore
   the **whole disc image**, which the game opens through the same `formats` disc API (`Disc::open`) as the PC
   tools. Trimming the image would mean rewriting those tables, and splitting out movies or music would mean the game
   reads them from other files; neither is needed today (below).
   - `auto`: the image goes into the `.nro`'s RomFS as `sly1.iso` if the `.nro` stays within 4 GiB - 1 byte, the
     largest file FAT32 holds; otherwise it is copied next to the `.nro` as `switch/SlyNX/sly1.iso`.
   - `romfs`: always inside (fails if the `.nro` would be too large).
   - `sd`: always next to it.

   The game (the viewer for now) looks for `sdmc:/switch/SlyNX/sly1.iso` first, then `romfs:/sly1.iso`
   (`docs/viewer.md`). A Sly 2 package holds `sly2.iso` instead (or puts it at `switch/SlyNX/sly2.iso`); the Sly 2
   game program (`crates/game/tools/build-switch-sly2.sh`) looks for it in those two places. Sizes of the USA discs:
   Sly 1 is 2.80 GB, so its `.nro` is 2.80 GB; Sly 2 is 2.88 GB (its `.nro` 2.88 GB); Sly 3 is 3.55 GB, which also
   fits. A disc image over 4 GiB could not go in either place on FAT32 and is
   refused; splitting such an image into parts would need the game to read the parts, which none of the three discs
   requires.
3. **Build the RomFS** (`romfs.rs`): `sly1.iso` (streamed from the image file, never loaded into memory) and
   `slynx/package.txt`, a small text manifest: format version, game id, executable SHA-256, where the image is,
   its size, packager version.
4. **Write the `.nro`** (`nro.rs`): the program image, the asset header, the icon, the NACP and the RomFS, into
   `<name>.nro.part`, renamed to `<name>.nro` once complete.

### Formats written

All in pure Rust, so the player needs no devkitPro tools:

- **NRO** (layouts from libnx's `nro.h`): the program's three loadable ELF segments (code, read-only data, data)
  at their addresses, each padded to 4 KiB; the NRO header (`NRO0`, image size, segments, bss size, GNU build ID)
  in bytes 0x10..0x80 of the code, which libnx's startup code reserves; then the asset header (`ASET`) with icon,
  NACP and RomFS back to back. An `.nro` given as `--program` keeps its image and gets new assets.
- **NACP** (libnx `NacpStruct`, 0x4000 bytes): title and author in the first 12 language entries, the version, and
  the same control fields devkitPro's `nacptool --create` sets for homebrew.
- **RomFS** (libnx `romfs_header`/`romfs_dir`/`romfs_file`): header, file data from 0x200 at 16-byte alignment,
  then directory and file hash tables and entry tables. Hash buckets, chains and entry order follow the format as the
  console reads it.
- **Icon** (`icon.rs`, `jpeg.rs`): the SlyNX wordmark and the game number on a blue-violet gradient, drawn in code
  and encoded by a small baseline JPEG encoder (per-image Huffman tables). Original project art, no game imagery or
  trademarks. A player's own icon must be a 256x256 JPEG.

The tests check these writers against devkitPro's own tools when installed (below): the NACP is byte-identical to
`nacptool`'s, and a whole `.nro` (program image, icon, NACP, a 31-file RomFS with nested folders) is byte-identical
to `elf2nro`'s.

### Pure Rust versus devkitPro's tools

| | Pure Rust (chosen) | Calling `elf2nro` / `nacptool` |
|---|---|---|
| What the player installs | `slynx-pack` and the program only | devkitPro, a developer toolchain, on top |
| Platforms | wherever Rust's `std` runs (Linux, Windows, macOS) | where devkitPro's tools are packaged |
| Large images | streamed: memory use stays small for a 3 GB image | the RomFS contents must first be laid out in a folder (a copy or link of the image) |
| Format risk | our writers must match the formats; the parity tests compare with devkitPro's output byte for byte | the reference implementation |
| Licences | our code only | the tools stay separate programs; nothing to bundle |

The parity tests make the main risk of the pure-Rust path measurable, so the packager does not call devkitPro at
all. Developers still build the program ELF with devkitPro (`tools/build-switch.sh`).

### The program

The packager does not compile or link anything: it needs the SlyNX program already built. For releases that is a
`slynx-game.elf` or `slynx-game.nro` (our code only, publishable) shipped next to `slynx-pack`. Until the game crate
exists, the Sly 1 viewer stands in:

```sh
crates/viewer/tools/build-switch.sh       # builds the viewer ELF and dist/slynx-viewer.nro (no game data)
slynx-pack sly1.iso --out out --program ~/slynx-target/switch-build/release/slynx-viewer/slynx-viewer.elf
```

Later: `SLYNX_CRATE=game tools/build-switch.sh` (see `docs/switch-build.md`).

## NSP output: not supported yet

`--nsp` and `--keys` are accepted on the command line and immediately refused with "NSP output is not supported
yet". The key file is never opened, read or copied. [legal/DISTRIBUTION.md](legal/DISTRIBUTION.md) section 3 calls
the NSP path higher risk and defers it pending a lawyer's view; the interface exists so that adding it later does not
change the command line. The `.nro` path needs no keys.

## Title ID

An `.nro` has no title ID; the NACP's ID fields stay zero. If the NSP path is ever added, it must use a homebrew
title ID, never an official one (DISTRIBUTION.md section 3).

## Tests

```sh
cargo test -p packager                               # unit tests + devkitPro parity (when installed)
SLYNX_DISC_SLY1=/path/to/sly1.iso cargo test --release -p packager --test disc
SLYNX_DISC_SLY2=/path/to/sly2.iso cargo test --release -p packager --test disc_sly2
```

- **Unit tests:** RomFS layout (the one-file layout `elf2nro` produces), a 40-file tree read back by hash lookup the
  way libnx does, streaming from disk; NACP fields and string cutting; ELF to NRO image (segments, padding, bss,
  build ID, `MOD0`) on a synthetic ELF, and rejection of malformed ones; the asset layout; the JPEG encoder checked
  by an independent baseline decoder in the tests (PSNR above 34 dB) and Huffman code-length limiting; the icon;
  disc classification; the pipeline with a stand-in image in both layouts, icon checks, and the NSP refusal (with a
  key path that does not exist, to show it is never opened).
- **devkitPro parity** (`tests/devkitpro.rs`): with `DEVKITPRO` (default `/opt/devkitpro`) and a linked program
  (`SLYNX_TEST_ELF`, default the platform demo ELF from `tools/build-switch.sh`), compares our NACP with
  `nacptool`'s and our whole `.nro` with `elf2nro`'s, byte for byte. Skips (and passes) without them.
- **Sly 2 disc test** (`tests/disc_sly2.rs`): with `SLYNX_DISC_SLY2`, the same for Sly 2: identity, layout, title
  "SlyNX 2", the RomFS list and manifest, and the packed `sly2.iso` opening through `formats_sly2::Disc` (executable
  hash, file table, level table).
- **Disc test** (`tests/disc.rs`): with `SLYNX_DISC_SLY1` and a program ELF (`SLYNX_PACK_PROGRAM`, default the
  viewer's or the demo's), identifies the disc, packages it into a temporary folder outside the repository
  (`SLYNX_PACK_TMP`, default the system temp folder; needs about 3 GB free), checks the `.nro` (title, author, icon,
  RomFS list, sizes), opens the packed image through `formats::disc::Disc` (executable hash, world table, first and
  last world equal to the original's) and compares the start, end and 48 spread windows of the image byte for byte.
  Then it deletes the package.

## Not verified yet

- On a Switch: the packaged `.nro` booting from the Homebrew Menu and reading `romfs:/sly1.iso` (the format checks
  above pass; the console has not run one yet).
- A Windows build of `slynx-pack` (it uses only `std`; no Windows toolchain was available).
