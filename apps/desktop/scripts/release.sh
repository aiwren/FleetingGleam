#!/bin/sh
# Builds a protected release and the folder of sealed models to put on the model server.
#
#   FG_SEAL_KEY=<64 hex chars> FG_MODEL_BASE=https://models.example.com/fleetinggleam/v1 scripts/release.sh
#
# * FG_SEAL_KEY   secret; the same key for every release that uses the same sealed files (openssl rand -hex 32)
# * FG_MODEL_BASE where the sealed models will be hosted; the app downloads them from there, one pack at a time, when first needed
set -e
cd "$(dirname "$0")/.."
: "${FG_SEAL_KEY:?set FG_SEAL_KEY (64 hex characters, e.g. from: openssl rand -hex 32)}"
: "${FG_MODEL_BASE:?set FG_MODEL_BASE (the https:// folder the sealed models are uploaded to)}"

# Build paths (your home folder, the checkout) must not end up in panic messages inside the binary.
export RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$(pwd)=/src ${RUSTFLAGS:-}"

sh scripts/build-vision.sh

# 1. The sealed models and their manifest: upload this whole folder to FG_MODEL_BASE. The app itself contains no models.
rm -rf dist-models
( cd src-tauri && cargo run --release --example seal_models -- ../models ../dist-models )

# 2. The app.
pnpm install --frozen-lockfile
pnpm tauri build --config src-tauri/tauri.release.conf.json
if find src-tauri/target/release/bundle \( -name '*.onnx' -o -name '*.fgm' \) 2>/dev/null | grep -q .; then echo "a model file ended up inside the app bundle" >&2; exit 1; fi

echo "done."
echo " - upload dist-models/ (including manifest.json) to $FG_MODEL_BASE"
echo " - sign and notarise the .app with the hardened runtime before distributing (see docs/PROTECTION.md)"
