# AeroLog Agent & Architecture Guidelines

Welcome to **AeroLog**. This file outlines mandatory architectural constraints, coding standards, and design invariants that all AI agents and human contributors must strictly uphold across sessions.

---

## 1. Core Architectural Invariants

### Invariant 1: Append-Only Sequential Log (Bitcask Design)
* In `aerolog-core`, write operations (`put`, `delete`, `append`) must always be purely sequential appends to the active `.aerolog` file.
* Never overwrite or mutate existing bytes in-place on disk.
* Deletions must be represented as immutable tombstone records appended to the log.
* All disk operations must be atomic and crash-resilient, protected by 32-bit CRC32 checksums on every record header.

### Invariant 2: Dual-Tier Memory Partitioning
* **KeyDir Index**: Used strictly for mutable Key-Value entities. KeyDir maps `(collection, key)` to `(file_id, offset, length, hlc)`.
* **Sparse Stream Index**: Used for append-only streams (chat messages, event logs). Stream entries must NEVER flood the KeyDir. Instead, streams are indexed via sparse checkpoints (e.g. 1 pointer per 100/1000 records).
* Total resident set size (RSS) memory footprint must stay strictly below 15 MB even when managing 100,000+ stream records on mobile devices.

### Invariant 3: Deterministic Causality & Monotonic Clocks
* Every record header must carry a monotonically increasing Hybrid Logical Clock (HLC) containing physical time (epoch ms), logical counter (u16), and node identifier hash (u64).
* Conflict resolution follows deterministic Last-Write-Wins (LWW) based on total HLC ordering:
  1. `HlcTimestamp.physical_time`
  2. `HlcTimestamp.logical_counter`
  3. `HlcTimestamp.node_id_hash` (deterministic tie-breaker)

### Invariant 4: Zero-Trust & Defensive Boundary Validation
* Always validate slice bounds, length prefixes, and CRC32 checksums before parsing byte buffers to prevent buffer underflows or memory corruption from malformed records.
* File segment corruption or sudden process crashes must be isolated: valid pre-crash records must remain fully readable, while corrupted trailing fragments are safely rejected during recovery.

### Invariant 5: Universal Zero-Dependency Core
* `aerolog-core` must remain 100% pure Rust with zero platform-specific dependencies (no hard dependency on Android NDK, Apple Cocoa, or OS windowing).
* Platform bindings (Kotlin, Swift) are generated cleanly via Mozilla UniFFI and maintain 100% byte-for-byte binary parity on disk.

---

## 2. Code Quality & Standards

1. **Zero-Emoji Policy**: Strictly zero emojis across all source code, tests, docstrings, commit messages, PR descriptions, and documentation files.
2. **Language**: All code, symbol names, comments, docstrings, and commit messages must be in English.
3. **No Unwraps in Core Logic**: Never use `unwrap()` or `expect()` in production engine code. All errors must map into `Result<T, AeroLogError>`.
4. **License Header**: All new source files must begin with the standard Apache-2.0 copyright header.
5. **Conventional Commits v1.0.0**: Every commit must follow `<type>(<scope>): <summary>` (e.g., `feat(core): implement segment file manager with crc32 verification`).
6. **Zero Warnings Policy**: Code must pass `cargo clippy --all-targets -- -D warnings` with zero warnings.
7. **Test Before Commit**: All tests in `cargo test --all` must pass deterministically in under 5 seconds.

---

## 3. Engineering Roadmap

- **Track 1 (Core Storage & Bitcask Engine)**: Segment file manager, LogWriter, LogReader, CRC32, KeyDir, SparseStreamIndex, and crash recovery scanner.
- **Track 2 (Compactor & Pluggable Retention)**: Background merge worker, tombstone purging, and declarative retention policies (`KeepAll`, `MaxEntries`, `TimeToLive`).
- **Track 3 (Zero-Dependency CBOR Codec)**: Pure Rust compact binary codec (`aerolog-cbor`) for efficient structured payload encoding.
- **Track 4 (CLI Tool & Benchmark Suite)**: Standalone `aerolog-cli` binary with throughput benchmarking (`bench`), inspection (`inspect`), and diagnostics (`dump`).
- **Track 5 (UniFFI Cross-Platform Bindings)**: Native Kotlin (Android/JVM) and Swift (iOS/macOS) bindings with automated tests.
- **Track 6 (AirDB Clean Adapter)**: Drop-in `AeroLogAirDbAdapter` implementing AirDB's `AirDbAdapter` persistence contract.
