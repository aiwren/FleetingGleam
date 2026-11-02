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
# 应用内更新的签名密钥：CI 用 Secrets 里的；本机默认读 ~/.config/fleetinggleam/updater.key（见 docs/RELEASING.md 的"应用内更新"）。
# 没有密钥 tauri build 会因 createUpdaterArtifacts 直接失败——没签名的更新包任何已装的应用都不会接受。
# （tauri-cli 2.11 只认 TAURI_SIGNING_PRIVATE_KEY，值是私钥文件的内容）
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$HOME/.config/fleetinggleam/updater.key" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.config/fleetinggleam/updater.key")"
  [ -f "$HOME/.config/fleetinggleam/updater.key.password" ] && export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$HOME/.config/fleetinggleam/updater.key.password")"
fi
pnpm tauri build --config src-tauri/tauri.release.conf.json
if find src-tauri/target/release/bundle -name '*.onnx' 2>/dev/null | grep -q .; then echo "a model file ended up inside the app bundle" >&2; exit 1; fi

echo "done."
echo " - sign and notarise the .app before distributing it (see docs/RELEASING.md)"
