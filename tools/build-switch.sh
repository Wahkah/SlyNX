#!/usr/bin/env bash
# Builds the Switch smoke test (crates/hello-switch) into dist/slynx-hello.nro.
#
#   tools/build-switch.sh            release build -> dist/slynx-hello.nro
#   tools/build-switch.sh --debug    debug build   -> dist/slynx-hello-debug.nro
#
# Steps: Rust staticlib (nightly, build-std, aarch64-nintendo-switch-freestanding) -> shaders (uam) -> C shim
# (devkitA64) -> link against libnx + deko3d with switch.specs -> NACP (nacptool) -> NRO (elf2nro) -> header check.
#
# Run it on Linux or WSL with devkitPro installed. Environment overrides:
#   DEVKITPRO          default /opt/devkitpro
#   CARGO_TARGET_DIR   default $HOME/slynx-target (kept outside the repo and off synced folders)
#   SLYNX_BUILD_DIR    intermediate files, default $CARGO_TARGET_DIR/switch-build
#   SLYNX_ICON         256x256 JPEG for the Homebrew Menu, default libnx's default_icon.jpg
set -euo pipefail

PROFILE=release
if [[ "${1:-}" == "--debug" ]]; then PROFILE=debug; fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/slynx-target}"
BUILD="${SLYNX_BUILD_DIR:-$CARGO_TARGET_DIR/switch-build}/$PROFILE"
DIST="$ROOT/dist"
ICON="${SLYNX_ICON:-$DEVKITPRO/libnx/default_icon.jpg}"

TARGET=aarch64-nintendo-switch-freestanding
NAME=slynx-hello
[[ "$PROFILE" == debug ]] && NAME=slynx-hello-debug
TITLE="SlyNX"
AUTHOR="Wahkah"
VERSION="0.0.1"

A64="$DEVKITPRO/devkitA64/bin/aarch64-none-elf-"
TOOLS="$DEVKITPRO/tools/bin"
LIBNX="$DEVKITPRO/libnx"
SHIM="$ROOT/crates/platform-switch/c"

for tool in "${A64}gcc" "$TOOLS/uam" "$TOOLS/elf2nro" "$TOOLS/nacptool"; do
    [[ -x "$tool" ]] || { echo "missing $tool (is devkitPro installed with switch-dev and deko3d?)" >&2; exit 1; }
done
[[ -f "$LIBNX/lib/libdeko3d.a" ]] || { echo "missing libdeko3d.a (install the deko3d package)" >&2; exit 1; }
if [[ -f "$HOME/.cargo/env" ]]; then . "$HOME/.cargo/env"; fi

mkdir -p "$BUILD/shaders" "$DIST"

echo "== Rust staticlib ($PROFILE)"
CARGO_PROFILE_FLAG=()
[[ "$PROFILE" == release ]] && CARGO_PROFILE_FLAG=(--release)
(cd "$ROOT" && cargo +nightly build -p hello-switch "${CARGO_PROFILE_FLAG[@]}" \
    -Z build-std=core,alloc --target "$TARGET")
RUSTLIB="$CARGO_TARGET_DIR/$TARGET/$PROFILE/libhello_switch.a"

echo "== Shaders (uam)"
"$TOOLS/uam" -s vert -o "$BUILD/shaders/slynx_color_vsh.dksh" "$SHIM/shaders/slynx_color_vsh.glsl"
"$TOOLS/uam" -s frag -o "$BUILD/shaders/slynx_color_fsh.dksh" "$SHIM/shaders/slynx_color_fsh.glsl"

echo "== C shim"
ARCH=(-march=armv8-a+crc+crypto -mtune=cortex-a57 -mtp=soft -fPIE)
CFLAGS=(-g -O2 -Wall -Wextra -ffunction-sections -fdata-sections -D__SWITCH__ -I"$LIBNX/include" "${ARCH[@]}")
"${A64}gcc" "${CFLAGS[@]}" -std=gnu11 -c "$SHIM/slynx_shim.c" -o "$BUILD/slynx_shim.o"
"${A64}gcc" "${ARCH[@]}" -g -c "$SHIM/slynx_shaders.S" -Wa,-I,"$BUILD/shaders" -o "$BUILD/slynx_shaders.o"

echo "== Link"
# deko3d before nx: libdeko3d depends on libnx. switch.specs adds the linker script, crt0 and -pie.
"${A64}gcc" -specs="$LIBNX/switch.specs" -g "${ARCH[@]}" -Wl,-Map,"$BUILD/$NAME.map" \
    "$BUILD/slynx_shim.o" "$BUILD/slynx_shaders.o" "$RUSTLIB" \
    -L"$LIBNX/lib" -ldeko3d -lnx -o "$BUILD/$NAME.elf"

echo "== NACP + NRO"
"$TOOLS/nacptool" --create "$TITLE" "$AUTHOR" "$VERSION" "$BUILD/$NAME.nacp"
"$TOOLS/elf2nro" "$BUILD/$NAME.elf" "$DIST/$NAME.nro" --icon="$ICON" --nacp="$BUILD/$NAME.nacp"

echo "== Check"
if command -v python3 >/dev/null; then
    python3 -I - "$DIST/$NAME.nro" "$TITLE" "$AUTHOR" "$VERSION" <<'PY'
import struct, sys
path, title, author, version = sys.argv[1:5]
d = open(path, "rb").read()
def fail(msg):
    sys.exit(f"NRO check failed: {msg}")
if d[0x10:0x14] != b"NRO0": fail("no NRO0 magic at 0x10")
size = struct.unpack_from("<I", d, 0x18)[0]
segs = [struct.unpack_from("<II", d, 0x20 + 8 * i) for i in range(3)]
bss = struct.unpack_from("<I", d, 0x38)[0]
if size > len(d): fail(f"header size {size} > file size {len(d)}")
off = 0
for i, (o, s) in enumerate(segs):
    if o != off or s == 0 or o + s > size: fail(f"segment {i} at {o:#x}+{s:#x} not contiguous")
    off = o + s
mod0 = struct.unpack_from("<I", d, 0x4)[0]
if d[mod0:mod0 + 4] != b"MOD0": fail(f"no MOD0 at {mod0:#x}")
if d[size:size + 4] != b"ASET": fail("no ASET asset header after the image")
icon_o, icon_s, nacp_o, nacp_s = struct.unpack_from("<QQQQ", d, size + 8)
icon = d[size + icon_o:size + icon_o + icon_s]
nacp = d[size + nacp_o:size + nacp_o + nacp_s]
if icon[:2] != b"\xff\xd8": fail("icon is not a JPEG")
if nacp_s != 0x4000: fail(f"NACP is {nacp_s:#x} bytes, expected 0x4000")
name = nacp[0:0x200].split(b"\0")[0].decode()
pub = nacp[0x200:0x300].split(b"\0")[0].decode()
ver = nacp[0x3060:0x3070].split(b"\0")[0].decode()
if (name, pub, ver) != (title, author, version): fail(f"NACP says {name!r}/{pub!r}/{ver!r}")
print(f"ok: {path}: {len(d)} bytes, image {size:#x}, text {segs[0][1]:#x} ro {segs[1][1]:#x} "
      f"data {segs[2][1]:#x} bss {bss:#x}, icon {icon_s} B, NACP {name!r} by {pub!r} v{ver}")
PY
else
    echo "python3 not found; skipped the NRO header check"
fi
echo "Built $DIST/$NAME.nro (ELF with symbols: $BUILD/$NAME.elf)"
