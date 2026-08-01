#!/usr/bin/env bash
# 启动本地 web 服务器测试 WASM 版
#
# 前置：
#   cargo install miniserve  (或用 python3 -m http.server 替代)
#
# 用法：
#   ./scripts/serve-web.sh         # 默认 8080
#   ./scripts/serve-web.sh 9000    # 指定端口

set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${1:-8080}"
PKG_DIR="platforms/web/pkg"

if [[ ! -d "$PKG_DIR" ]]; then
    echo "✗ 未找到 $PKG_DIR，请先运行: ./scripts/build-web.sh"
    exit 1
fi

echo "==> 启动 web 服务器: http://localhost:$PORT"
echo "    (Ctrl+C 退出)"

if command -v miniserve &> /dev/null; then
    miniserve --port "$PORT" --index index.html "$PKG_DIR"
else
    echo "    (miniserve 未安装，用 python3 替代)"
    cd "$PKG_DIR"
    python3 -m http.server "$PORT"
fi
