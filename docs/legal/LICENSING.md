# Licensing

Status: draft, 2026-11-13. **The license choice in section 1 is pending the owner's approval.**

This document is not legal advice. Points marked **[lawyer]** should be checked by a lawyer before a release with
real visibility; points marked **[verify online]** were worked out from the projects' repository contents and the installed packages and should be confirmed
against the upstream source.

## 1. Our license (pending owner approval)

**Recommendation: dual license, `MIT OR Apache-2.0`**, the same terms as the Rust compiler, its standard library and
most of the crate ecosystem. Draft texts: [`LICENSE-MIT`](../../LICENSE-MIT) and
[`LICENSE-APACHE`](../../LICENSE-APACHE) (the canonical Apache 2.0 text, checksum-identical to
`https://www.apache.org/licenses/LICENSE-2.0.txt`). Copyright holder: **Wahkah**.

### Why

| | MIT OR Apache-2.0 | GPL-3.0-or-later |
|---|---|---|
| Fits the clean room | Yes: we take nothing from the references, so we need no copyleft to inherit | Only needed if we derived code from sly2-decomp or sly-rs, which the clean room forbids anyway |
| Compatible with our dependencies | Yes (libnx ISC, deko3d zlib, newlib BSD-style, Rust std MIT/Apache) | Yes |
| Rust ecosystem | The norm; crates and code can move between projects freely | Crates we publish would be unusable by most of the ecosystem |
| Patent grant | Apache-2.0 section 3 gives an explicit patent license and a patent-retaliation clause | GPL-3.0 section 11 also has one |
| Switch builds | No conditions on the console side | GPL-3.0 section 6 "Installation Information" for User Products adds obligations for console-side distribution that are awkward to reason about on a locked device **[lawyer]** |
| Someone else could close-source a fork | Yes | No |

The last row is the one real trade-off. A fan port has little commercial value to a closed fork, and the game data
can never be bundled anyway, so the permissive license costs us little and makes the code usable as widely as
possible. If the owner prefers that every fork stays open, GPL-3.0-or-later is a sound alternative and changes
nothing else in the clean-room protocol.

### How to apply it (once approved)

- Root: `LICENSE-MIT`, `LICENSE-APACHE`, and a "License" section in the README (text in the change list below).
- `Cargo.toml` (`[workspace.package]`): `license = "MIT OR Apache-2.0"`; each crate sets `license.workspace = true`.
- Source file headers (optional, recommended for the C shim and any file that may be copied alone):
  `// SPDX-License-Identifier: MIT OR Apache-2.0` and `// Copyright (c) 2026 Wahkah`.
- Inbound = outbound: a `CONTRIBUTING.md` line saying that contributions are accepted under the same dual license
  (Apache-2.0 section 5 already says so for the Apache side; the sentence makes it explicit for MIT).

README section to add:

```markdown
## License

Copyright (c) 2026 Wahkah. Licensed under either of the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option. Third-party components and their licenses are listed
in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

This license covers SlyNX's own code only. It grants nothing in the Sly Cooper games, their data, names or
characters, which belong to their owners.
```

## 2. Reference projects

Checked against each project's repository contents as of 2026-11-13. A project with **no
license** is "all rights reserved": the code may be read, but not copied, translated, adapted or redistributed
(17 U.S.C. section 106; GitHub's Terms of Service, section D.5, only lets other users view and fork a public repository
within GitHub). Facts and ideas learned by reading are not protected (17 U.S.C. section 102(b)); expression is.

### ProjectCane (https://github.com/theclub654/ProjectCane)

- **License: none.** No LICENSE/COPYING file in the tree; the README has no license section, only a "Legal"
  disclaimer (unofficial, not affiliated with Sony or Sucker Punch, no game data). The license files under
  `Libraries/` (stb, dirent, FFmpeg) belong to vendored third-party libraries, not to ProjectCane.
  **[verify online]** that upstream has not added a license since.
