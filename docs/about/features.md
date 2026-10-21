# Core Features

AeroLog delivers an uncompromising set of storage capabilities engineered for high performance, crash safety, and cross-platform ubiquity.

---

## 1. Bitcask Append-Only Storage Engine

* **Pure Sequential Appends ($O(1)$)**: Disk writes are appended sequentially to an active `.aerolog` segment file. No seeking, no block rewriting, and no lock contention on disk pages.
* **Segment Rolling**: Active segment files roll over when reaching configurable size thresholds (default: 64 MB), producing sealed immutable historical segments.
* **Single-Seek Random Reads ($O(1)$)**: Point lookups consult the in-memory index and jump directly to the exact file offset and byte length, reading the value in a single I/O operation.

---

## 2. Dual-Tier Memory Partitioning

* **KeyDir Index**: High-speed in-memory hash table mapping `(collection, key)` pairs to `(file_id, offset, length, hlc)`.
* **Sparse Stream Index**: Specialized index for append-only streams (chat messages, telemetry events) that records checkpoints at sparse intervals (e.g., 1 pointer per 100 entries), keeping resident memory footprint strictly bounded under 15 MB.

---

## 3. Monotonic Hybrid Logical Clocks (HLC)

* **Physical + Logical Ordering**: Formulated around Kulkarni et al.'s HLC algorithm, combining local physical time (epoch milliseconds), an incremental counter for sub-millisecond events, and a deterministic node identifier hash.
* **Distributed Last-Write-Wins (LWW)**: Provides total causal ordering across nodes without requiring centralized NTP coordination or server consensus.
* **Forward Ratcheting**: When receiving records from remote peers during synchronization, the local clock automatically ratchets forward past the highest observed remote timestamp.

---

## 4. Hardware-Accelerated Data Integrity & Recovery

* **CRC32 Checksum Validation**: Every 32-byte record header and variable payload is verified using hardware-accelerated CRC32-IEEE instructions (`SSE4.2` on x86-64, `ARMv8 CRC32` on aarch64).
* **Defensive Boundary Validation**: Slice lengths, string bounds, and buffer boundaries are validated before deserialization to prevent memory corruption.
* **Torn-Write Isolation**: Truncated or corrupted trailing record fragments from abrupt system crashes or power failures are automatically identified, logged, and isolated during startup without compromising valid data.

---

## 5. Pluggable Compaction & Retention

* **Background Log Compactor**: Merges sealed historical segments, evicting dead records, overwritten keys, and soft-deleted tombstones into a new compacted segment.
* **Declarative Stream Retention**:
  * `KeepAll`: Retains complete historical stream data.
  * `MaxEntries(u64)`: Caps stream partition size, automatically pruning oldest entries during compaction.
  * `TimeToLive(u64)`: Prunes stream entries older than a specified duration in seconds.

---

## 6. Zero-Dependency RFC 8949 CBOR Codec

* **Pure Rust Binary Serializer**: Standalone `aerolog-cbor` crate provides binary serialization for structured payloads (maps, arrays, integers, text strings, byte buffers, floating-point numbers).
* **Zero External C/C++ Dependencies**: Immune to native memory safety vulnerabilities and buffer overflow exploits.

---

## 7. Universal Cross-Platform Bindings (Mozilla UniFFI)

* **Multi-Platform Native Support**: Full idiomatic bindings for Kotlin (Android and JVM) and Swift (iOS and macOS).
* **Automatic Resource Disposal**: Implements Kotlin `AutoCloseable` and Swift ARC / `defer` patterns for leak-free memory safety across FFI boundaries.
* **Binary Disk Parity**: Storage files created on Android are 100% byte-for-byte compatible with iOS, macOS, Linux, and Windows.
