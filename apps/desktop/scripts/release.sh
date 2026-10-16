#!/bin/sh
# Builds a release of the app on this machine (macOS). No models are in it: the app downloads them when they are first needed.
#
#   scripts/release.sh
set -e
cd "$(dirname "$0")/.."

# Build paths (your home folder, the checkout) must not end up in panic messages inside the binary.
export RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$(pwd)=/src ${RUSTFLAGS:-}"

sh scripts/build-vision.sh

pnpm install --frozen-lockfile
pnpm tauri build --config src-tauri/tauri.release.conf.json
if find src-tauri/target/release/bundle -name '*.onnx' 2>/dev/null | grep -q .; then echo "a model file ended up inside the app bundle" >&2; exit 1; fi

echo "done."
echo " - sign and notarise the .app before distributing it (see docs/RELEASING.md)"
