#!/usr/bin/env bash
# Prepares the final Switch test pass (docs/switch-test.md): builds every Switch program into dist/ from a snapshot,
# then packages the player's own discs into ~/slynx-packaged/ and prints where each file goes on the SD card.
#
#   tools/switch-test-package.sh             dry run: build the program .nro files into dist/, check the space,
#                                            print the packaging commands and the SD card paths
#   tools/switch-test-package.sh --package   the same, then package for real
#
# Run in WSL. Nothing is copied to a Switch. Packages hold game data: they go to ~/slynx-packaged/ only (never into
# the synced tree), and the script refuses to start without enough free space there (at least 15 GB).
#
# Disc images: SLYNX_DISC_SLY1, SLYNX_DISC_SLY2, SLYNX_DISC_SLY3 (default: the USA images in ../ISOs). Icons:
# SLYNX_ICONS (default ../Extracted/Switch/icons with sly1.jpg, sly2.jpg, sly3.jpg).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GAMES="$(cd "$ROOT/.." && pwd)"
ISOS="$GAMES/ISOs"
SLY1="${SLYNX_DISC_SLY1:-$ISOS/Sly Cooper and the Thievius Raccoonus (USA).iso}"
SLY2="${SLYNX_DISC_SLY2:-$ISOS/Sly 2 - Band of Thieves (USA).iso}"
SLY3="${SLYNX_DISC_SLY3:-$ISOS/Sly 3 - Honor Among Thieves (USA).iso}"
ICONS="${SLYNX_ICONS:-$GAMES/Extracted/Switch/icons}"
OUT="${SLYNX_PACKAGED:-$HOME/slynx-packaged}"
MIN_FREE_GB=15

PACKAGE=0
case "${1:-}" in
    "") ;;
    --package) PACKAGE=1 ;;
    *) echo "usage: $0 [--package]" >&2; exit 2 ;;
esac

for f in "$SLY1" "$SLY2" "$SLY3" "$ICONS/sly1.jpg" "$ICONS/sly2.jpg" "$ICONS/sly3.jpg"; do
    [ -f "$f" ] || { echo "missing: $f" >&2; exit 1; }
done

# Each package: name | program in dist/ | disc | icon (empty: the packager's default) | title | name on the SD card.
# The tools carry the Sly 1 disc in their RomFS, as the games carry theirs, so nothing else goes on the SD card.
JOBS=(
    "sly1|slynx.nro|$SLY1|$ICONS/sly1.jpg|SlyNX 1|slynx-sly1.nro"
    "sly2|slynx-sly2.nro|$SLY2|$ICONS/sly2.jpg|SlyNX 2|slynx-sly2.nro"
    "sly3|slynx-sly3.nro|$SLY3|$ICONS/sly3.jpg|SlyNX 3|slynx-sly3.nro"
    "viewer|slynx-viewer.nro|$SLY1||SlyNX viewer|slynx-viewer.nro"
    "smoke|slynx-smoke.nro|$SLY1||SlyNX smoke|slynx-smoke.nro"
    "video-bench|slynx-video-bench.nro|$SLY1||SlyNX video bench|slynx-video-bench.nro"
)

# ---- 1. Space: the larger of 15 GB and the discs packaged plus 1 GB.
mkdir -p "$OUT"
need=$((1 << 30))
for j in "${JOBS[@]}"; do
    IFS='|' read -r _ _ disc _ _ _ <<<"$j"
    need=$((need + $(stat -c %s "$disc")))
done
min=$((MIN_FREE_GB * (1 << 30)))
[ "$need" -lt "$min" ] && need=$min
free=$(df --output=avail -B1 "$OUT" | tail -1)
printf '== space: %d GB free in %s, %d GB needed\n' $((free >> 30)) "$OUT" $((need >> 30))
if [ "$free" -lt "$need" ]; then
    echo "not enough free space in $OUT" >&2
    exit 1
fi

# ---- 2. Build every program from a snapshot of its own (docs/switch-build.md, "Building from a snapshot").
SNAP="$(mktemp -d "$HOME/slynx-snap.XXXXXX")"
export CARGO_TARGET_DIR="$SNAP-target"
cleanup() { rm -rf "$SNAP" "$SNAP-target"; }
trap cleanup EXIT
(cd "$ROOT" && tar --exclude=./dist --exclude=./.git --exclude=./target -cf - .) | tar -xf - -C "$SNAP"
echo "== snapshot $SNAP"
(cd "$SNAP" && cargo build -q -p game) || { echo "the snapshot does not build; try again later" >&2; exit 1; }
cd "$SNAP"
mkdir -p dist
SLYNX_ICON="$ICONS/sly1.jpg" crates/game/tools/build-switch.sh
SLYNX_ICON="$ICONS/sly2.jpg" crates/game/tools/build-switch-sly2.sh
SLYNX_ICON="$ICONS/sly3.jpg" crates/game/tools/build-switch-sly3.sh
crates/game/tools/build-switch-smoke.sh
crates/viewer/tools/build-switch.sh
SLYNX_CRATE=video-bench SLYNX_NAME=slynx-video-bench SLYNX_TITLE="SlyNX video bench" tools/build-switch.sh
cargo build -q --release -p packager
PACK="$CARGO_TARGET_DIR/release/slynx-pack"
mkdir -p "$ROOT/dist"
for j in "${JOBS[@]}"; do
    IFS='|' read -r _ nro _ _ _ _ <<<"$j"
    cp "dist/$nro" "$ROOT/dist/$nro"
done
echo "== programs in $ROOT/dist: $(cd "$ROOT/dist" && ls slynx*.nro | tr '\n' ' ')"

# ---- 3. Package (or say how), then the SD card paths.
for j in "${JOBS[@]}"; do
    IFS='|' read -r name nro disc icon title _ <<<"$j"
    cmd=("$PACK" "$disc" --out "$OUT/$name" --program "$ROOT/dist/$nro" --title "$title")
    [ -n "$icon" ] && cmd+=(--icon "$icon")
    if [ "$PACKAGE" = 1 ]; then
        rm -rf "${OUT:?}/$name"
        "${cmd[@]}"
        "$PACK" --inspect "$OUT/$name/switch/SlyNX/"*.nro
    else
        printf '== would run:'
        printf ' %q' "${cmd[@]}"
        echo
    fi
done
# The video bench reads Sly 1 from its own RomFS.
mkdir -p "$OUT/sd/switch/SlyNX/user"
printf 'sly1 = romfs:/sly1.iso\n' >"$OUT/sd/switch/SlyNX/user/video-bench.cfg"

echo
echo "== SD card (copy single files; never drop whole folders):"
for j in "${JOBS[@]}"; do
    IFS='|' read -r name _ disc _ _ sd <<<"$j"
    n=1
    case "$disc" in "$SLY2") n=2 ;; "$SLY3") n=3 ;; esac
    printf '  %s/%s/switch/SlyNX/slynx-sly%d.nro  ->  sdmc:/switch/SlyNX/%s\n' "$OUT" "$name" "$n" "$sd"
done
printf '  %s/sd/switch/SlyNX/user/video-bench.cfg  ->  sdmc:/switch/SlyNX/user/video-bench.cfg\n' "$OUT"
[ "$PACKAGE" = 1 ] || echo "(dry run: nothing packaged; run with --package to make the files above)"