- **Modified versions:** changes made to ProjectCane (ports to other platforms, renderer or packaging changes) are
  derivatives of an unlicensed work. Their authors own their own lines, but the result cannot be published, and a
  patch against ProjectCane is hard to separate from it.
- **May:** read it on the reference side for facts (formats, behaviour, constants).
- **May not:** publish its code, a modified copy, a fork, patches, or builds based on it (with or without game data);
  translate or paraphrase its code into SlyNX; give its code to the implementation side.
- **If ever wanted:** ask its author for a license or for written permission, or contribute changes upstream.

### SafeHouse (https://github.com/FridiNaTor1/SafeHouse)

- **License: none for its own code.** `THIRD_PARTY_NOTICES.md` and `licenses/` list only third-party components
  (GLFW, FAudio, SDL2, ImGui, GLM, stb, miniz, libchdr, FFmpeg LGPL, Kenney CC0, fonts under OFL, and others). It
  grew out of ProjectCane, so its Sly 1 code carries ProjectCane's status too. It also ships Sony's game logos and
  community-made Steam art; none of that may be copied.
- **May:** read on the reference side for facts.
- **May not:** copy, translate or publish its code, assets, logos, or builds based on it.

### sly2-decomp (https://github.com/OpenThievius/sly2-decomp)

- **License: GPL-3.0-or-later** (`LICENSE` is the GPL-3.0 text; README "License" and `CONTRIBUTING.md` require
  `SPDX-License-Identifier: GPL-3.0-or-later` headers).
- **Important:** its own `THIRD_PARTY_NOTICES.md` says the reconstructed game functions in `src/` and `nonmatching/`
  "remain the intellectual property" of Sucker Punch and Sony; the GPL covers only its tooling. So `src/` is not
  GPL code we could use even under the GPL: it is the game's own logic.
- **May:** read on the reference side for facts (formats, constants, behaviour). Its tooling could be used under
  GPL-3.0 as separate programs (not linked into SlyNX).
- **May not:** copy or translate any of it into SlyNX. Any code derived from its GPL parts would force SlyNX under
  GPL-3.0; anything derived from `src/`/`nonmatching/` would be Sony's code and is not ours to publish under any
  license.

### Sly-2-Recomp (https://github.com/menaman123/Sly-2-Recomp) and NativeEE (https://github.com/menaman123/NativeEE)

- **License: Apache-2.0** (`LICENSE`, canonical text with the appendix template unfilled; README "License"
  section). The README says the `engine/` submodule, NativeEE, is **MIT** (`engine/LICENSE`); not yet checked
  **[verify online]**.
- **May:** reuse code under Apache-2.0 section 4: keep its copyright and license notices, ship the license text,
  mark modified files as changed, and carry over any `NOTICE` file (none found). Compatible with MIT OR Apache-2.0.
- **Policy anyway:** treat it as reference-only. Its design is a recompiler of the PS2 executable, not ours, and
  recompiled or "title" code derived from the game's executable is the game's code, which no license from the
  recompiler author can give us. If a specific generic piece (a tool, a math routine with no game content) is ever
  worth reusing, it goes through the legal review first and gets an entry in `THIRD_PARTY_NOTICES.md`.

### sly-rs (https://github.com/jadzziaa/sly-rs)

- **License: GPL-3.0-or-later** (`LICENSE` is the GPL-3.0 text; `Cargo.toml` `license = "GPL-3.0-or-later"`;
  `NOTICE.md`). Its `NOTICE.md` lists code ported from PCSX2 (GPL-3.0-or-later), rac-rs (GPL-3.0) and OpenGOAL (ISC).
- **May:** read on the reference side for facts.
- **May not:** copy or translate any of it (that would make SlyNX GPL-3.0 and pull in PCSX2-derived code).

### Summary

| Project | License (verified) | Read (reference side) | Copy / translate into SlyNX | Publish its code or builds |
|---|---|---|---|---|
| ProjectCane | none | yes | no | no |
| SafeHouse | none (own code) | yes | no | no |
| sly2-decomp | GPL-3.0-or-later (tooling); `src/` is the game owners' | yes | no | n/a |
| Sly-2-Recomp | Apache-2.0 (NativeEE: MIT, unverified) | yes | only after legal review, with notices | n/a |
| sly-rs | GPL-3.0-or-later | yes | no | n/a |

