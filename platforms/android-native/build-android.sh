#!/usr/bin/env bash
# ============================================================================
# build-android.sh — 交叉编译 Rust cyrio-jni 并构建 Android APK
#
# 流程：
#   1. 检查 NDK / Rust target / Android SDK
#   2. 交叉编译 cyrio-jni → libcyrio_jni.so (aarch64-linux-android)
#   3. 复制 .so 到 app/src/main/jniLibs/arm64-v8a/
#   4. Gradle 构建 APK（debug 或 release）
#
# 用法：
#   ./build-android.sh          # debug 构建
#   ./build-android.sh release  # release 构建
#   ./build-android.sh clean    # 清理后构建
# ============================================================================

set -euo pipefail

# ── 颜色输出 ──
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# ── 路径常量 ──
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ANDROID_DIR="$SCRIPT_DIR"
JNI_LIBS_DIR="$ANDROID_DIR/app/src/main/jniLibs/arm64-v8a"
NDK_DIR="/opt/homebrew/share/android-commandlinetools/ndk/27.0.12077973"
RUST_TARGET="aarch64-linux-android"

# ── 构建参数 ──
BUILD_TYPE="${1:-debug}"
CLEAN=false
if [[ "$BUILD_TYPE" == "clean" ]]; then
    CLEAN=true
    BUILD_TYPE="debug"
fi

echo ""
echo "========================================"
echo "  cyrio Android Native Build"
echo "  Type: $BUILD_TYPE"
echo "========================================"
echo ""

# ============================================================================
# 1. 前置检查
# ============================================================================

info "检查前置依赖..."

# 检查 NDK
if [[ ! -d "$NDK_DIR" ]]; then
    error "NDK 未找到: $NDK_DIR\n请通过 Android Studio SDK Manager 安装 NDK 27.0.12077973"
fi
ok "NDK: $NDK_DIR"

# 检查 Android SDK
ANDROID_SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [[ ! -d "$ANDROID_SDK" ]]; then
    error "Android SDK 未找到: $ANDROID_SDK\n请设置 ANDROID_HOME 环境变量"
fi
ok "Android SDK: $ANDROID_SDK"

# 检查 Rust target
if ! rustup target list --installed 2>/dev/null | grep -q "$RUST_TARGET"; then
    warn "Rust target $RUST_TARGET 未安装，正在安装..."
    rustup target add "$RUST_TARGET"
fi
ok "Rust target: $RUST_TARGET"

# ============================================================================
# 2. 交叉编译 Rust cyrio-jni
# ============================================================================

info "交叉编译 cyrio-jni → libcyrio_jni.so..."

cd "$PROJECT_ROOT"

if [[ "$CLEAN" == true ]]; then
    info "清理 Rust 构建产物..."
    cargo clean
fi

# 编译参数
if [[ "$BUILD_TYPE" == "release" ]]; then
    CARGO_FLAG="--release"
    PROFILE_DIR="release"
else
    CARGO_FLAG=""
    PROFILE_DIR="debug"
fi

# 交叉编译 cyrio-jni
cargo build $CARGO_FLAG -p cyrio-jni --target "$RUST_TARGET"

# 检查 .so 是否生成
SO_FILE="$PROJECT_ROOT/target/$RUST_TARGET/$PROFILE_DIR/libcyrio_jni.so"
if [[ ! -f "$SO_FILE" ]]; then
    error "编译失败: $SO_FILE 不存在"
fi

ok "编译成功: $(du -h "$SO_FILE" | cut -f1) libcyrio_jni.so"

# ============================================================================
# 3. 复制 .so 到 jniLibs
# ============================================================================

info "复制 .so 到 jniLibs..."

mkdir -p "$JNI_LIBS_DIR"
cp -f "$SO_FILE" "$JNI_LIBS_DIR/libcyrio_jni.so"

ok "已复制到: $JNI_LIBS_DIR/libcyrio_jni.so"

# ============================================================================
# 4. 生成 local.properties
# ============================================================================

info "生成 local.properties..."

cat > "$ANDROID_DIR/local.properties" << EOF
sdk.dir=$ANDROID_SDK
EOF

ok "local.properties 已生成"

# ============================================================================
# 5. Gradle 构建 APK
# ============================================================================

info "Gradle 构建 APK ($BUILD_TYPE)..."

cd "$ANDROID_DIR"

# 检查 gradlew 是否存在
if [[ ! -f "$ANDROID_DIR/gradlew" ]]; then
    warn "gradlew 不存在，使用系统 gradle 生成 wrapper..."
    if command -v gradle &>/dev/null; then
        gradle wrapper --gradle-version 8.9
        chmod +x gradlew
    else
        warn "系统 gradle 也未安装，尝试直接下载 wrapper..."
        # 下载 gradle-wrapper.jar
        WRAPPER_DIR="$ANDROID_DIR/gradle/wrapper"
        WRAPPER_JAR="$WRAPPER_DIR/gradle-wrapper.jar"
        if [[ ! -f "$WRAPPER_JAR" ]]; then
            curl -sL "https://raw.githubusercontent.com/gradle/gradle/v8.9.0/gradle/wrapper/gradle-wrapper.jar" -o "$WRAPPER_JAR"
        fi
        # 下载 gradlew 脚本
        curl -sL "https://raw.githubusercontent.com/gradle/gradle/v8.9.0/gradlew" -o "$ANDROID_DIR/gradlew"
        chmod +x "$ANDROID_DIR/gradlew"
        curl -sL "https://raw.githubusercontent.com/gradle/gradle/v8.9.0/gradlew.bat" -o "$ANDROID_DIR/gradlew.bat"
    fi
fi

# 执行 Gradle 构建
if [[ "$BUILD_TYPE" == "release" ]]; then
    ./gradlew assembleRelease --no-daemon
    APK_PATH="$ANDROID_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
else
    ./gradlew assembleDebug --no-daemon
    APK_PATH="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
fi

# 检查 APK
if [[ ! -f "$APK_PATH" ]]; then
    error "APK 构建失败: $APK_PATH 不存在"
fi

ok "APK 构建成功: $(du -h "$APK_PATH" | cut -f1)"

echo ""
echo "========================================"
ok "构建完成!"
echo "  APK: $APK_PATH"
echo "========================================"
echo ""

# ============================================================================
# 6. 可选：安装到设备
# ============================================================================

if [[ "${2:-}" == "install" ]]; then
    info "安装 APK 到设备..."
    adb install -r "$APK_PATH" || warn "adb 安装失败，请检查设备连接"
    ok "安装完成"
fi
