# Embedded Rust Developer Guide

This guide covers building production-grade embedded storage systems using `aerolog-core` in pure Rust.

---

## 1. Engine Configuration

`AeroLogConfig` allows tuning segment thresholds and retention policies:

```rust
use aerolog_core::{AeroLogConfig, AeroLogDb, StreamRetentionPolicy};
use std::time::Duration;

fn initialize_engine() -> aerolog_core::Result<AeroLogDb> {
    let config = AeroLogConfig::new("/var/data/edge_node")
        // Rotate segment files every 64 MB
        .with_max_segment_size(64 * 1024 * 1024)
        // Set sparse checkpoint frequency for streams (1 index entry per 100 records)
        .with_sparse_interval(100)
        // Default retention for streams: keep max 10,000 entries
        .with_default_retention(StreamRetentionPolicy::MaxEntries(10_000));

    AeroLogDb::open(config)
}
```

---

## 2. Point CRUD Operations

AeroLog exposes point Key-Value operations organized by collections:

```rust
// Insert / Update (Overwrites existing key via LWW HLC)
db.put("inventory", "item_401", b"{\"name\":\"Widget\",\"qty\":150}")?;

// Point Read
let item = db.get("inventory", "item_401")?;

// Delete (Appends a Tombstone record with monotonic HLC)
db.delete("inventory", "item_401")?;

// Verify Deletion (Returns Ok(None))
assert!(db.get("inventory", "item_401")?.is_none());
```

---

## 3. Append-Only Streams with Retention

Streams are append-only partitions identified by UTF-8 names:

```rust
use aerolog_core::StreamRetentionPolicy;

// Open stream with custom retention policy
let chat_stream = db.stream("chat:room_1")?;
chat_stream.set_retention_policy(StreamRetentionPolicy::MaxEntries(5_000));

// Append event
let offset = chat_stream.append(b"Hello edge mesh!")?;

// Read recent entries
let entries = chat_stream.read_recent(50)?;
for entry in entries {
    println!("Offset {}: {} bytes", entry.offset, entry.payload.len());
}
```

---

## 4. Delta Replication Scanning

To replicate local mutations across peers without scanning entire tables, use `scan_mutations_since`:

```rust
use aerolog_core::HlcTimestamp;

// Scan mutations newer than last known remote sync timestamp
let last_sync_hlc = Some(HlcTimestamp::new(1700000000000, 4, 0x12345678));
let new_mutations = db.scan_mutations_since(last_sync_hlc, 500)?;

for mutation in new_mutations {
    println!(
        "Collection: {}, Key: {}, Type: {:?}",
        mutation.collection, mutation.key, mutation.record_type
    );
}
```

---

## 5. Offline & Background Compaction

Reclaim disk space by purging overwritten records and tombstones:

```rust
let stats = db.compact()?;
println!("Compacted {} files into 1 segment.", stats.files_merged);
println!("Reclaimed {} bytes of disk space.", stats.bytes_reclaimed);
```
