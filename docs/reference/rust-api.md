# Rust API Reference

This reference documents the primary types and methods exposed by the `aerolog-core` crate.

---

## 1. `AeroLogDb`

The main database facade for point operations and stream management.

### Methods

#### `open(config: AeroLogConfig) -> Result<AeroLogDb>`
Opens or creates an AeroLog database at the configured directory. Reconstructs in-memory indexes via the crash recovery scanner.

#### `put(&self, collection: &str, key: &str, value: &[u8]) -> Result<()>`
Appends an Insert mutation to the active segment and updates the `KeyDir`.

#### `get(&self, collection: &str, key: &str) -> Result<Option<Vec<u8>>>`
Performs a single-seek point read by looking up coordinates in the `KeyDir`.

#### `delete(&self, collection: &str, key: &str) -> Result<()>`
Appends a Tombstone record to the active segment and evicts the key from the `KeyDir`.

#### `stream(&self, name: &str) -> Result<Stream>`
Retrieves or creates a named append-only stream partition.

#### `scan_collection(&self, collection: &str) -> Result<Vec<(String, Vec<u8>)>>`
Scans all active point key-value pairs in a collection.

#### `scan_mutations_since(&self, since_hlc: Option<HlcTimestamp>, limit: usize) -> Result<Vec<Record>>`
Scans local mutations strictly newer than the provided HLC timestamp (used for peer delta synchronization).

#### `compact(&self) -> Result<CompactionStats>`
Executes an offline compaction merge across sealed segment files.

---

## 2. `Stream`

Handle to an append-only sequential stream partition.

### Methods

#### `append(&self, payload: &[u8]) -> Result<u64>`
Appends a payload to the stream partition, returning the assigned monotonic offset.

#### `read_recent(&self, limit: usize) -> Result<Vec<StreamEntry>>`
Reads the most recent $N$ entries from the stream using the sparse checkpoint index.

#### `set_retention_policy(&self, policy: StreamRetentionPolicy)`
Sets declarative retention behavior for this stream partition.

---

## 3. `AeroLogConfig`

Builder struct for configuring database parameters:

* `new(path: impl Into<PathBuf>) -> Self`
* `with_max_segment_size(mut self, bytes: u64) -> Self`
* `with_sparse_interval(mut self, interval: usize) -> Self`
* `with_default_retention(mut self, policy: StreamRetentionPolicy) -> Self`
