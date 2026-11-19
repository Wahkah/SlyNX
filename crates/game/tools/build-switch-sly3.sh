#!/usr/bin/env bash
# Builds the Sly 3 game program (crates/game, feature sly3-switch) into dist/slynx-sly3.nro.
#
#   crates/game/tools/build-switch-sly3.sh            release -> dist/slynx-sly3.nro
#   crates/game/tools/build-switch-sly3.sh --debug    debug   -> dist/slynx-sly3-debug.nro
#
# The same wrapper over tools/build-switch.sh as build-switch.sh, with the Sly 3 entry point (src/switch_sly3.rs).
# It looks for the player's disc image at sdmc:/switch/SlyNX/sly3.iso or romfs:/sly3.iso (see docs/switch-build.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export SLYNX_CRATE=game
export SLYNX_FEATURES=sly3-switch
export SLYNX_NAME=slynx-sly3
export SLYNX_TITLE="SlyNX 3"
export SLYNX_EXTRA_C=crates/render/c
export SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_
exec "$ROOT/tools/build-switch.sh" "$@"
