#!/usr/bin/env bash
#
# Build script for cross-compiling Rust to Android
#
# Prerequisites:
# 1. Install Android NDK (typically via Android Studio)
# 2. Install Rust Android targets:
#    rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
# 3. Set ANDROID_NDK_HOME environment variable
#
# Usage:
#   ./build-rust.sh [debug|release]
#

set -e

# Configuration
BUILD_MODE="${1:-debug}"
ANDROID_API_LEVEL=26
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/app/src/main/jniLibs"

# Android NDK path
if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "Error: ANDROID_NDK_HOME environment variable is not set"
    echo "Please set it to your Android NDK installation path"
    echo "Example: export ANDROID_NDK_HOME=\$HOME/Android/Sdk/ndk/26.1.10909125"
    exit 1
fi

if [ ! -d "$ANDROID_NDK_HOME" ]; then
    echo "Error: ANDROID_NDK_HOME directory does not exist: $ANDROID_NDK_HOME"
    exit 1
fi

echo "Using Android NDK: $ANDROID_NDK_HOME"
echo "Build mode: $BUILD_MODE"
echo "Project root: $PROJECT_ROOT"

# Set up NDK toolchain paths
NDK_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
export PATH="$NDK_TOOLCHAIN/bin:$PATH"

# Set CC and AR for each target
export CC_aarch64_linux_android="$NDK_TOOLCHAIN/bin/aarch64-linux-android${ANDROID_API_LEVEL}-clang"
export AR_aarch64_linux_android="$NDK_TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN/bin/aarch64-linux-android${ANDROID_API_LEVEL}-clang"

export CC_armv7_linux_androideabi="$NDK_TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang"
export AR_armv7_linux_androideabi="$NDK_TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$NDK_TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang"

export CC_i686_linux_android="$NDK_TOOLCHAIN/bin/i686-linux-android${ANDROID_API_LEVEL}-clang"
export AR_i686_linux_android="$NDK_TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_I686_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN/bin/i686-linux-android${ANDROID_API_LEVEL}-clang"

export CC_x86_64_linux_android="$NDK_TOOLCHAIN/bin/x86_64-linux-android${ANDROID_API_LEVEL}-clang"
export AR_x86_64_linux_android="$NDK_TOOLCHAIN/bin/llvm-ar"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK_TOOLCHAIN/bin/x86_64-linux-android${ANDROID_API_LEVEL}-clang"

# Build targets
TARGETS=(
    "aarch64-linux-android:arm64-v8a"
    "armv7-linux-androideabi:armeabi-v7a"
    "i686-linux-android:x86"
    "x86_64-linux-android:x86_64"
)

# Build flag
CARGO_FLAGS=""
if [ "$BUILD_MODE" = "release" ]; then
    CARGO_FLAGS="--release"
fi

echo "Building MyriadMesh Android library..."
echo ""

cd "$PROJECT_ROOT"

# Clean previous builds
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

# Build for each target
for target_pair in "${TARGETS[@]}"; do
    IFS=':' read -r rust_target android_abi <<< "$target_pair"

    echo "Building for $rust_target ($android_abi)..."

    cargo build $CARGO_FLAGS \
        --target "$rust_target" \
        -p myriadmesh-android

    # Copy the library to jniLibs
    if [ "$BUILD_MODE" = "release" ]; then
        BUILD_DIR="target/$rust_target/release"
    else
        BUILD_DIR="target/$rust_target/debug"
    fi

    LIB_FILE="$BUILD_DIR/libmyriadmesh_android.so"

    if [ -f "$LIB_FILE" ]; then
        mkdir -p "$OUTPUT_DIR/$android_abi"
        cp "$LIB_FILE" "$OUTPUT_DIR/$android_abi/"
        echo "✓ Copied to $OUTPUT_DIR/$android_abi/"
    else
        echo "✗ Library not found: $LIB_FILE"
        exit 1
    fi

    echo ""
done

echo "Build complete!"
echo "Libraries copied to: $OUTPUT_DIR"
echo ""
echo "You can now build the Android app with Gradle."
