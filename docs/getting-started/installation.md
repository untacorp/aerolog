# Installation & Setup

AeroLog provides first-class support for Rust, Kotlin (Android/JVM), Swift (iOS/macOS), and command-line usage.

---

## 1. Rust Projects

Add `aerolog-core` to your `Cargo.toml`:

```toml
[dependencies]
aerolog-core = { git = "https://github.com/untacorp/aerolog.git", branch = "main" }
# Optional pure-Rust RFC 8949 binary CBOR codec
aerolog-cbor = { git = "https://github.com/untacorp/aerolog.git", branch = "main" }
```

Or add local path references if working inside a workspace:

```toml
[dependencies]
aerolog-core = { path = "path/to/aerolog/crates/aerolog-core" }
```

---

## 2. Kotlin & Android (Gradle)

### Dependency Declaration
In your module's `build.gradle.kts`:

```kotlin
dependencies {
    // Java Native Access (JNA) required by Mozilla UniFFI
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    
    // AeroLog Kotlin bindings
    implementation(files("libs/aerolog-uniffi.jar"))
}
```

### Native Binary Libraries (`.so`)
Place the target architecture binaries in `src/main/jniLibs/` or `src/main/resources/`:
* `arm64-v8a/libaerolog_uniffi.so` (Modern Android devices)
* `armeabi-v7a/libaerolog_uniffi.so` (Legacy 32-bit devices)
* `x86_64/libaerolog_uniffi.so` (Android Studio Emulator & Linux servers)

---

## 3. Apple Platforms (Swift Package Manager)

Add AeroLog to your `Package.swift`:

```swift
// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "MyApp",
    dependencies: [
        .package(url: "https://github.com/untacorp/aerolog.git", branch: "main")
    ],
    targets: [
        .target(
            name: "MyApp",
            dependencies: [
                .product(name: "AeroLogUniFFI", package: "aerolog")
            ]
        )
    ]
)
```

---

## 4. CLI Tool Installation

Build and install the `aerolog` CLI diagnostic binary using Cargo:

```bash
cargo install --path crates/aerolog-cli
```

Verify installation:
```bash
aerolog --help
```
