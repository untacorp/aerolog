# Dual-Tier In-Memory Indexing

A fundamental challenge of Bitcask storage engines is in-memory indexing. If every entry in the database requires an in-memory hash pointer, append-only streams (like chat histories or sensor data) will quickly exhaust mobile RAM.

AeroLog solves this through **Dual-Tier Memory Partitioning**.

---

## Tier 1: In-Memory `KeyDir` (For Key-Value Entities)

The `KeyDir` is an in-memory hash map that indexes point mutable records:

```rust
// Conceptually:
HashMap<(String, String), DiskPointer>
// Where DiskPointer is:
struct DiskPointer {
    file_id: u32,
    offset: u64,
    length: u32,
    hlc: HlcTimestamp,
}
```

### Point Read Execution ($O(1)$)
1. Hash lookup `KeyDir.get((collection, key))` retrieves the `DiskPointer`.
2. The engine issues a single seek to `file_id` at `offset`.
3. It reads `length` bytes directly from the segment file into memory.
4. CRC32 is validated, and the payload is returned to the user.

### Point Write Execution ($O(1)$)
1. The record is appended to the active segment file, returning `(file_id, offset)`.
2. `KeyDir.insert()` updates the hash table. If the key already existed, the pointer is replaced with the new location and HLC timestamp.
3. The old record on disk becomes garbage and will be reclaimed during compaction.

---

## Tier 2: `SparseStreamIndex` (For Append-Only Streams)

Stream partitions (chat logs, audit trails, telemetry) append thousands of events over time. If indexed in `KeyDir`, they would exhaust RAM.

Instead, AeroLog partitions streams into the `SparseStreamIndex`:

```
Stream: "sensor_telemetry"
Logical Offset:  0      100     200     300     400
Checkpoints:    [Ptr0]  [Ptr1]  [Ptr2]  [Ptr3]  [Ptr4]  ... (Sparse in RAM)
Disk:           ======================================> (Dense on Disk)
```

### Mechanics of Sparse Seeking
* **Checkpoint Frequency**: An index checkpoint is recorded only once every $N$ entries (default: 100 entries, configurable via `sparse_interval`).
* **RAM Footprint**: A stream with 100,000 entries requires only 1,000 pointer checkpoints in RAM (~32 KB total memory).
* **Bounded RSS Proof**: Even across dozens of active stream partitions and hundreds of thousands of events, total resident set size remains strictly below 15 MB on mobile platforms.
* **Reading an Arbitrary Offset**:
  To read offset `245`:
  1. Jump directly to the sparse checkpoint at offset `200`.
  2. Scan forward sequentially 45 records in the segment file.
  3. Return the target entry.
