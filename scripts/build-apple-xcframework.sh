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

echo "=== AeroLog Apple XCFramework Builder ==="

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "Warning: Apple XCFramework assembly requires macOS (Darwin) with Xcode and lipo." >&2
    echo "This script should be run on a macOS development machine or macOS GitHub Actions runner." >&2
    if [[ "${1:-}" != "--force" ]]; then
        echo "Pass --force to continue cross-compilation of available targets only." >&2
        exit 1
    fi
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo command not found in PATH." >&2
    exit 1
fi

echo "Installing Apple Rust target triples..."
rustup target add \
    aarch64-apple-ios \
    aarch64-apple-ios-sim \
    x86_64-apple-ios-sim \
    aarch64-apple-darwin \
    x86_64-apple-darwin

cd "${ROOT_DIR}"

echo "Building static libraries across Apple targets..."
cargo build --release -p aerolog-uniffi --target aarch64-apple-ios
cargo build --release -p aerolog-uniffi --target aarch64-apple-ios-sim
cargo build --release -p aerolog-uniffi --target x86_64-apple-ios-sim
cargo build --release -p aerolog-uniffi --target aarch64-apple-darwin
cargo build --release -p aerolog-uniffi --target x86_64-apple-darwin

INTERMEDIATE_DIR="${ROOT_DIR}/target/apple-universal"
mkdir -p "${INTERMEDIATE_DIR}/ios-sim"
mkdir -p "${INTERMEDIATE_DIR}/macos"

echo "Creating universal binary for iOS Simulator (arm64 + x86_64)..."
lipo -create \
    "${ROOT_DIR}/target/aarch64-apple-ios-sim/release/libaerolog_uniffi.a" \
    "${ROOT_DIR}/target/x86_64-apple-ios-sim/release/libaerolog_uniffi.a" \
    -output "${INTERMEDIATE_DIR}/ios-sim/libaerolog_uniffi.a"

echo "Creating universal binary for macOS (arm64 + x86_64)..."
lipo -create \
    "${ROOT_DIR}/target/aarch64-apple-darwin/release/libaerolog_uniffi.a" \
    "${ROOT_DIR}/target/x86_64-apple-darwin/release/libaerolog_uniffi.a" \
    -output "${INTERMEDIATE_DIR}/macos/libaerolog_uniffi.a"

OUTPUT_XCFRAMEWORK="${ROOT_DIR}/bindings/swift/AeroLogUniFFI.xcframework"
rm -rf "${OUTPUT_XCFRAMEWORK}"

echo "Packaging AeroLogUniFFI.xcframework..."
xcodebuild -create-xcframework \
    -library "${ROOT_DIR}/target/aarch64-apple-ios/release/libaerolog_uniffi.a" \
    -headers "${ROOT_DIR}/bindings/swift/include" \
    -library "${INTERMEDIATE_DIR}/ios-sim/libaerolog_uniffi.a" \
    -headers "${ROOT_DIR}/bindings/swift/include" \
    -library "${INTERMEDIATE_DIR}/macos/libaerolog_uniffi.a" \
    -headers "${ROOT_DIR}/bindings/swift/include" \
    -output "${OUTPUT_XCFRAMEWORK}"

echo "Apple XCFramework successfully built at: ${OUTPUT_XCFRAMEWORK}"
