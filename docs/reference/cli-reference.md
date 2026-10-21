# CLI Command Reference (`aerolog`)

Complete flag and argument reference for the `aerolog-cli` binary.

---

## Global Syntax

```bash
aerolog [SUBCOMMAND] [OPTIONS]
```

---

## Subcommands

### 1. `bench`
Runs the built-in storage throughput and latency benchmark suite.

#### Arguments & Options
* `--records <COUNT>`: Number of operations to execute per benchmark stage (default: `50000`).
* `--val-size <BYTES>`: Size of the binary payload per record in bytes (default: `128`).
* `--dir <PATH>`: Custom directory for temporary benchmark files (default: system temporary directory).

#### Example
```bash
aerolog bench --records 100000 --val-size 256
```

---

### 2. `inspect`
Reads and inspects the binary headers and CRC32 validity of an `.aerolog` segment file.

#### Arguments & Options
* `<SEGMENT_PATH>`: Path to the `.aerolog` file to inspect.
* `--verbose`: Dumps complete payload byte hex previews.

#### Example
```bash
aerolog inspect /var/data/data-000000.aerolog
```

---

### 3. `dump`
Outputs active records from a collection or stream to standard output.

#### Arguments & Options
* `<DB_DIR>`: Path to the AeroLog database directory.
* `--collection <NAME>`: Collection to filter by.
* `--format <FORMAT>`: Output format (`json` or `raw`).

#### Example
```bash
aerolog dump /var/data --collection users --format json
```

---

### 4. `compact`
Triggers offline compaction across all sealed segment files in a database directory.

#### Arguments & Options
* `<DB_DIR>`: Path to the AeroLog database directory.

#### Example
```bash
aerolog compact /var/data
```
