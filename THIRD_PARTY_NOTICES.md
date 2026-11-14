# Third-party notices

SlyNX is copyright (c) 2026 Wahkah and licensed under MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`;
license choice pending the owner's approval). It contains no code or data from the Sly Cooper games. Sly Cooper and
related names and characters are trademarks of Sony Interactive Entertainment; the games were developed by Sucker
Punch Productions. Nintendo Switch is a trademark of Nintendo. SlyNX is not affiliated with or endorsed by any of
them.

This file lists third-party software that is compiled into, linked into or shipped with SlyNX builds, with its
license. Each entry says where it is used. Add an entry before the dependency lands. Rust crates compiled into a
release are listed in the crate notices generated for that release (`cargo about`), which ship alongside this file.

Status of each entry: **verified** (text checked against the upstream license file) or **to verify** (text taken
from the installed headers or package metadata; confirm against upstream before the first release).

## Linked into Switch builds

### libnx

- Project: https://github.com/switchbrew/libnx (version 4.12.0)
- Use: Switch system library, statically linked into every `.nro`/`.nsp`.
- License: ISC. Status: **to verify** (copyright years and wording from libnx's `LICENSE.md`).

```text
Copyright 2017-2018 libnx Authors

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby
granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN
AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THIS SOFTWARE.
```

libnx includes networking and system headers derived from FreeBSD (`netinet/`, `sys/`, `poll.h`) under the
BSD-3-Clause license. Status: **verified** against the installed headers. The copyright years differ per file
(1980 to 1997, some with additional holders such as Digital Equipment Corporation and Peter Wemm); the conditions
are the same. TODO: list each file's holders if those headers' code ends up in a binary.

```text
Copyright (c) 1982, 1986, 1990, 1993
	The Regents of the University of California.  All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
following conditions are met:
1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
   disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
   following disclaimer in the documentation and/or other materials provided with the distribution.
3. Neither the name of the University nor the names of its contributors may be used to endorse or promote products
   derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN
NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA,
OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### deko3d

- Project: https://github.com/devkitPro/deko3d (version 0.5.0)
- Use: Switch graphics API, statically linked into every `.nro`/`.nsp`.
- License: zlib. Status: **to verify** (text from the installed `deko3d.h`; confirm years against the repository's
  license file).

```text
Copyright (C) 2018-2020 fincs

This software is provided 'as-is', without any express or implied warranty. In no event will the authors be held
liable for any damages arising from the use of this software.

Permission is granted to anyone to use this software for any purpose, including commercial applications, and to
alter it and redistribute it freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim that you wrote the original software.
   If you use this software in a product, an acknowledgment in the product documentation would be appreciated but is
   not required.
2. Altered source versions must be plainly marked as such, and must not be misrepresented as being the original
   software.
3. This notice may not be removed or altered from any source distribution.
```

### newlib (devkitA64)

- Project: newlib as packaged by devkitPro (`devkita64-newlib` 4.6.0)
- Use: C runtime, statically linked into Switch builds.
- License: a collection of BSD-style and similar licenses listed in newlib's `COPYING.NEWLIB`. Status: **to verify**.
  TODO before the first Switch release: include the notices of the newlib parts actually linked.

### GCC runtime library (devkitA64)

- Use: `libgcc` support routines linked into Switch builds.
- License: GPL-3.0 with the GCC Runtime Library Exception 3.1, which places no conditions on programs compiled with
  GCC in the normal way. Listed for completeness.

### Rust standard library (`core`, `alloc`, `compiler_builtins`)

- Project: https://github.com/rust-lang/rust
- Use: compiled into every Rust build, PC and Switch.
- License: MIT OR Apache-2.0 (some parts of `compiler_builtins` derive from LLVM's compiler-rt, Apache-2.0 WITH
  LLVM-exception). Copyright: The Rust Project Developers. Status: **to verify** (attach the toolchain's `COPYRIGHT`
  notices to binary releases).

## Build tools (not shipped)

Listed so the toolchain is documented; no notice obligations unless these tools are redistributed.

| Tool | License | Use |
|---|---|---|
| devkitA64 (GCC 16.1.0, binutils 2.46.0) | GPL-3.0-or-later and others | Cross-compiler and linker for Switch builds |
| switch-tools 1.13.1 (`elf2nro`, `elf2nso`, `nacptool`) | ISC | Building `.nro`/`.nso` and the NACP |
| uam 1.1.0 | ISC | Compiles deko3d shaders |
| hacBrewPack (https://github.com/rlaphoenix/hacBrewPack) | GPL-2.0 | Builds `.nsp` packages on the player's machine; run as a separate program, not bundled. If a release ever bundles it, add its GPL-2.0 text and source here |

## Rust crates

None yet. Every crate must pass `cargo deny` with the license allow-list in `deny.toml` (policy in
`docs/legal/LICENSING.md`). Crates compiled into a
binary release are listed with their licenses in that release's generated crate notices.

## Assets

None yet. Original project art (logo, default icon) is copyright Wahkah and covered by the project license unless
noted here. No Sony or Nintendo art is included.
