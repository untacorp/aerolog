# Binary File Format Specification

This document specifies the exact byte-level physical layout of `.aerolog` segment files. All multi-byte numeric fields are encoded in **Big-Endian** (Network Byte Order).

---

## 1. Segment Overview

An `.aerolog` segment file consists of a continuous stream of variable-length binary records. There is no file-level header; every record begins with a fixed 32-byte header.

```
+-------------------------------------------------------------------------------+
|                       32-BYTE FIXED RECORD HEADER                             |
+-------------------------------------------------------------------------------+
|                         VARIABLE-LENGTH RECORD BODY                           |
|                    [Key Bytes] + [Payload / Value Bytes]                      |
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
| `0x04 - 0x05` | `magic` | `u16` | Magic signature bytes `0x414C` (ASCII "AL") |
| `0x06` | `version` | `u8` | Record layout version (currently `0x01`) |
| `0x07` | `record_type` | `u8` | `0x00` = Put, `0x01` = Delete, `0x02` = StreamEntry |
| `0x08 - 0x0F` | `physical_time` | `u64` | Wall-clock epoch millisecond timestamp |
| `0x10 - 0x11` | `logical_counter`| `u16` | Monotonic sub-millisecond logical counter |
| `0x12 - 0x19` | `node_id_hash` | `u64` | FNV-1a 64-bit hash of originating node identifier |
| `0x1A - 0x1B` | `key_len` | `u16` | Length of the composite key string in bytes |
| `0x1C - 0x1F` | `value_len` | `u32` | Length of the Payload/Value in bytes |

---

## 3. Record Body Layout

Immediately following byte `0x1F` of the header:

| Field Name | Length in Bytes | Description |
| :--- | :--- | :--- |
| `key` | `key_len` | UTF-8 String (`collection:key` for KV; `stream_id` for streams) |
| `value` | `value_len` | Raw Binary Payload (CBOR, JSON, binary blob) |

Total record length on disk:
$$\text{Total Bytes} = 32 + \text{key\_len} + \text{value\_len}$$

---

## 4. Checksum Calculation Scope

The `crc32` field at bytes `0x00..0x03` protects all subsequent bytes of the record:
$$\text{CRC32}(\text{bytes}[0x04 \dots 0x1F] \,\|\, \text{body\_bytes})$$
During read operations, the engine calculates the checksum over this slice and asserts equality with `crc32`.
