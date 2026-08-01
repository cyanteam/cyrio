#!/usr/bin/env bash
# 编译 Web (WASM) 版
#
# 前置：
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli
#
# 用法：
#   ./scripts/build-web.sh          # debug
#   ./scripts/build-web.sh release  # release

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-debug}"
EXTRA_FLAGS=""

if [[ "$MODE" == "release" ]]; then
    EXTRA_FLAGS="--release"
fi

echo "==> 编译 Web (WASM) ($MODE)..."

# 检查 wasm-bindgen-cli
if ! command -v wasm-bindgen &> /dev/null; then
    echo "✗ 未安装 wasm-bindgen-cli，请先运行: cargo install wasm-bindgen-cli"
    exit 1
fi

cargo build $EXTRA_FLAGS --target wasm32-unknown-unknown -p cyrio-web

echo "==> 生成 JS 绑定..."

PKG_DIR="platforms/web/pkg"
mkdir -p "$PKG_DIR"

# 获取 wasm 文件路径
WASM_FILE="target/wasm32-unknown-unknown/$MODE/cyrio_web.wasm"
if [[ "$MODE" == "debug" && ! -f "$WASM_FILE" ]]; then
    WASM_FILE="target/wasm32-unknown-unknown/debug/cyrio_web.wasm"
fi

wasm-bindgen --out-dir "$PKG_DIR" --target web "$WASM_FILE"

# 拷贝 index.html 到 pkg/
cp platforms/web/index.html "$PKG_DIR/"

echo "==> 产物: $PKG_DIR/"
echo "==> 测试: ./scripts/serve-web.sh"
