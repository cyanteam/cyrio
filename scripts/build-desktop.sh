#!/usr/bin/env bash
# 编译桌面版（当前平台：macOS / Linux / Windows）
#
# 用法：
#   ./scripts/build-desktop.sh          # debug
#   ./scripts/build-desktop.sh release  # release

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-debug}"
TARGET_DIR="target/debug"
EXTRA_FLAGS=""

if [[ "$MODE" == "release" ]]; then
    TARGET_DIR="target/release"
    EXTRA_FLAGS="--release"
fi

echo "==> 编译桌面版 ($MODE)..."

cargo build $EXTRA_FLAGS -p cyrio-egui

BINARY="cyrio-egui"
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    BINARY="$BINARY.exe"
fi

echo "==> 产物: $TARGET_DIR/$BINARY"
echo "==> 运行: ./$TARGET_DIR/$BINARY"
