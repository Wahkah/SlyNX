#!/usr/bin/env bash
# Builds the level viewer (crates/viewer) into dist/slynx-viewer.nro.
#
#   crates/viewer/tools/build-switch.sh            release -> dist/slynx-viewer.nro
#   crates/viewer/tools/build-switch.sh --debug    debug   -> dist/slynx-viewer-debug.nro
#
# A thin wrapper over tools/build-switch.sh (see docs/switch-build.md): it picks the viewer crate and adds the
# renderer's C code (crates/render/c) and shaders (crates/render/shaders, embedded as slynx_render_<name>.dksh).
# Other SLYNX_* variables (icon, build folder, RomFS) pass through.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export SLYNX_CRATE=viewer
export SLYNX_FEATURES=switch-app
export SLYNX_NAME=slynx-viewer
export SLYNX_TITLE=SlyNX
export SLYNX_EXTRA_C=crates/render/c
export SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_
exec "$ROOT/tools/build-switch.sh" "$@"
