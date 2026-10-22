# AeroLog v0.1.0: Universal, Local-First Multi-Model Storage Engine

AeroLog v0.1.0 is the inaugural production release of AeroLog: an embedded, local-first multi-model storage engine engineered from first principles in pure **Rust**. Optimized for NAND flash memory, high-frequency synchronization, and resource-constrained edge computing environments.

---

## What is New in v0.1.0

### 1. Pure Rust Append-Only Bitcask Core (`aerolog-core`)
- **Sequential Write I/O ($O(1)$ writes)**: Direct immutable append-only segment storage (`.aerolog`) eliminating random write-amplification on flash storage and reducing mobile battery drain.
- **Hardware-Accelerated CRC32**: Every 32-byte binary record header includes hardware-computed CRC32 integrity checksums over header and payload buffers.
- **Automated Crash Recovery**: Startup scanner detects torn writes, incomplete buffers, and power-failure corruption, safely truncating partial fragments back to the last valid boundary.
- **Concurrent Thread Safety**: Lock-free reads with fine-grained read-write locks (`parking_lot::RwLock`) supporting multi-threaded workloads without database-level blocking.

### 2. Dual-Tier Partitioned Memory Architecture
- **In-Memory KeyDir**: Fast hash map index for Key-Value collections mapping keys directly to segment file IDs and 64-bit byte offsets for single-seek disk reads.
- **Sparse Stream Index**: Memory-bounded stream index using periodic sparse checkpoints (e.g. 1 pointer per 100 entries), keeping RAM usage strictly bounded under **15 MB** across hundreds of thousands of events.
- **Pluggable Retention Policies**: Declarative stream retention rules (`KeepAll`, `MaxEntries(u64)`, `TimeToLive(u64)`) evaluated during background compaction.

### 3. Distributed Causality & Deterministic Conflict Resolution
- **Monotonic Hybrid Logical Clock (HLC)**: Every record header stores physical epoch milliseconds, a logical sequence counter, and a unique node tie-breaker identifier.
- **Peer-to-Peer Causality**: Guarantees total monotonic ordering and deterministic Last-Write-Wins (LWW) conflict resolution across offline partitions without NTP server synchronization.

### 4. Zero-Dependency CBOR Codec (`aerolog-cbor`)
- High-efficiency binary serialization complying with **RFC 8949 (CBOR)**.
- Pure Rust, zero external dependencies, and zero unsafe code.

### 5. Cross-Platform FFI Bindings (`aerolog-uniffi`)
- Universal type-safe C-ABI bridge generated via Mozilla UniFFI v0.32.
- Ready-to-use foreign language bindings for **Kotlin** (Android & JVM) and **Swift** (iOS & macOS).

### 6. AeroLog CLI Diagnostic Suite (`aerolog-cli`)
- Command-line utility for database operations: `bench`, `inspect`, `dump`, `compact`.
- Demonstrated native throughput: **>200,000 sequential writes/sec** and **>300,000 stream appends/sec**.

---

## Installation & Coordinates

### Rust (Cargo)
Add to your `Cargo.toml`:
```toml
[dependencies]
aerolog-core = "0.1.0"
aerolog-cbor = "0.1.0"
```

### Pre-compiled Release Binaries
Pre-compiled executable archives for `aerolog` CLI are attached to this release for:
- `aerolog-x86_64-unknown-linux-gnu.tar.gz` (Linux x86_64)
- `aerolog-aarch64-apple-darwin.tar.gz` (macOS Apple Silicon M-series)
- `aerolog-x86_64-apple-darwin.tar.gz` (macOS Intel)
- `aerolog-x86_64-pc-windows-msvc.zip` (Windows x64)

---

## Quickstart

### Key-Value Collection Mutation & Point Read
```rust
use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};

let config = AeroLogConfig::new(0x1001)
    .with_sync_mode(SyncMode::Periodic)
    .with_max_segment_size(16 * 1024 * 1024);

let db = AeroLogDb::open("/path/to/db_dir", config)?;

// Put key-value
db.put("users", "user_1001", b"{\"name\": \"Alice\"}".to_vec())?;

// Single-seek get
if let Some(record) = db.get("users", "user_1001")? {
    println!("Retrieved: {}", String::from_utf8_lossy(&record.payload));
}
```

### Append-Only Event Stream
```rust
use aerolog_core::RetentionPolicy;

let stream = db.get_stream_with_retention("sensor_telemetry", RetentionPolicy::MaxEntries(50000))?;

// Append event
stream.append(b"TEMPERATURE:23.4C,HUMIDITY:55%".to_vec())?;

// Efficient retrieval of the 50 most recent records
let recent = stream.read_recent(50)?;
```

---

## Integrity & Verification

All release packages include a `SHA256SUMS.txt` manifest. Verify artifact integrity:
```bash
sha256sum -c SHA256SUMS.txt
```
