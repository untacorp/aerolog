# Frequently Asked Questions (FAQ)

### What happens if the device loses power during a write?
AeroLog writes every record with an upfront 32-byte header containing a hardware CRC32 checksum. If power is interrupted mid-append:
1. The pre-crash records preceding the interrupted write remain fully valid and intact.
2. The trailing partial bytes fail the CRC32 checksum during startup scan.
3. The crash recovery scanner automatically discards the corrupted fragment and truncates the active file segment back to the last known valid record boundary.

---

### What is the difference between a Collection and a Stream?
* **Collection**: Designed for mutable Key-Value entities. Keys in a collection have Last-Write-Wins semantics. Only the latest active value for each key is retained in the `KeyDir` index. Lookups are $O(1)$.
* **Stream**: Designed for ordered, append-only event logs (e.g., chat messages, sensor logs, audit events). Entries are appended sequentially and are assigned monotonic offsets. Streams use the bounded `SparseStreamIndex` so that millions of messages do not exhaust RAM.

---

### How does AeroLog keep RAM usage below 15 MB for large streams?
For point Key-Value collections, every unique key resides in `KeyDir`. For streams, however, AeroLog applies **sparse indexing**: it only stores an index checkpoint every $N$ entries (e.g., entry 0, 100, 200, 300). When seeking entry 254, AeroLog jumps to the checkpoint at 200 and scans forward 54 records sequentially. This bounded index requires only a few kilobytes of RAM per stream partition.

---

### Is AeroLog thread-safe?
Yes. The `AeroLogDb` facade is thread-safe and can be safely shared across threads (`Arc<AeroLogDb>`). Internal write locks protect log segment appends while read operations execute concurrently.

---

### Can AeroLog segment files be copied across platforms?
Yes. AeroLog uses Little-Endian encoding with explicit byte widths (u16, u32, u64) for all record headers and CBOR payloads. A database directory generated on an Android device (ARM64) can be copied and opened directly on a Linux server (x86-64) or macOS MacBook (Apple Silicon) without data migration.

---

### How do I inspect what is stored inside `.aerolog` files?
Use the built-in CLI diagnostic tool:
```bash
cargo run -p aerolog-cli -- inspect /path/to/data-000000.aerolog
```
This prints the header fields, CRC32 status, physical timestamps, record types, keys, and payload sizes for every record in the segment.
