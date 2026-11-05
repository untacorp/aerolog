// swift-tools-version: 5.9
//
// Copyright 2026 AeroLog Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import PackageDescription

let package = Package(
    name: "AeroLog",
    platforms: [
        .iOS(.v13),
        .macOS(.v10_15),
        .tvOS(.v13),
        .watchOS(.v6),
        .visionOS(.v1)
    ],
    products: [
        .library(
            name: "AeroLog",
            targets: ["AeroLog"]
        ),
    ],
    targets: [
        .target(
            name: "AeroLogUniFFIFFI",
            path: "bindings/swift",
            sources: [],
            publicHeadersPath: "."
        ),
        .target(
            name: "AeroLogUniFFI",
            dependencies: ["AeroLogUniFFIFFI"],
            path: "bindings/swift",
            sources: ["AeroLogUniFFI.swift"]
        ),
        .target(
            name: "AeroLog",
            dependencies: ["AeroLogUniFFI"],
            path: "bindings/swift",
            sources: ["AeroLog.swift"]
        ),
    ]
)
