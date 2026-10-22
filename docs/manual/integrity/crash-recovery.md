# Crash Recovery & Data Integrity

AeroLog is engineered to withstand abrupt power loss, battery disconnects, kernel panics, and unexpected process termination without database corruption.

---

## Defensive Integrity Protections

### 1. Hardware-Accelerated CRC32
Every record written to disk is prepended with a 32-bit CRC32 checksum computed across both the fixed header fields and the payload bytes. On modern processors, checksum computation uses dedicated CPU vector instructions:
* `CRC32` instruction via SSE 4.2 on x86-64
* `__crc32d` instruction via ARMv8-A Cryptography Extensions on aarch64

### 2. Upfront Length Prefixes
All variable-length fields (`key_len`, `collection_len`, `val_len`) are stored in the fixed 32-byte header. Before reading payloads into buffers, AeroLog validates that:

$$
\text{offset} + 32 + \text{key\_len} + \text{coll\_len} + \text{val\_len} \le \text{file\_size}
$$

This prevents buffer over-reads and segmentation faults when encountering malformed or partial writes.

---

## The Cold Startup Recovery Scanner

When `AeroLogDb::open()` executes, the recovery scanner walks all `.aerolog` files sequentially:

```
[Record 0: CRC OK] -> [Record 1: CRC OK] -> [Record 2: CRC OK] -> [Torn Bytes: CRC FAIL]
==================================================================>
      Valid Historical State Recovered                             Fragment Discarded
```

1. **Sequential Traversal**: Starts at offset 0 of each file and reads the 32-byte header.
2. **Checksum Verification**: Re-computes the CRC32 checksum over the stored header and payload bytes.
   * If valid: Adds the coordinate to `KeyDir` or `SparseStreamIndex`.
   * If invalid or incomplete:
     - The corrupted trailing fragment is safely discarded.
     - The file is truncated to the end of the last valid record.
     - Recovery completes successfully with zero loss of prior historical data.
