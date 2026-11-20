# Building for the Nintendo Switch

The Switch build turns Rust code into a homebrew `.nro` that runs from the Homebrew Menu. Graphics go through
deko3d, the Switch's native graphics API (no OpenGL/Mesa).

## Requirements

Linux or WSL with:

- **devkitPro** at `/opt/devkitpro` (override with `DEVKITPRO`): devkitA64, libnx, deko3d (`libdeko3d.a`), and the
  `uam` shader compiler, `elf2nro` and `nacptool` from switch-tools.
- **Rust** via rustup, nightly with `rust-src` (the repository's `rust-toolchain.toml` selects it). The Switch target
  `aarch64-nintendo-switch-freestanding` is tier 3, so `core` and `alloc` are built from source with `-Z build-std`.
- `python3` (optional) for the post-build `.nro` check.

## Build

From the repository root (in WSL: `wsl -e bash -lc 'cd /mnt/c/.../SlyNX && tools/build-switch.sh'`):

```sh
tools/build-switch.sh            # release -> dist/slynx-hello.nro
tools/build-switch.sh --debug    # debug   -> dist/slynx-hello-debug.nro
```

Any other argument is an error. Copy the `.nro` to `/switch/` on the SD card and start it from the Homebrew Menu.
Press **+** to exit.

Intermediate files go to `$CARGO_TARGET_DIR` (default `~/slynx-target`), outside the repository and off any synced
folder: the Rust build in `<target>/aarch64-nintendo-switch-freestanding/`, everything else (shaders, objects, the
linked `.elf` with symbols, the link map, the `.nacp`) in `<target>/switch-build/<profile>/<name>/`, cleared at the
start of each build. Only the `.nro` lands in `dist/`, which must never be committed.

| Variable | Default | Meaning |
|---|---|---|
| `DEVKITPRO` | `/opt/devkitpro` | devkitPro install |
| `CARGO_TARGET_DIR` | `~/slynx-target` | cargo output and intermediate files |
| `SLYNX_BUILD_DIR` | `$CARGO_TARGET_DIR/switch-build` | intermediate files only |
| `SLYNX_ICON` | libnx's `default_icon.jpg` | 256x256 JPEG shown in the Homebrew Menu |
| `SLYNX_CRATE` | `hello-switch` | the staticlib crate that provides `main` |
| `SLYNX_FEATURES` | none | cargo features for that crate (comma- or space-separated) |
| `SLYNX_NAME` | `slynx-hello` for hello-switch, `slynx-<crate>` otherwise | output name; `-debug` is appended for `--debug` |
| `SLYNX_TITLE` | `SlyNX` | title in the NACP (Homebrew Menu) |
| `SLYNX_ROMFS` | `crates/<crate>/romfs` if it exists | folder packed into the `.nro` as its RomFS |
| `SLYNX_EXTRA_C` | none | colon-separated folders whose `*.c` and `*.S` files are compiled and linked too |
| `SLYNX_EXTRA_SHADERS` | none | colon-separated shader folders, each `folder` or `folder=prefix` |

Relative paths are relative to the repository root.

### Building another crate (the renderer demo)

Any staticlib crate whose Switch build provides `main` can be packaged the same way. Extra C folders are compiled
with `-I` set to the platform shim (`slynx_shim.h`) and to the folder itself. Each `<name>.glsl` in an extra shader
folder becomes `<prefix><name>.dksh` in the same output folder as the platform's shaders, so a `*.S` file in an
extra C folder can embed it with `.incbin "<prefix><name>.dksh"`. The stage comes from the file name: `*_vsh`
vertex, `*_fsh` fragment, `*_gsh` geometry, `*_csh` compute, `*_tcsh`/`*_tesh` tessellation. Two shaders that map to
the same `.dksh` name stop the build.

```sh
SLYNX_CRATE=render SLYNX_FEATURES=switch-demo SLYNX_NAME=slynx-render-demo SLYNX_TITLE="SlyNX render demo" \
SLYNX_EXTRA_C=crates/render/c SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_ \
    tools/build-switch.sh                           # -> dist/slynx-render-demo.nro
```

`crates/render/tools/build-switch-demo.sh [--debug]` is a wrapper that sets exactly these variables.

### The games

The game crate builds three programs. None contains any game data: each reads the player's own disc image.

```sh
SLYNX_ICON=icon1.jpg crates/game/tools/build-switch.sh        # Sly 1 -> dist/slynx.nro ("SlyNX")
SLYNX_ICON=icon2.jpg crates/game/tools/build-switch-sly2.sh   # Sly 2 -> dist/slynx-sly2.nro ("SlyNX 2")
SLYNX_ICON=icon3.jpg crates/game/tools/build-switch-sly3.sh   # Sly 3 -> dist/slynx-sly3.nro ("SlyNX 3")
slynx-pack --write-icon icon1.jpg --number 1                  # the SlyNX icons (docs/packager.md)
```

- **Sly 1** (`src/switch.rs`) looks for its disc as `game::settings` says: the `disc` line of
  `sdmc:/switch/SlyNX/user/game.cfg`, then `sdmc:/switch/SlyNX/sly1.iso`, then `romfs:/sly1.iso`.
- **Sly 2** (`src/switch_sly2.rs`, feature `sly2-switch`) looks for the `disc` line of
  `sdmc:/switch/SlyNX/user/sly2.cfg`, then `sdmc:/switch/SlyNX/sly2.iso`, then `romfs:/sly2.iso`. It starts in that
  file's `world` (a world id such as `0x41`, or a level-table row) or in the nightclub hub.
- **Sly 3** (`src/switch_sly3.rs`, feature `sly3-switch`) looks for the `disc` line of
  `sdmc:/switch/SlyNX/user/sly3.cfg`, then `sdmc:/switch/SlyNX/sly3.iso`, then `romfs:/sly3.iso`. It starts in that
  file's `world` (a world name, or a world slot) or in `v_apt` (`game::sly3::FIRST_WORLD`). Its sound is not
  started yet.

### The smoke build

`crates/game/tools/build-switch-smoke.sh [--debug]` builds `dist/slynx-smoke.nro` ("SlyNX smoke"; the game crate
with feature `smoke-switch`, entry `src/switch_smoke.rs`) for the final test pass on hardware. It finds the Sly 1
disc as the Sly 1 program does and runs the Sly 1 smoke route of `game::smoke` once, with the real renderer and no
player input: the title, a new game into Paris, the Hideout and each episode's first level, 20 s each. After every
stage it rewrites `sdmc:/switch/SlyNX/user/smoke.log` (one line per stage: frame times, the update share, load time,
script and host errors), and each line also goes to the debug log. The log ends with the failures, checked against
the 60 fps budget (a world not reached, a frozen world, growing host errors, a 95th-percentile frame over
16.7 ms), and `smoke: passed`, `smoke: failed` or `smoke: stopped` (+ stops it early). The PC tool runs the same route
(docs/testing.md, "Smoke runs").

A program `.nro` runs with the image on the SD card. `slynx-pack <disc.iso> --out <dir> --program <its .elf>
--icon <jpg>` makes the packaged `.nro`, with the image in its RomFS. The packaged `.nro` holds the player's game
data, so it belongs on their own machine, never in a synced or shared folder.

### Building from a snapshot

Other crates in the tree may be mid-change and not build when a release is made. The release builds are then made
from a snapshot of a tree state that builds, so the build cannot pick up a half-edited file:

1. Copy the repository to a snapshot folder of its own, outside the synced tree, leaving out `dist`, `.git` and
   `target`. Every snapshot gets a fresh folder, so two snapshots made at the same time (two agents, a build and
   a review) never share or delete each other's files:
   `SNAP=$(mktemp -d "$HOME/slynx-snap.XXXXXX")`, then
   `(cd SlyNX && tar --exclude=./dist --exclude=./.git --exclude=./target -cf - .) | tar -xf - -C "$SNAP"`.
2. Check that the snapshot builds on the host (`cargo build -p game`). If it does not, wait and copy again.
3. Build in the snapshot with a target folder of its own (`CARGO_TARGET_DIR="$SNAP-target"`), so neither the live
   tree's build nor another snapshot shares intermediate files with it:
   - the icons: `slynx-pack --write-icon dist/icons/slyN.jpg --number N`;
   - the programs: `SLYNX_ICON=dist/icons/slyN.jpg crates/game/tools/build-switch[-sly2|-sly3].sh`.
4. Copy only the program `.nro` files from the snapshot's `dist/` to the repository's `dist/`.
5. Package each disc with `slynx-pack <disc.iso> --out ~/slynx-packaged/slyN --program
   "$SNAP-target"/switch-build/release/<name>/<name>.elf --icon dist/icons/slyN.jpg`, into the WSL home and
   never into the synced tree. Check the result with `slynx-pack --inspect`.
6. Run the checks on the snapshot: the crates' tests, `cargo clippy` for the host and the Switch target, `cargo fmt
   --check`, and the packager's disc tests with `SLYNX_PACK_PROGRAM` set to the built ELF.

The snapshot is a build aid only: changes are made in the repository, never in the snapshot. Delete it and its
target folder when done (`rm -rf "$SNAP" "$SNAP-target"`). `tools/switch-test-package.sh` does all of this for the
final test pass (docs/switch-test.md): it builds every program from its own snapshot, then packages the discs into
`~/slynx-packaged/` (with `--package`) and prints where each file goes on the SD card.

## What the script does

1. **Rust staticlib.** `cargo +nightly build -p <crate> [--features ...] --release -Z build-std=core,alloc --target
   aarch64-nintendo-switch-freestanding` produces `lib<crate>.a` (`libhello_switch.a` by default), containing the
   program, the platform layer, `core`, `alloc` and `compiler_builtins`. The Rust `main` is the program entry that libnx's startup code calls.
2. **Shaders.** `uam` compiles the GLSL in `crates/platform-switch/c/shaders/` and the extra shader folders to
   deko3d `.dksh` binaries.
3. **C shim.** devkitA64 GCC compiles `crates/platform-switch/c/slynx_platform.c` (applet, input, time, threads,
   audio, files), `slynx_display.c` (deko3d device, queue, swapchain), `slynx_debugdraw.c` (the demo's triangle
   pipeline) and `slynx_shaders.S`, which embeds the `.dksh` files with `.incbin` so no shader has to be loaded at
   startup; then every `*.c` and `*.S` in the extra C folders.
4. **Link.** `aarch64-none-elf-gcc -specs=$DEVKITPRO/libnx/switch.specs` links the C objects, the embedded shaders
   and the Rust staticlib against `-ldeko3d -lnx` (newlib's libc comes in through the specs). The specs file brings libnx's
   linker script, startup code and position-independent executable settings.
5. **NACP.** `nacptool --create "<title>" "Wahkah" "0.0.1"`.
6. **NRO.** `elf2nro` with the icon, the NACP and the RomFS folder.
7. **Check.** A short Python reader verifies the NRO header (`NRO0` magic, contiguous text/rodata/data segments,
   `MOD0` header), the asset section (`ASET`, JPEG icon, 0x4000-byte NACP, RomFS header) and the NACP's title,
   author and version.

## Code layout

The platform API itself (what the game sees on both the Switch and the PC) is described in
[platform.md](platform.md).

- `crates/platform-api/` is `platform-api`: the `Platform` trait and the shared types, `no_std` and FFI-free.
- `crates/platform-switch/` is the Switch backend. On the Switch (`target_os = "horizon"`) it is `no_std` + `alloc`
  and provides `SwitchPlatform`, the display hand-off to the renderer, `SwitchDebugDraw`, the global allocator
  (newlib's `malloc`/`memalign`/`realloc`/`free`) and the panic handler (message to `svcOutputDebugString`, then
  abort). On other targets only the controller mapping is compiled, so it is tested on a PC.
- `crates/platform-switch/c/` is the C shim. `slynx_shim.h` declares the FFI boundary (mirrored in `src/sys.rs`) and
  is also the header the renderer's C code includes to reach the deko3d device. All `unsafe` Rust is in `sys.rs`
  (declarations) and the modules that call it, each block commented with what it relies on.
- `crates/hello-switch/` is the platform demo (see [platform.md](platform.md#the-demo)). It is a staticlib (for the
  `.nro`) and an rlib (so `cargo test` runs its tests and `examples/hello_pc.rs` runs it on a PC).

### Display and debug draw

The display is one deko3d device, one graphics queue and a swapchain of three 1920x1080 RGBA8 images. Docked, the
whole image is presented; handheld, the swapchain crops to the top-left 1280x720. Device creation, image memory,
swapchain and queue are each checked, and a failure part-way destroys whatever was already created.

The debug draw has one shader pair (position + colour) and two frame slots, each with its own command memory,
vertex memory and fence. A frame waits on its slot's fence (the GPU finished with it two frames ago), copies the
vertices in, acquires an image, records a short command list (bind target, viewport and scissor at the output size,
clear, pipeline state, vertex buffer, one draw, signal fence), submits it and presents. Vertex memory is CPU- and
GPU-uncached so CPU writes are visible to the GPU without cache maintenance; fine for a demo, the real renderer
manages its own buffers. Colours are stored to the RGBA8_Unorm framebuffer as given (no gamma conversion).

## Testing on a PC

```sh
cargo test --workspace
SWITCH="-Z build-std=core,alloc --target aarch64-nintendo-switch-freestanding"
cargo clippy -p platform-api -p platform-switch -p hello-switch -p render -p viewer $SWITCH
cargo clippy -p render --features switch-demo $SWITCH    # the renderer demo's entry point
cargo clippy -p formats --no-default-features $SWITCH    # formats is no_std without its std feature
cargo run -p hello-switch --example hello_pc
```

`formats` gets its own run because `--no-default-features` applies to every package on the command line,
and on the Switch `viewer` already uses `formats` without `std`.

## Debugging

- The `.elf` with full symbols is in `<target>/switch-build/<profile>/<name>/`; use `aarch64-none-elf-addr2line -e` on crash
  addresses.
- Panic messages go to `svcOutputDebugString`, readable with a debugger attached or a system log module.
