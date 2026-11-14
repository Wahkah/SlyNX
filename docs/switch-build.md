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

Copy the `.nro` to `/switch/` on the SD card and start it from the Homebrew Menu. Press **+** to exit.

Intermediate files go to `$CARGO_TARGET_DIR` (default `~/slynx-target`), outside the repository and off any synced
folder: the Rust build in `<target>/aarch64-nintendo-switch-freestanding/`, everything else (shaders, objects, the
linked `.elf` with symbols, the link map, the `.nacp`) in `<target>/switch-build/<profile>/`. Only the `.nro` lands in
`dist/`, which must never be committed.

| Variable | Default | Meaning |
|---|---|---|
| `DEVKITPRO` | `/opt/devkitpro` | devkitPro install |
| `CARGO_TARGET_DIR` | `~/slynx-target` | cargo output and intermediate files |
| `SLYNX_BUILD_DIR` | `$CARGO_TARGET_DIR/switch-build` | intermediate files only |
| `SLYNX_ICON` | libnx's `default_icon.jpg` | 256x256 JPEG shown in the Homebrew Menu |

## What the script does

1. **Rust staticlib.** `cargo +nightly build -p hello-switch --release -Z build-std=core,alloc --target
   aarch64-nintendo-switch-freestanding` produces `libhello_switch.a`, containing the program, the platform layer,
   `core`, `alloc` and `compiler_builtins`. The Rust `main` is the program entry that libnx's startup code calls.
2. **Shaders.** `uam` compiles the GLSL in `crates/platform-switch/c/shaders/` to deko3d `.dksh` binaries.
3. **C shim.** devkitA64 GCC compiles `crates/platform-switch/c/slynx_shim.c` (libnx and deko3d calls) and
   `slynx_shaders.S`, which embeds the `.dksh` files with `.incbin` so there is no RomFS to load at startup.
4. **Link.** `aarch64-none-elf-gcc -specs=$DEVKITPRO/libnx/switch.specs` links the shim, the embedded shaders and the
   Rust staticlib against `-ldeko3d -lnx` (newlib's libc comes in through the specs). The specs file brings libnx's
   linker script, startup code and position-independent executable settings.
5. **NACP.** `nacptool --create "SlyNX" "Wahkah" "0.0.1"`.
6. **NRO.** `elf2nro` with the icon and the NACP.
7. **Check.** A short Python reader verifies the NRO header (`NRO0` magic, contiguous text/rodata/data segments,
   `MOD0` header), the asset section (`ASET`, JPEG icon, 0x4000-byte NACP) and the NACP's title, author and version.

## Code layout

- `crates/platform-switch/` is the Rust side of the platform layer. On the Switch (`target_os = "horizon"`) it is
  `no_std` + `alloc` and provides:
  - the global allocator, forwarding to newlib's `malloc`/`memalign`/`realloc`/`free` (libnx gives newlib the heap);
  - the panic handler, which formats the message into a stack buffer, writes it with `svcOutputDebugString` and
    aborts;
  - safe wrappers: `applet_main_loop()`, `Pad` (input), `ticks_ns()`, `debug_log()`, and `Gfx`, a minimal deko3d
    renderer that clears and draws a triangle list each frame.
  On other targets only the plain data types (`Vertex`, `Buttons`) are compiled, so the workspace builds and tests on
  a PC.
- `crates/platform-switch/c/` is the C shim: `slynx_shim.h` declares the FFI boundary (mirrored in
  `src/sys.rs`), `slynx_shim.c` implements it. All `unsafe` Rust is in `sys.rs` (declarations), `heap.rs` and
  `switch.rs`, each block commented with what it relies on.
- `crates/hello-switch/` is the smoke test. It is a staticlib (for the `.nro`) and an rlib (so
  `cargo test -p hello-switch` runs its scene tests on a PC). The scene is built in plain Rust: a breathing blue
  clear, a spinning rainbow triangle, "SlyNX" in large bobbing block letters and two status lines, all as one
  triangle list.

### The renderer

`Gfx` owns a deko3d device, a graphics queue, two 1280x720 RGBA8 framebuffers in a swapchain, one shader pair
(position + colour) and two frame slots. Each slot has its own command memory and vertex memory and a fence.
A frame waits on its slot's fence (the GPU finished with it two frames ago), copies the vertices in, records a
short command list (bind target, viewport, clear, pipeline state, vertex buffer, one draw, signal fence), submits
it and presents. The device uses an upper-left origin, so normalised y = -1 is the top of the screen.

Vertex memory is CPU- and GPU-uncached so CPU writes are visible to the GPU without cache maintenance. That is fine
for a few hundred KiB per frame; the real renderer will manage its own buffers.

## Testing on a PC

```sh
cargo test --workspace
cargo clippy -p hello-switch -Z build-std=core,alloc --target aarch64-nintendo-switch-freestanding
```

## Debugging

- The `.elf` with full symbols is in `<target>/switch-build/<profile>/`; use `aarch64-none-elf-addr2line -e` on crash
  addresses.
- Panic messages go to `svcOutputDebugString`, readable with a debugger attached or a system log module.
