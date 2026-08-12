#!/usr/bin/env bash
# Dream Melody — a notation editor that stores scale degrees, not pitches.
# Copyright (C) 2026 mizaimao
#
# This program is free software: you can redistribute it and/or modify it
# under the terms of the GNU Affero General Public License as published by
# the Free Software Foundation, either version 3 of the License, or (at your
# option) any later version. See <https://www.gnu.org/licenses/>.
#
# Builds the wasm bindings twice: once as an ES module for the browser, and
# once for Node so the JS test suite can run without a browser.
#
# Needs the wasm32-unknown-unknown target and a wasm-bindgen CLI whose version
# matches the crate's. Both are checked below, because a mismatch produces a
# baffling runtime error rather than a build failure.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"

if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
  echo "missing target: run 'rustup target add wasm32-unknown-unknown'" >&2
  exit 1
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "missing wasm-bindgen: run 'brew install wasm-bindgen' or" >&2
  echo "'cargo install wasm-bindgen-cli --version \$wanted'" >&2
  exit 1
fi

wanted="$(grep -o 'wasm-bindgen = "=[0-9.]*"' "$here/Cargo.toml" | grep -o '[0-9][0-9.]*')"
found="$(wasm-bindgen --version | grep -o '[0-9][0-9.]*')"
if [ "$wanted" != "$found" ]; then
  echo "wasm-bindgen CLI is $found but the crate pins $wanted" >&2
  echo "the two must match, or the generated glue will not load" >&2
  exit 1
fi

profile="${1:-release}"
flag=""
if [ "$profile" = "release" ]; then flag="--release"; fi

echo "building dream-melody-wasm ($profile)"
cargo build -p dream-melody-wasm --target wasm32-unknown-unknown $flag

binary="$root/target/wasm32-unknown-unknown/$profile/dream_melody_wasm.wasm"

echo "generating ES modules for the browser -> web/wasm"
wasm-bindgen --target web --out-dir "$root/web/wasm" "$binary"

echo "generating CommonJS for Node -> core/wasm/pkg-node"
wasm-bindgen --target nodejs --out-dir "$here/pkg-node" "$binary"

echo "done. run the JS tests with: node --test 'core/wasm/tests/*.test.mjs'"
