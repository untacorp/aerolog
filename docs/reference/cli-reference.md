# CLI Command Reference (`aerolog`)

Complete flag, command, and REPL reference for the `aerolog-cli` binary.

---

## Global Options

```bash
aerolog [--db <PATH>] [COMMAND]
```

* `--db, -d <PATH>`: Filesystem path to the AeroLog database directory (default: `./aerolog_data`).
* `--help, -h`: Prints command help and exits.
* `--version, -V`: Prints current version.

Running `aerolog` without arguments launches the **Interactive Database Shell (REPL)**.

---

## Subcommands

### 1. `put`
Stores a Key-Value pair in a specified collection, generating a monotonic HLC timestamp.

```bash
aerolog [--db <PATH>] put <COLLECTION> <KEY> <VALUE>
```

```bash
aerolog --db ./storage put users alice '{"name":"Alice","role":"admin"}'
```

---

### 2. `get`
Retrieves a record payload by collection and key via single-seek `KeyDir` lookup.

```bash
aerolog [--db <PATH>] get <COLLECTION> <KEY>
```

```bash
aerolog --db ./storage get users alice
```

---

### 3. `delete`
Soft-deletes a record by appending an immutable tombstone record.

```bash
aerolog [--db <PATH>] delete <COLLECTION> <KEY>
```

```bash
aerolog --db ./storage delete users alice
```

---

### 4. `scan`
Scans and displays all live, non-deleted key-value pairs in a collection.

```bash
aerolog [--db <PATH>] scan <COLLECTION>
```

```bash
aerolog --db ./storage scan users
```

---

### 5. `stream`
Manages append-only event streams (chat partitions, sensor metrics, changelogs).

#### Stream Actions
* `append <STREAM_ID> <PAYLOAD>`: Appends an event to the stream partition.
* `read <STREAM_ID> [--limit <N>]`: Reads the most recent $N$ events from the stream (default limit: 10).
* `info <STREAM_ID>`: Displays total entry count for the stream partition.

```bash
# Append an event
aerolog stream append mesh_chat "Node B joined cluster"

# Read recent 20 messages
aerolog stream read mesh_chat --limit 20

# View stream metadata
aerolog stream info mesh_chat
```

---

### 6. `compact`
Triggers offline segment compaction, merging sealed `.aerolog` files, evicting dead records and expired tombstones, and applying retention policies.

```bash
aerolog [--db <PATH>] compact
```

---

### 7. `inspect`
Displays physical segment counts, active segment identifier, and disk usage metrics.

```bash
aerolog [--db <PATH>] inspect
```

---

### 8. `dump`
Sequentially scans and dumps all decoded physical records from disk to stdout.

```bash
aerolog [--db <PATH>] dump
```

---

### 9. `repl` (Interactive Database Shell)
Launches an interactive prompt to execute database operations continuously without re-opening files on every command.

```bash
aerolog [--db <PATH>] repl
```

#### REPL Commands
* `PUT <collection> <key> <value>`
* `GET <collection> <key>`
* `DELETE <collection> <key>`
* `SCAN <collection>`
* `STREAM APPEND <id> <payload>`
* `STREAM READ <id> [limit]`
* `STREAM INFO <id>`
* `COMPACT`
* `INSPECT`
* `HELP`
* `EXIT` or `QUIT`
