#!/usr/bin/env bash
# Builds a Switch .nro from a staticlib crate, the platform shim and any extra C code and shaders. By default it
# builds the platform demo (crates/hello-switch) into dist/slynx-hello.nro.
#
#   tools/build-switch.sh            release build -> dist/<name>.nro
#   tools/build-switch.sh --debug    debug build   -> dist/<name>-debug.nro
#
# Steps: Rust staticlib (nightly, build-std, aarch64-nintendo-switch-freestanding) -> shaders (uam) -> C
# (devkitA64: the platform shim plus extra folders) -> link against libnx + deko3d with switch.specs -> NACP
# (nacptool) -> NRO with RomFS (elf2nro) -> header check.
#
# Run it on Linux or WSL with devkitPro installed. Environment overrides:
#   DEVKITPRO            default /opt/devkitpro
#   CARGO_TARGET_DIR     default $HOME/slynx-target (kept outside the repo and off synced folders)
#   SLYNX_BUILD_DIR      intermediate files, default $CARGO_TARGET_DIR/switch-build
#   SLYNX_ICON           256x256 JPEG for the Homebrew Menu, default libnx's default_icon.jpg
#   SLYNX_CRATE          the staticlib crate to link, default hello-switch
#   SLYNX_FEATURES       cargo features for that crate (comma- or space-separated), default none
#   SLYNX_NAME           output name without .nro, default slynx-hello for hello-switch, slynx-<crate> otherwise
#   SLYNX_TITLE          title shown in the Homebrew Menu, default SlyNX
#   SLYNX_ROMFS          folder packed as the .nro's RomFS, default crates/<crate>/romfs if it exists
#   SLYNX_EXTRA_C        colon-separated folders whose *.c and *.S files are compiled and linked too
#   SLYNX_EXTRA_SHADERS  colon-separated shader folders, each optionally `folder=prefix`; every <name>.glsl
#                        becomes <prefix><name>.dksh next to the platform's shaders, for .incbin from *.S files
# Relative paths are relative to the repository root. Shader stages come from the file name: *_vsh.glsl vertex,
# *_fsh.glsl fragment, *_gsh.glsl geometry, *_csh.glsl compute, *_tcsh.glsl / *_tesh.glsl tessellation.
#
# Example (the renderer demo, see crates/render/tools/build-switch-demo.sh):
#   SLYNX_CRATE=render SLYNX_FEATURES=switch-demo SLYNX_NAME=slynx-render-demo SLYNX_TITLE="SlyNX render demo" \
#   SLYNX_EXTRA_C=crates/render/c SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_ tools/build-switch.sh
set -euo pipefail

usage() {
    echo "usage: tools/build-switch.sh [--debug]"
    echo "  (no option)  release build -> dist/<name>.nro"
    echo "  --debug      debug build   -> dist/<name>-debug.nro"
    echo "  crate, features, name, extra C and shaders: see the SLYNX_* variables at the top of this script"
}

PROFILE=release
for arg in "$@"; do
    case "$arg" in
        --debug) PROFILE=debug ;;
        -h | --help) usage; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Makes a path from the environment absolute (relative to the repository root).
