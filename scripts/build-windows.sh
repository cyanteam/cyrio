#!/usr/bin/env bash
# 交叉编译 Windows 版（无需 Docker，使用 cargo-xwin）
#
# 前置：
#   cargo install cargo-xwin
#
# 用法：
#   ./scripts/build-windows.sh          # debug
#   ./scripts/build-windows.sh release  # release

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-debug}"
TARGET="x86_64-pc-windows-msvc"
EXTRA_FLAGS=""

if [[ "$MODE" == "release" ]]; then
    EXTRA_FLAGS="--release"
fi

echo "==> 交叉编译 Windows ($MODE)..."

# 检查 cargo-xwin
if ! command -v cargo-xwin &> /dev/null; then
    echo "✗ 未安装 cargo-xwin，请先运行: cargo install cargo-xwin"
    exit 1
fi

cargo xwin build $EXTRA_FLAGS --target $TARGET -p cyrio-egui

echo "==> 产物: target/$TARGET/$MODE/cyrio-egui.exe"
echo "==> 拷贝到: target/cyrio-windows.exe"
cp "target/$TARGET/$MODE/cyrio-egui.exe" "target/cyrio-windows.exe" 2>/dev/null || true
