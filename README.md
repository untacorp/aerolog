<div align="center">

# AeroLog

**Universal, Local-First Multi-Model Storage Engine for Edge & Off-Grid Systems**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Architecture](https://img.shields.io/badge/Architecture-Bitcask%20Log-success.svg)](#architecture)
[![Zero-GC](https://img.shields.io/badge/Runtime-Zero--GC-brightgreen.svg)](#key-capabilities)

*Sub-millisecond writes. Predictable memory footprint. Cryptographic provenance. Zero Garbage Collection.*

</div>

---

## Overview

**AeroLog** is an open-source, embedded, local-first storage engine engineered from first principles in pure **Rust**. While traditional mobile databases (like SQLite) rely on page-based B-Trees that suffer from heavy write-amplification on flash memory, AeroLog implements an **Append-Only Bitcask Log** optimized for sequential I/O, extreme crash-resilience, and high-frequency wireless mesh synchronization.

AeroLog natively supports both:
1. **Key-Value Collections**: Instant $\mathcal{O}(1)$ point reads and writes for mutable state entities.
2. **Append-Only Streams**: Sequential event logs and persistent messaging partitions (chat rooms, sensor streams) with an ultra-lightweight memory footprint.

---

## Key Capabilities

- **Pure Sequential I/O ($\mathcal{O}(1)$ Writes)**: Mutations are appended to active `.aerolog` segment files without in-place page rewriting. Flash memory wear is minimized and battery drain is reduced.
- **Dual-Tier Memory Partitioning**:
  - **In-Memory KeyDir**: Hash table mapping keys to disk coordinates for fast single-seek random reads.
  - **Sparse Stream Index**: Indexes stream partitions using sparse checkpoints (e.g. 1 pointer per 100 entries), keeping RAM usage strictly under **15 MB** even across 100,000+ messages.
- **Monotonic Hybrid Logical Clocks (HLC)**: Every record header stores physical epoch time, a logical counter, and a node tie-breaker, providing deterministic Last-Write-Wins (LWW) conflict resolution without NTP synchronization.
- **Crash-Resilience & Hardware CRC32**: Every 32-byte record header is protected by hardware-accelerated CRC32. Unwritten or truncated fragments from abrupt power cuts are automatically isolated and discarded during startup recovery.
- **Zero-Dependency Core**: `aerolog-core` contains zero platform-specific dependencies and zero C/C++ FFI leaks. Runs on Linux, macOS, Windows, Android, and iOS.
- **Pluggable Retention & Compaction**: Merges sealed segment files in the background, evicting dead records and soft-deleted tombstones without blocking active reads or writes.

---

## Architecture

```
+-----------------------------------------------------------------------------+
|                         Application / P2P Mesh Layer                        |
|       (Distributed Replication, Messaging Streams, Edge POS, Sensor Logs)   |
+--------------------------------------+--------------------------------------+
                                       |
+--------------------------------------v--------------------------------------+
|                     AeroLogDb (Multi-Model Storage Facade)                  |
|  - put(collection, key, val)                 - get_stream(stream_id)        |
|  - get(collection, key)                      - stream.append(payload)       |
|  - delete(collection, key)                   - stream.read_recent(limit)    |
+--------------------------------------+--------------------------------------+
                                       |
          +----------------------------+----------------------------+
          |                                                         |
+---------v---------------------------+ +---------------------------v---------+
|     In-Memory Index Subsystem       | |           Causality Core            |
| - KeyDir (RwLock<HashMap>)          | | - Monotonic Hybrid Logical Clock    |
|   (O(1) Point Seeks for KV)         | |   (HlcTimestamp: pt, lc, node_hash) |
| - SparseStreamIndex                 | | - Deterministic LWW Conflict Guard  |
|   (Sparse Checkpoints for Streams)  | +-------------------------------------+
+-------------------------------------+
          |
+---------v-------------------------------------------------------------------+
|                        Log I/O & Segment Subsystem                          |
| - LogWriter: Sequential buffer appends with per-record hardware CRC32       |
| - LogReader: Single-seek random reads and sequential stream cursors         |
| - SegmentManager: Rolling active data-XXXXXX.aerolog segments (default 16MB)|
| - RecoveryManager: Zero-copy startup scanning and corruption isolation      |
| - LogCompactor: Background segment merge and tombstone garbage collector    |
+-----------------------------------------------------------------------------+
```

---

## On-Disk Binary Format (`.aerolog`)

Each record is encoded as a fixed **32-byte header** followed by the key and value payload:

```text
Offset (Byte)  Size (B)  Type        Field Description
---------------------------------------------------------------------------------
0x00           4         UInt32 BE   CRC32 Checksum (covers header fields 04..31 + payload)
0x04           2         ASCII       Magic Bytes: 0x41 0x4C ("AL")
0x06           1         UInt8       Format Version (0x01)
0x07           1         UInt8       Record Type (0x00=Put, 0x01=Delete, 0x02=StreamEntry)
0x08           8         Int64 BE    HLC Physical Time (Epoch milliseconds)
0x10           2         UInt16 BE   HLC Logical Counter
0x12           8         Int64 BE    Node ID Hash (64-bit originating node fingerprint)
0x1A           2         UInt16 BE   Key Length (K)
0x1C           4         UInt32 BE   Value Length (V)
0x20           K         UTF-8       Key Bytes
0x20 + K       V         Binary      Value / Message Payload
```

---

## Quick Start

### Add to `Cargo.toml`
```toml
[dependencies]
aerolog-core = { path = "../aerolog/crates/aerolog-core" }
```

### Basic Key-Value Operations
```rust
use aerolog_core::{AeroLogConfig, AeroLogDb, SyncMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AeroLogConfig::new(0x01)
        .with_sync_mode(SyncMode::Periodic)
        .with_max_segment_size(16 * 1024 * 1024);

    let db = AeroLogDb::open("./aerolog_data", config)?;

    // 1. Put
    let hlc = db.put("users", "alice", b"{\"name\": \"Alice\"}".to_vec())?;
    println!("Saved record at HLC: {}", hlc);

    // 2. Get
    if let Some(val) = db.get("users", "alice")? {
        println!("Retrieved value: {}", String::from_utf8_lossy(&val));
    }

    // 3. Delete (appends an immutable tombstone)
    db.delete("users", "alice")?;
    assert_eq!(db.get("users", "alice")?, None);

    Ok(())
}
```

### Append-Only Messaging Streams (Persistent Chat)
```rust
use aerolog_core::{AeroLogConfig, AeroLogDb};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = AeroLogDb::open("./chat_data", AeroLogConfig::default())?;

    // Open or create a stream partition
    let chat = db.get_stream("emergency-channel")?;

    // Append messages sequentially
    chat.append(b"Team Alpha arriving at coordinates.".to_vec())?;
    chat.append(b"Medical kit requested at Sector 2.".to_vec())?;

    // Read the 10 most recent messages without loading the entire history into memory
    let recent = chat.read_recent(10)?;
    for entry in recent {
        println!("[Seq {} | HLC {}]: {}", entry.sequence, entry.hlc, String::from_utf8_lossy(&entry.payload));
    }

    Ok(())
}
```

---

## Benchmarks & Performance

AeroLog includes [Criterion.rs](https://bheisler.github.io/criterion.rs/book/) benchmarks and YCSB workloads:

```bash
# Run all Criterion benchmarks
cargo bench -p aerolog-core

# Run standard YCSB workloads (Workload A, B, C, F)
cargo bench -p aerolog-core --bench ycsb_bench

# Run native release performance test
cargo test --release --test perf_tests -- --nocapture
```

### Verified Release Throughput (x86-64 Linux & ARM64 Apple Silicon)
* **Sequential Write Throughput**: **264,677 writes/sec** (avg latency: **3.78 µs/write**)
* **Concurrent Ingestion (4 Threads)**: **219,854 writes/sec** (avg latency: **4.54 µs/write**)
* **Random Point Reads**: **73,749 reads/sec** (avg latency: **13.56 µs/read**)
* **Stream Append Ingestion**: **520,820 msgs/sec** (avg latency: **1.92 µs/append**)
* **YCSB Workload A (50% Read, 50% Update)**: **128,520 ops/sec** (mean latency: **7.78 µs**)
* **YCSB Workload B (95% Read, 5% Update)**: **85,803 ops/sec** (mean latency: **11.65 µs**)
* **Cold Crash Recovery**: **1,187,695 records/sec** (70,000 records re-indexed in **58.94 ms**)
* **Memory Footprint**: Strictly bounded `< 13 MB RSS` across 100,000 continuous stream events

Detailed methodologies and charts are available in the [Benchmarks Reference](https://untacorp.github.io/aerolog/reference/benchmarks/).

---

## Command-Line Interface (`aerolog`)

AeroLog includes a standalone CLI and interactive shell:

```bash
# Launch interactive REPL
cargo run --release -p aerolog-cli -- --db ./my_data

# Or run commands directly
aerolog --db ./my_data put users alice '{"role":"admin"}'
aerolog --db ./my_data get users alice
aerolog --db ./my_data stream append chat "Node joined"
aerolog --db ./my_data compact
```

See the [CLI Reference](https://untacorp.github.io/aerolog/reference/cli-reference/) for complete command documentation.

---

## License

AeroLog is licensed under the **Apache License, Version 2.0**. See [LICENSE](LICENSE) for details.
