#!/usr/bin/env bash
# Builds the renderer demo (crates/render, feature switch-demo) into dist/slynx-render-demo.nro.
#
#   crates/render/tools/build-switch-demo.sh            release -> dist/slynx-render-demo.nro
#   crates/render/tools/build-switch-demo.sh --debug    debug   -> dist/slynx-render-demo-debug.nro
#
# A thin wrapper over tools/build-switch.sh (see docs/switch-build.md): it picks the render crate and its demo
# feature, adds the renderer's C code (crates/render/c) and its shaders (crates/render/shaders, embedded as
# slynx_render_<name>.dksh). Other SLYNX_* variables (icon, build folder, RomFS) pass through.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export SLYNX_CRATE=render
export SLYNX_FEATURES=switch-demo
export SLYNX_NAME=slynx-render-demo
export SLYNX_TITLE="SlyNX render demo"
export SLYNX_EXTRA_C=crates/render/c
export SLYNX_EXTRA_SHADERS=crates/render/shaders=slynx_render_
exec "$ROOT/tools/build-switch.sh" "$@"
