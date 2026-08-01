#!/usr/bin/env bash
# 跑全部测试

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> 跑全部测试..."

cargo test --workspace

echo "==> 测试通过 ✓"
