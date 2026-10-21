# Binary File Format Specification

This document specifies the exact byte-level physical layout of `.aerolog` segment files. All numeric values are encoded in **Little-Endian** byte order.

---

## 1. Segment Overview

An `.aerolog` segment file consists of a continuous stream of variable-length binary records. There is no file-level header; every record begins with a fixed 32-byte header.

```
+-------------------------------------------------------------------------------+
|                       32-BYTE FIXED RECORD HEADER                             |
+-------------------------------------------------------------------------------+
|                         VARIABLE-LENGTH RECORD BODY                           |
|       [Collection Bytes] + [Key Bytes] + [Payload / Value Bytes]              |
+-------------------------------------------------------------------------------+
|                       NEXT RECORD (Header + Body)                             |
|                                  ...                                          |
+-------------------------------------------------------------------------------+
```

---

## 2. Fixed Header Byte Layout (32 Bytes)

| Byte Offset | Field Name | Data Type | Description |
| :--- | :--- | :--- | :--- |
| `0x00 - 0x03` | `crc32` | `u32` | Hardware CRC32-IEEE checksum of bytes `0x04..0x20` + Body |
| `0x04 - 0x0B` | `physical_time` | `u64` | Wall-clock epoch millisecond timestamp |
| `0x0C - 0x0D` | `logical_counter`| `u16` | Monotonic sub-millisecond logical counter |
| `0x0E - 0x15` | `node_id_hash` | `u64` | FNV-1a 64-bit hash of originating node identifier |
| `0x16` | `record_type` | `u8` | `1` = Insert/Update, `2` = Tombstone, `3` = StreamAppend |
| `0x17 - 0x18` | `key_len` | `u16` | Length of the entity Key string in bytes |
| `0x19 - 0x1A` | `collection_len`| `u16` | Length of the Collection string in bytes |
| `0x1B - 0x1E` | `val_len` | `u32` | Length of the Payload/Value in bytes |
| `0x1F - 0x1F` | `reserved` | `u8[1]` | Alignment padding byte (must be `0x00`) |

---

## 3. Record Body Layout

Immediately following byte `0x1F` of the header:

| Field Name | Length in Bytes | Encoding |
| :--- | :--- | :--- |
| `collection` | `collection_len` | UTF-8 String (e.g. `"users"`, `"messages"`) |
| `key` | `key_len` | UTF-8 String (e.g. `"usr_01"`, `"offset_100"`) |
| `value` | `val_len` | Raw Binary Blob (CBOR, JSON, encrypted bytes) |

Total record length on disk:
$$\text{Total Bytes} = 32 + \text{key\_len} + \text{collection\_len} + \text{val\_len}$$

---

## 4. Checksum Calculation Scope

The `crc32` field at bytes `0x00..0x03` protects all subsequent bytes of the record:
$$\text{CRC32}(\text{bytes}[0x04 \dots 0x1F] \,\|\, \text{body\_bytes})$$
During read operations, the engine calculates the checksum over this slice and asserts equality with `crc32`.