The game itself (executables, disc data) is Sony's and Sucker Punch's: see [DISTRIBUTION.md](DISTRIBUTION.md).

## 3. Dependencies

### Switch toolchain and libraries (devkitPro packages)

| Component | Version checked | License | Shipped in our binaries? | Obligation |
|---|---|---|---|---|
| libnx | 4.12.0 | ISC ("libnx Authors"; pacman lists "custom") **[verify online: `switchbrew/libnx` `LICENSE.md`]** | yes, statically linked | Keep the copyright and permission notice in the notices file |
| libnx BSD headers (`netinet/`, `sys/`, `poll.h`) | with libnx | BSD-3-Clause (Regents of the University of California) | yes if their code is used | Reproduce the notice in binary distributions |
| deko3d | 0.5.0 | zlib (header: "Copyright (C) 2018-2020 fincs") | yes | Don't misrepresent origin; mark altered source; acknowledgement appreciated |
| devkitA64 newlib | 4.6.0 | various BSD-style and similar (newlib `COPYING.NEWLIB`) **[verify online]** | yes (C runtime) | Several of its licenses require the notice in binary distributions |
| GCC runtime (libgcc) | 16.1.0 | GPL-3.0 with the GCC Runtime Library Exception 3.1 | yes | None for the output when compiled with GCC normally |
| devkitA64 GCC, binutils | 16.1.0 / 2.46.0 | GPL-3.0-or-later and others | no (build tools) | None for our code |
| switch-tools (`elf2nro`, `elf2nso`, `nacptool`) | 1.13.1 | ISC | no (build tools) | None unless we redistribute the tools |
| uam (deko3d shader compiler) | 1.1.0 | ISC | no (build tool; compiled shaders are our output) | None |
| hacBrewPack (https://github.com/rlaphoenix/hacBrewPack) | current | GPL-2.0 (README: "version 2"), with hactool/switch-tools parts | only if we bundle it | Invoke it as a separate program. If a release ever bundles the binary: ship the GPL-2.0 text and its source (or a written offer) |
| Rust `core`/`alloc`/`compiler_builtins` | nightly | MIT OR Apache-2.0 | yes | Include the MIT notice (and Apache text) in binary releases |

Not used and to be avoided on the Switch path: the devkitPro Mesa port (MIT, but not needed with deko3d) and
FFmpeg (LGPL/GPL depending on build) unless the legal review signs off on how it is linked.

### Rust crates: `cargo deny` allow-list

The allow-list lives in one place: [`deny.toml`](../../deny.toml) at the repository root (`[licenses] allow`).
That file is the source of truth; this section only explains the policy behind it. Adding a license to it, or a
per-crate exception, needs the legal review and a comment in `deny.toml` saying why.

The list contains permissive licenses only: their conditions (keep the notice, no endorsement, for Apache-2.0 also
carry `NOTICE` files) are met by shipping the crate notices with each binary release. `MPL-2.0` (file-level copyleft:
modified files of the crate must stay MPL) is allowed only as a reviewed per-crate exception.

Never allowed: `GPL-*`, `AGPL-*`, `LGPL-*` (Rust links statically, which makes LGPL's relinking requirement
impractical), `SSPL-*`, `EUPL-*`, `OSL-*`, `CC-BY-NC-*` or any non-commercial or "no modification" terms, and crates
with no license. `Unlicense` and `WTFPL` only by exception (some jurisdictions doubt public-domain dedications).
Fonts and other assets: `OFL-1.1` and `CC0-1.0`/`CC-BY-4.0` are fine for original assets, listed in the notices.

### Releasing binaries

Every binary release (PC packager, `.nro` of our own code) includes `THIRD_PARTY_NOTICES.md`, both license files,
and the notices of every crate compiled in. Generate the crate part with `cargo about` (or `cargo deny list`) at
release time and check it in the release audit.