abs() { case "$1" in /*) echo "$1" ;; *) echo "$ROOT/$1" ;; esac; }

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/slynx-target}"

CRATE="${SLYNX_CRATE:-hello-switch}"
FEATURES="${SLYNX_FEATURES:-}"
if [[ "$CRATE" == hello-switch ]]; then DEFAULT_NAME=slynx-hello; else DEFAULT_NAME="slynx-$CRATE"; fi
NAME="${SLYNX_NAME:-$DEFAULT_NAME}"
[[ "$PROFILE" == debug ]] && NAME="$NAME-debug"
TITLE="${SLYNX_TITLE:-SlyNX}"
AUTHOR="Wahkah"
VERSION="0.0.1"

BUILD="${SLYNX_BUILD_DIR:-$CARGO_TARGET_DIR/switch-build}/$PROFILE/$NAME"
DIST="$ROOT/dist"
ICON="${SLYNX_ICON:-$DEVKITPRO/libnx/default_icon.jpg}"
ROMFS="$(abs "${SLYNX_ROMFS:-crates/$CRATE/romfs}")"

TARGET=aarch64-nintendo-switch-freestanding
A64="$DEVKITPRO/devkitA64/bin/aarch64-none-elf-"
TOOLS="$DEVKITPRO/tools/bin"
LIBNX="$DEVKITPRO/libnx"
SHIM="$ROOT/crates/platform-switch/c"

for tool in "${A64}gcc" "$TOOLS/uam" "$TOOLS/elf2nro" "$TOOLS/nacptool"; do
    [[ -x "$tool" ]] || { echo "missing $tool (is devkitPro installed with switch-dev and deko3d?)" >&2; exit 1; }
done
[[ -f "$LIBNX/lib/libdeko3d.a" ]] || { echo "missing libdeko3d.a (install the deko3d package)" >&2; exit 1; }
if [[ -f "$HOME/.cargo/env" ]]; then . "$HOME/.cargo/env"; fi

# Extra C folders and shader folders (with their prefixes), checked up front.
EXTRA_C=()
if [[ -n "${SLYNX_EXTRA_C:-}" ]]; then
    IFS=: read -r -a dirs <<<"$SLYNX_EXTRA_C"
    for d in "${dirs[@]}"; do
        [[ -z "$d" ]] && continue
        d="$(abs "$d")"
        [[ -d "$d" ]] || { echo "SLYNX_EXTRA_C: no folder $d" >&2; exit 1; }
        EXTRA_C+=("$d")
    done
fi
SHADER_DIRS=("$SHIM/shaders")
SHADER_PREFIXES=("")
if [[ -n "${SLYNX_EXTRA_SHADERS:-}" ]]; then
    IFS=: read -r -a entries <<<"$SLYNX_EXTRA_SHADERS"
    for e in "${entries[@]}"; do
        [[ -z "$e" ]] && continue
        dir="${e%%=*}"
        prefix=""
        [[ "$e" == *=* ]] && prefix="${e#*=}"
        dir="$(abs "$dir")"
        [[ -d "$dir" ]] || { echo "SLYNX_EXTRA_SHADERS: no folder $dir" >&2; exit 1; }
        SHADER_DIRS+=("$dir")
        SHADER_PREFIXES+=("$prefix")
    done
fi

# Start clean: stale objects or shaders from removed sources must not end up in the link.
rm -rf "$BUILD"
mkdir -p "$BUILD/shaders" "$BUILD/obj" "$DIST"

echo "== Rust staticlib: $CRATE ($PROFILE${FEATURES:+, features $FEATURES})"
CARGO_ARGS=(-p "$CRATE")
[[ "$PROFILE" == release ]] && CARGO_ARGS+=(--release)
[[ -n "$FEATURES" ]] && CARGO_ARGS+=(--features "$FEATURES")
(cd "$ROOT" && cargo +nightly build "${CARGO_ARGS[@]}" -Z build-std=core,alloc --target "$TARGET")
RUSTLIB="$CARGO_TARGET_DIR/$TARGET/$PROFILE/lib${CRATE//-/_}.a"
[[ -f "$RUSTLIB" ]] || { echo "no staticlib at $RUSTLIB (is $CRATE a staticlib crate?)" >&2; exit 1; }

echo "== Shaders (uam)"
stage_of() {
    case "$1" in
        *_vsh.glsl) echo vert ;;
        *_fsh.glsl) echo frag ;;
        *_gsh.glsl) echo geom ;;
        *_csh.glsl) echo comp ;;
        *_tcsh.glsl) echo tess_ctrl ;;
        *_tesh.glsl) echo tess_eval ;;
        *) echo "unknown shader stage (name it *_vsh, *_fsh, *_gsh, *_csh, *_tcsh or *_tesh): $1" >&2; return 1 ;;
    esac
}
for i in "${!SHADER_DIRS[@]}"; do
    for src in "${SHADER_DIRS[$i]}"/*.glsl; do
        [[ -e "$src" ]] || continue
        out="$BUILD/shaders/${SHADER_PREFIXES[$i]}$(basename "${src%.glsl}").dksh"
        [[ -e "$out" ]] && { echo "two shaders map to $(basename "$out"); give the folder a prefix" >&2; exit 1; }
        "$TOOLS/uam" -s "$(stage_of "$src")" -o "$out" "$src"
    done
done

echo "== C (platform shim${EXTRA_C:+ + ${#EXTRA_C[@]} extra folder(s)})"
ARCH=(-march=armv8-a+crc+crypto -mtune=cortex-a57 -mtp=soft -fPIE)
CFLAGS=(-g -O2 -Wall -Wextra -ffunction-sections -fdata-sections -D__SWITCH__ -I"$LIBNX/include" -I"$SHIM"
    "${ARCH[@]}")
OBJS=()
n=0
for dir in "$SHIM" "${EXTRA_C[@]}"; do
    for src in "$dir"/*.c "$dir"/*.S; do
        [[ -e "$src" ]] || continue
        n=$((n + 1))
        # Numbered so files with the same name in different folders do not collide.
        obj="$BUILD/obj/$n-$(basename "$src").o"
        case "$src" in
            *.c) "${A64}gcc" "${CFLAGS[@]}" -I"$dir" -std=gnu11 -c "$src" -o "$obj" ;;
            *.S) "${A64}gcc" "${ARCH[@]}" -g -I"$dir" -c "$src" -Wa,-I,"$BUILD/shaders" -o "$obj" ;;
        esac
        OBJS+=("$obj")
    done
done

echo "== Link"
# deko3d before nx: libdeko3d depends on libnx. switch.specs adds the linker script, crt0 and -pie.
"${A64}gcc" -specs="$LIBNX/switch.specs" -g "${ARCH[@]}" -Wl,-Map,"$BUILD/$NAME.map" \
    "${OBJS[@]}" "$RUSTLIB" \
    -L"$LIBNX/lib" -ldeko3d -lnx -o "$BUILD/$NAME.elf"

echo "== NACP + NRO"
"$TOOLS/nacptool" --create "$TITLE" "$AUTHOR" "$VERSION" "$BUILD/$NAME.nacp"
ROMFS_ARG=()
if [[ -d "$ROMFS" ]]; then
    ROMFS_ARG=(--romfsdir="$ROMFS")
else
    echo "no RomFS folder at $ROMFS; building without one"
fi
"$TOOLS/elf2nro" "$BUILD/$NAME.elf" "$DIST/$NAME.nro" --icon="$ICON" --nacp="$BUILD/$NAME.nacp" "${ROMFS_ARG[@]}"

echo "== Check"
if command -v python3 >/dev/null; then
    python3 -I - "$DIST/$NAME.nro" "$TITLE" "$AUTHOR" "$VERSION" "${#ROMFS_ARG[@]}" <<'PY'
import struct, sys
path, title, author, version, want_romfs = sys.argv[1:6]
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
romfs_o, romfs_s = struct.unpack_from("<QQ", d, size + 0x28)
if want_romfs != "0":
    # A RomFS image starts with its header size, 0x50.
    if romfs_s < 0x50 or size + romfs_o + romfs_s > len(d): fail("RomFS section missing or truncated")
    if struct.unpack_from("<Q", d, size + romfs_o)[0] != 0x50: fail("RomFS header size is not 0x50")
print(f"ok: {path}: {len(d)} bytes, image {size:#x}, text {segs[0][1]:#x} ro {segs[1][1]:#x} "
      f"data {segs[2][1]:#x} bss {bss:#x}, icon {icon_s} B, RomFS {romfs_s} B, NACP {name!r} by {pub!r} v{ver}")
PY
else
    echo "python3 not found; skipped the NRO header check"
fi
echo "Built $DIST/$NAME.nro (ELF with symbols: $BUILD/$NAME.elf)"
