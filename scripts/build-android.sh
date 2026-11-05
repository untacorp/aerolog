#!/usr/bin/env bash
#
# Copyright 2026 AeroLog Authors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== AeroLog Android Cross-Compilation Builder ==="

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo command not found in PATH." >&2
    exit 1
fi

if ! cargo ndk --version >/dev/null 2>&1; then
    echo "Error: cargo-ndk is required but not installed." >&2
    echo "Install via: cargo install cargo-ndk" >&2
    echo "Ensure Android NDK is installed and ANDROID_NDK_HOME is exported." >&2
    exit 1
fi

if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    if [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
        LATEST_NDK="$(find "${ANDROID_HOME}/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
        if [ -n "${LATEST_NDK}" ]; then
            export ANDROID_NDK_HOME="${LATEST_NDK}"
            echo "Auto-detected ANDROID_NDK_HOME: ${ANDROID_NDK_HOME}"
        fi
    fi
fi

if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${ANDROID_NDK_ROOT:-}" ]; then
    echo "Error: ANDROID_NDK_HOME or ANDROID_NDK_ROOT environment variable must be set." >&2
    exit 1
fi

TARGET_OUT_DIR="${ROOT_DIR}/bindings/kotlin/src/main/jniLibs"
mkdir -p "${TARGET_OUT_DIR}/arm64-v8a"
mkdir -p "${TARGET_OUT_DIR}/armeabi-v7a"
mkdir -p "${TARGET_OUT_DIR}/x86_64"

echo "Installing required Rust target triples..."
rustup target add \
    aarch64-linux-android \
    armv7-linux-androideabi \
    x86_64-linux-android

echo "Compiling aerolog-uniffi with cargo-ndk..."
cd "${ROOT_DIR}"
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -o "${TARGET_OUT_DIR}" \
    build --release -p aerolog-uniffi

echo "Generating Kotlin UniFFI bindings..."
cargo run -p aerolog-uniffi --bin uniffi-bindgen generate \
    --library "${TARGET_OUT_DIR}/arm64-v8a/libaerolog_uniffi.so" \
    --language kotlin \
    --out-dir "${ROOT_DIR}/bindings/kotlin/src/main/kotlin"

echo "Android native binaries successfully compiled to: ${TARGET_OUT_DIR}"
