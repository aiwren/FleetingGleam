#!/usr/bin/env bash
# 两端一致性（路线图阶段 B 第 6 步）：把 Android 绑定 crate 的测试编成 arm64 程序，推到连着的手机 / 模拟器上跑，
# 和 Mac 上 `cargo test` 比的是同一份基准（packages/core/consistency*.golden.json）。
# 用法：apps/android/scripts/consistency.sh [模型文件夹]
#   不给模型文件夹：只比"没有模型"那一份；给了（Mac 上的 apps/desktop/models），会把标准档要的几个文件推上去再比"装了模型"那一份。
set -euo pipefail
cd "$(dirname "$0")/../../.."
export PATH="$HOME/.cargo/bin:$HOME/Library/Android/sdk/platform-tools:$PATH"
cargo ndk -t arm64-v8a -P 31 test -p fleetinggleam-android --lib --no-run >/dev/null 2>&1
# 测试程序：最新的那个 fg_android-<哈希>（没有扩展名的可执行文件）
OUT=$(ls -t target/aarch64-linux-android/*/deps/fg_android-* 2>/dev/null | grep -vE '\.(d|so|rlib|rmeta)$' | head -1)
[ -n "$OUT" ] || { echo "没编出测试程序"; exit 1; }
DEV=/data/local/tmp/fg-consistency
adb shell "rm -rf $DEV && mkdir -p $DEV/models"
adb push "$OUT" $DEV/test >/dev/null
# 测试程序要 libc++_shared.so
LIBCXX=$(find "${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk}" -path "*aarch64-linux-android/libc++_shared.so" | head -1)
adb push "$LIBCXX" $DEV/ >/dev/null
ENVS="LD_LIBRARY_PATH=$DEV TMPDIR=$DEV"
if [ $# -ge 1 ]; then
  for f in clip_vit_b32_vision_fp16.onnx clip_vit_b32_vision_int8.onnx aesthetic_head_vit_b_32.json; do
    [ -f "$1/$f" ] && adb push "$1/$f" $DEV/models/ >/dev/null
  done
  ENVS="$ENVS FG_MODELS=$DEV/models"
fi
adb shell "cd $DEV && $ENVS ./test consistency --nocapture --test-threads=1"
