#!/usr/bin/env bash
# 格式化 + lint 检查

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt..."
cargo fmt --all

echo "==> cargo clippy..."
cargo clippy --workspace --all-targets -- -D warnings

echo "==> 检查通过 ✓"
