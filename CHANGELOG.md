# Changelog

All notable changes to AeroLog will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Bitcask-inspired append-only segment file engine (`aerolog-core`) with `.aerolog` files.
- Monotonic Hybrid Logical Clock (`HlcTimestamp`) for deterministic causality and total ordering.
- 32-byte fixed binary record header with per-record hardware CRC32 validation.
- In-memory `KeyDir` index with high-concurrency read/write operations.
- Memory-bounded `SparseStreamIndex` for append-only streaming workloads (messaging, chat, event logs).
- Pluggable retention policies (`KeepAll`, `MaxEntries`, `TimeToLive`).
- Background merge compactor for segment consolidation and tombstone eviction.
- Zero-dependency compact CBOR codec (`aerolog-cbor`).
- Standalone CLI benchmark and diagnostics utility (`aerolog-cli`).
- Mozilla UniFFI bindings generator for Kotlin (Android/JVM) and Swift (Apple platforms).
