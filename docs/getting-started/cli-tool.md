# CLI Diagnostics Tool (`aerolog-cli`)

AeroLog ships with a high-performance standalone command-line diagnostic and benchmark utility (`aerolog`).

---

## 1. Running Throughput & Latency Benchmarks

The `bench` subcommand measures point write throughput, point read throughput, and streaming append performance on physical hardware:

```bash
# Default benchmark: 50,000 records with 128-byte payloads
aerolog bench

# Custom benchmark configuration
aerolog bench --records 100000 --val-size 256
```

### Sample Benchmark Output
```text
==================================================================
           AeroLog Storage Engine Diagnostic Benchmark
==================================================================
Target Directory   : /tmp/aerolog_bench_xyz
Record Count       : 50,000
Payload Size       : 128 bytes
------------------------------------------------------------------
[1/3] Benchmarking Point Key-Value Sequential Writes...
      Completed 50,000 writes in 242.1 ms (206,526 ops/sec)
[2/3] Benchmarking Point Key-Value Random Reads...
      Completed 50,000 reads in 791.4 ms (63,179 ops/sec)
[3/3] Benchmarking Append-Only Stream Ingestion...
      Completed 50,000 appends in 135.3 ms (369,549 ops/sec)
==================================================================
```

---

## 2. Inspecting Segment Files

Inspect binary headers, CRC32 status, and record layouts inside `.aerolog` files without opening the database:

```bash
aerolog inspect ./storage_data/data-000000.aerolog
```

### Sample Output
```text
=== Segment: ./storage_data/data-000000.aerolog ===
Segment File Size: 3,450,112 bytes

[Offset 0x000000] CRC: 0x8F32A10C (VALID) | HLC: 1700000000100-0-0x38AF | Type: Insert
                  Collection: 'users' | Key: 'usr_001' | Payload: 128 bytes
[Offset 0x0000B0] CRC: 0x1290EE4B (VALID) | HLC: 1700000000101-1-0x38AF | Type: Tombstone
                  Collection: 'users' | Key: 'usr_000' | Payload: 0 bytes
```

---

## 3. Dumping Raw Database Records

Dump all current active key-value pairs or stream logs directly to standard output:

```bash
aerolog dump ./storage_data --collection users
```

---

## 4. Offline Database Compaction

Merge fragmented segment files and purge soft-deleted tombstones offline:

```bash
aerolog compact ./storage_data
```
