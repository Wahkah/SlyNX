#!/usr/bin/env bash
# Builds the Switch smoke build (crates/game, feature smoke-switch) into dist/slynx-smoke.nro: the Sly 1 smoke
# route of src/smoke.rs, run once with no player input, logged to sdmc:/switch/SlyNX/user/smoke.log.
#
#   crates/game/tools/build-switch-smoke.sh            release -> dist/slynx-smoke.nro
#   crates/game/tools/build-switch-smoke.sh --debug    debug   -> dist/slynx-smoke-debug.nro
#
# A thin wrapper over tools/build-switch.sh (see docs/switch-build.md), as build-switch.sh is for the game.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export SLYNX_CRATE=game
export SLYNX_FEATURES=smoke-switch
export SLYNX_NAME=slynx-smoke
export SLYNX_TITLE="SlyNX smoke"
export SLYNX_EXTRA_C=crates/render/c
export SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_
exec "$ROOT/tools/build-switch.sh" "$@"
