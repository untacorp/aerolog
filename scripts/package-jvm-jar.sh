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
JVM_RESOURCES_DIR="${ROOT_DIR}/bindings/kotlin-jvm/src/main/resources"
JAR_OUTPUT="${ROOT_DIR}/target/aerolog-jvm-0.1.0.jar"

echo "=== AeroLog Multiplatform JVM JAR Packager ==="

mkdir -p "${JVM_RESOURCES_DIR}/linux-x86-64"
mkdir -p "${JVM_RESOURCES_DIR}/linux-aarch64"
mkdir -p "${JVM_RESOURCES_DIR}/darwin-aarch64"
mkdir -p "${JVM_RESOURCES_DIR}/darwin-x86-64"
mkdir -p "${JVM_RESOURCES_DIR}/win32-x86-64"

ARTIFACTS_DIR=""
BUILD_JAR=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --artifacts-dir)
            ARTIFACTS_DIR="$2"
            shift 2
            ;;
        --jar)
            BUILD_JAR=true
            shift
            ;;
        *)
            if [ -z "${ARTIFACTS_DIR}" ] && [ -d "$1" ]; then
                ARTIFACTS_DIR="$1"
            fi
            shift
            ;;
    esac
done

if [ -n "${ARTIFACTS_DIR}" ] && [ -d "${ARTIFACTS_DIR}" ]; then
    echo "Assembling native libraries from artifacts directory: ${ARTIFACTS_DIR}"

    find "${ARTIFACTS_DIR}" -type f \( -name "*linux*x86_64*.so" -o -name "*x86_64*linux*.so" \) \
        -exec cp {} "${JVM_RESOURCES_DIR}/linux-x86-64/libaerolog_uniffi.so" \; 2>/dev/null || true

    find "${ARTIFACTS_DIR}" -type f \( -name "*linux*aarch64*.so" -o -name "*aarch64*linux*.so" -o -name "*arm64*linux*.so" \) \
        -exec cp {} "${JVM_RESOURCES_DIR}/linux-aarch64/libaerolog_uniffi.so" \; 2>/dev/null || true

    find "${ARTIFACTS_DIR}" -type f \( -name "*darwin*aarch64*.dylib" -o -name "*aarch64*darwin*.dylib" -o -name "*arm64*apple*.dylib" -o -name "*aarch64*apple*.dylib" \) \
        -exec cp {} "${JVM_RESOURCES_DIR}/darwin-aarch64/libaerolog_uniffi.dylib" \; 2>/dev/null || true

    find "${ARTIFACTS_DIR}" -type f \( -name "*darwin*x86_64*.dylib" -o -name "*x86_64*darwin*.dylib" -o -name "*x86_64*apple*.dylib" \) \
        -exec cp {} "${JVM_RESOURCES_DIR}/darwin-x86-64/libaerolog_uniffi.dylib" \; 2>/dev/null || true

    find "${ARTIFACTS_DIR}" -type f \( -name "*win*x86_64*.dll" -o -name "*x86_64*windows*.dll" -o -name "*x86_64*pc*windows*.dll" -o -name "*.dll" \) \
        -exec cp {} "${JVM_RESOURCES_DIR}/win32-x86-64/aerolog_uniffi.dll" \; 2>/dev/null || true
else
    echo "No external artifacts directory specified. Compiling host native cdylib..."
    cd "${ROOT_DIR}"
    cargo build --release -p aerolog-uniffi

    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "${OS}" in
        Linux*)
            if [ "${ARCH}" = "x86_64" ]; then
                cp "${ROOT_DIR}/target/release/libaerolog_uniffi.so" "${JVM_RESOURCES_DIR}/linux-x86-64/libaerolog_uniffi.so"
                echo "Installed host binary to: linux-x86-64/libaerolog_uniffi.so"
            elif [ "${ARCH}" = "aarch64" ] || [ "${ARCH}" = "arm64" ]; then
                cp "${ROOT_DIR}/target/release/libaerolog_uniffi.so" "${JVM_RESOURCES_DIR}/linux-aarch64/libaerolog_uniffi.so"
                echo "Installed host binary to: linux-aarch64/libaerolog_uniffi.so"
            fi
            ;;
        Darwin*)
            if [ "${ARCH}" = "arm64" ]; then
                cp "${ROOT_DIR}/target/release/libaerolog_uniffi.dylib" "${JVM_RESOURCES_DIR}/darwin-aarch64/libaerolog_uniffi.dylib"
                echo "Installed host binary to: darwin-aarch64/libaerolog_uniffi.dylib"
            else
                cp "${ROOT_DIR}/target/release/libaerolog_uniffi.dylib" "${JVM_RESOURCES_DIR}/darwin-x86-64/libaerolog_uniffi.dylib"
                echo "Installed host binary to: darwin-x86-64/libaerolog_uniffi.dylib"
            fi
            ;;
        MINGW*|MSYS*|CYGWIN*)
            cp "${ROOT_DIR}/target/release/aerolog_uniffi.dll" "${JVM_RESOURCES_DIR}/win32-x86-64/aerolog_uniffi.dll"
            echo "Installed host binary to: win32-x86-64/aerolog_uniffi.dll"
            ;;
        *)
            echo "Warning: Unrecognized host platform (${OS} ${ARCH})." >&2
            ;;
    esac
fi

echo "Multiplatform JVM resources configured under: ${JVM_RESOURCES_DIR}"

if [ "${BUILD_JAR}" = true ]; then
    echo "Packaging JAR archive to: ${JAR_OUTPUT}"
    mkdir -p "${ROOT_DIR}/target"
    if [ -x "${ROOT_DIR}/gradlew" ]; then
        cd "${ROOT_DIR}"
        ./gradlew :bindings:kotlin-jvm:jar --no-daemon
        find "${ROOT_DIR}/bindings/kotlin-jvm/build/libs" -maxdepth 1 -name "aerolog-jvm-*.jar" ! -name "*-sources.jar" ! -name "*-javadoc.jar" -exec cp {} "${JAR_OUTPUT}" \;
    else
        cd "${JVM_RESOURCES_DIR}"
        jar cf "${JAR_OUTPUT}" .
    fi
    echo "JAR successfully assembled at: ${JAR_OUTPUT}"
fi
