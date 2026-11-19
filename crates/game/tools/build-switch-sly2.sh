#!/usr/bin/env bash
# Builds the Sly 2 game program (crates/game, feature sly2-switch) into dist/slynx-sly2.nro.
#
#   crates/game/tools/build-switch-sly2.sh            release -> dist/slynx-sly2.nro
#   crates/game/tools/build-switch-sly2.sh --debug    debug   -> dist/slynx-sly2-debug.nro
#
# The same wrapper over tools/build-switch.sh as build-switch.sh, with the Sly 2 entry point (src/switch_sly2.rs).
# It looks for the player's disc image at sdmc:/switch/SlyNX/sly2.iso or romfs:/sly2.iso (see docs/switch-build.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export SLYNX_CRATE=game
export SLYNX_FEATURES=sly2-switch
export SLYNX_NAME=slynx-sly2
export SLYNX_TITLE="SlyNX 2"
export SLYNX_EXTRA_C=crates/render/c
export SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_
exec "$ROOT/tools/build-switch.sh" "$@"
