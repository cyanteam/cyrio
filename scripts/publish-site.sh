#!/usr/bin/env bash
# ============================================================================
# publish-site.sh — 同步 release_public 到 ../_site 并推送到 cyrio-website
#
# 用法：
#   ./scripts/publish-site.sh
#
# 前提：
#   - release_public/ 已包含 index.html、screenshots/、downloads/
#   - ../_site 已 git init 并关联 cyrio-website 远程仓库
# ============================================================================

set -euo pipefail

# ── 颜色 ──
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ── 路径 ──
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RELEASE_PUBLIC="$PROJECT_ROOT/release_public"
SITE_DIR="$(cd "$PROJECT_ROOT/.." && pwd)/_site"

# ── 检查 ──
[[ -f "$RELEASE_PUBLIC/index.html" ]] || error "release_public/index.html 不存在"
[[ -d "$RELEASE_PUBLIC/screenshots" ]] || error "release_public/screenshots/ 不存在"

info "源目录: $RELEASE_PUBLIC"
info "目标目录: $SITE_DIR"

# ── 同步 ──
info "同步文件到 _site..."

# 用 rsync 同步（排除 .git），删除目标端多余文件
rsync -a --delete --exclude='.git' "$RELEASE_PUBLIC/" "$SITE_DIR/"

ok "文件已同步"

# ── Git 提交推送 ──
info "Git 提交并推送..."

cd "$SITE_DIR"
git add -A

if git diff --cached --quiet; then
    warn "没有变更，跳过推送"
    exit 0
fi

git commit -m "更新网站: $(date '+%Y-%m-%d %H:%M')"
git push origin main

ok "已推送到 cyrio-website"
echo ""
echo "========================================"
ok "发布完成!"
echo "  站点目录: $SITE_DIR"
echo "  仓库: https://github.com/cyanteam/cyrio-website"
echo "========================================"
