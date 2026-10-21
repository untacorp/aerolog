# Engine Manual & Internals

This section details the inner architecture of AeroLog, explaining how bitcask segments, in-memory indexes, hybrid logical clocks, compactor workers, and cross-platform foreign function bridges cooperate.

---

## Technical Chapters

1. [Bitcask Append-Only Storage](storage/bitcask-log.md): Mechanics of sequential log segments, file rotation, and active vs sealed states.
2. [Dual-Tier In-Memory Indexing](storage/dual-tier-indexing.md): Mathematical structure of `KeyDir` point lookups and bounded `SparseStreamIndex`.
3. [Hybrid Logical Clocks & Causality](causality/hlc-clock.md): Physical-logical ordering, monotonic ratcheting, and deterministic LWW convergence.
4. [Compaction & Retention Policies](storage/compaction-and-retention.md): Background merge workers, tombstone eviction, and declarative stream pruning.
5. [Crash Recovery & Data Integrity](integrity/crash-recovery.md): Hardware CRC32-IEEE checksum validation and torn-write isolation.
6. [Zero-Dependency CBOR Codec](formats/cbor-codec.md): Pure Rust RFC 8949 binary data serialization.
7. [UniFFI Cross-Platform Bridge](bindings/uniffi-bridge.md): Mozilla UniFFI v0.32 architecture, memory ownership, and foreign lifecycle.
