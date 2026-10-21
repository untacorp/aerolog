# Bitcask Append-Only Storage

AeroLog structures physical data on disk as a sequence of immutable and active log files known as **segments**.

---

## The Log-Structured Model

In traditional in-place update storage systems (like B-Trees), overwriting an existing record modifies bytes in the middle of a file. In contrast, AeroLog enforces an **append-only rule**: every mutation (insert, update, delete) is appended to the tail of the current active segment file.

```
data-000000.aerolog (Sealed)   | data-000001.aerolog (Active)
+--------+--------+--------+   | +--------+--------+----------------->
| Rec 0  | Rec 1  | Rec 2  |   | | Rec 3  | Rec 4  | [Next Append]
+--------+--------+--------+   | +--------+--------+----------------->
```

### Advantages for Flash & Battery
1. **Zero Random Disk Writes**: Operating systems and flash controllers stream contiguous blocks sequentially, eliminating internal flash block re-allocation overhead.
2. **Crash Atomicity**: Appending bytes to the end of a file guarantees that prior historical records are never partially overwritten if a crash occurs mid-write.
3. **No Lock Contention Between Readers and Writers**: Readers seeking historical records in sealed segments never contend with the active segment writer.

---

## Segment Lifecycle

* **Active Segment**: Exactly one segment file is open for both reading and appending (e.g., `data-000001.aerolog`).
* **Rolling Over**: When the active segment file size reaches `max_segment_size` (configured via `AeroLogConfig`, defaulting to 64 MB), the writer:
  1. Flushes remaining OS buffers to disk.
  2. Seals the segment file, making it strictly read-only.
  3. Increments the file ID sequence (`data-000002.aerolog`) and opens the new active segment.
* **Sealed Segments**: Immutable files that can be read concurrently by multiple threads without synchronization locks. Sealed segments are eligible for background compaction.
