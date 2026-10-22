# CLI Tool & Interactive Shell (`aerolog-cli`)

AeroLog ships with a standalone, production-grade command-line interface (`aerolog`) for inspecting, querying, writing, and administering AeroLog databases.

---

## 1. Installation

Build and install `aerolog` directly using `cargo`:

```bash
cargo install --path crates/aerolog-cli
```

Or run directly from the workspace:

```bash
cargo run --release -p aerolog-cli -- --help
```

---

## 2. Interactive Shell (REPL)

Running `aerolog` without arguments (or with `aerolog repl`) launches an interactive database shell similar to `redis-cli` or `sqlite3`:

```text
$ aerolog --db ./my_data

==================================================================
             AeroLog Interactive Shell (REPL)                     
==================================================================
Database: "./my_data"
Type 'HELP' for commands, 'EXIT' to quit.
------------------------------------------------------------------
aerolog> PUT users alice {"name":"Alice","role":"admin"}
OK [HLC: 1792627331615-0-000000004145524f]
aerolog> GET users alice
{"name":"Alice","role":"admin"}
aerolog> STREAM APPEND emergency "Node B online"
APPENDED [Seq: 0, HLC: 1792627331624-1-000000004145524f]
aerolog> STREAM READ emergency 10
Stream 'emergency' (1 entries):
  [#000000 | HLC: 1792627331624-1-000000004145524f] Node B online
aerolog> EXIT
Closing database...
Goodbye.
```

---

## 3. Direct Command-Line Operations

You can also run database commands directly from scripts or CI/CD pipelines:

### Key-Value Operations
```bash
# Insert or update
aerolog --db ./my_data put devices sensor_01 '{"temp": 24.5}'

# Read value
aerolog --db ./my_data get devices sensor_01

# List all keys in a collection
aerolog --db ./my_data scan devices

# Soft-delete a record
aerolog --db ./my_data delete devices sensor_01
```

### Append-Only Event Streams
```bash
# Append an event
aerolog --db ./my_data stream append mesh_logs "Link state: ESTABLISHED"

# Read recent events
aerolog --db ./my_data stream read mesh_logs --limit 50

# View event count
aerolog --db ./my_data stream info mesh_logs
```

---

## 4. Database Diagnostics & Maintenance

### Inspect Storage & Segments
```bash
aerolog --db ./my_data inspect
```

Output:
```text
Inspecting AeroLog Database at "./my_data"
Total Segments : 3
Total Disk Size: 48.00 MB (50331648 bytes)
Active Segment : data-000003.aerolog
  - data-000001.aerolog (size: 16384.00 KB)
  - data-000002.aerolog (size: 16384.00 KB)
  - data-000003.aerolog (size: 16384.00 KB)
```

### Log Compaction
Reclaims disk space by removing dead records and merging sealed files:

```bash
aerolog --db ./my_data compact
```

Output:
```text
Starting background compaction on "./my_data"...
Compaction finished successfully:
  Disk size before: 48.00 MB
  Disk size after : 31.25 MB
  Records evicted : 14200
  Space reclaimed : 34.9%
```
