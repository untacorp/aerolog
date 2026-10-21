# Philosophy & Architectural Motivation

## The Embedded Storage Challenge

Mobile devices, IoT nodes, and off-grid field hardware operate under physical constraints fundamentally different from data center servers:
1. **Flash Memory Wear**: NAND flash endurance is finite. In-place page overwrites wear out storage cells and trigger costly write amplification.
2. **Abrupt Power Cuts**: Battery exhaustion, hardware disconnects, and OS process terminations happen without warning.
3. **Severe RAM Limits**: Mobile operating systems (Android, iOS) aggressively terminate background processes whose memory footprint exceeds platform limits.
4. **Lack of Central Coordination**: Local-first networks communicate peer-to-peer over intermittent links (Bluetooth Low Energy, Wi-Fi Direct, LoRa) without NTP clock servers.

Traditional storage engines were not designed around these four simultaneous constraints.

---

## Comparative Storage Paradigms

```
+-------------------+---------------------------+-----------------------------------+
| Paradigm          | Primary Data Structure    | Key Limitations on Edge Hardware  |
+-------------------+---------------------------+-----------------------------------+
| Relational B-Tree | 4KB/16KB Paged Tree       | High write amplification; random  |
| (SQLite, Berkeley)| In-Place Leaf Overwriting | I/O wears flash; page tearing risk|
+-------------------+---------------------------+-----------------------------------+
| LSM-Tree          | MemTable + Multi-Level    | Heavy write stalls during merge;  |
| (RocksDB, LevelDB)| SSTable Compactions       | high read amplification (Bloom)   |
+-------------------+---------------------------+-----------------------------------+
| Append-Only Log   | Bitcask Sequential Log    | RAM bounded by active key count;  |
| (AeroLog)         | In-Memory Dual-Tier Index | requires periodic compaction      |
+-------------------+---------------------------+-----------------------------------+
```

### Why Not B-Trees for Edge Sync?
Engines like SQLite use page-based B-Trees. When updating a single 32-byte field in a record, the database engine must:
1. Write the modified 4,096-byte page to a rollback journal or Write-Ahead Log (WAL).
2. Checkpoint and rewrite the 4,096-byte page back to the main database file.
3. Update B-Tree parent navigation pointers if a page split occurs.

This results in a **10x to 100x write amplification factor**, generating excessive flash cell erase cycles, wasting battery power, and introducing vulnerability to page tearing during sudden power failure.

### Why Not LSM-Trees for Off-Grid Nodes?
Log-Structured Merge-Trees (such as RocksDB or LevelDB) improve sequential write speed by collecting mutations in a RAM MemTable before flushing them to Level 0 SSTables. However:
* Background multi-level compactions generate unpredictably high CPU and I/O spikes.
* Point reads often traverse multiple SSTable levels, relying heavily on Bloom filters and large in-memory block caches that exceed mobile RAM allocations.

---

## The AeroLog Approach: Bitcask + HLC

AeroLog takes the elegant **Bitcask model** originally conceived by Eric Brewer and Basho Technologies, and re-engineers it in pure Rust with three critical innovations:

1. **Hardware-Accelerated Integrity**: Every single append carries an upfront 32-byte fixed header protected by hardware CRC32. Corrupt tails from unexpected power cuts are isolated instantaneously during cold boot recovery.
2. **Dual-Tier Memory Partitioning**: By decoupling mutable Key-Value point entities (indexed in `KeyDir`) from append-only stream records (indexed in `SparseStreamIndex`), AeroLog guarantees that message streams (chat, sensors) **never flood RAM**, maintaining a resident set size strictly under 15 MB.
3. **Native Causality Clocking**: Rather than relying on unreliable system wall clocks or NTP daemons, every record header embeds a monotonic Hybrid Logical Clock (HLC). This enables deterministic, zero-conflict Last-Write-Wins (LWW) resolution across distributed peers.
