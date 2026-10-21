# Before You Start

Understanding when to choose AeroLog—and when an alternative technology is more appropriate—is essential to building robust systems.

---

## When to Choose AeroLog

AeroLog is the optimal storage solution when your application satisfies one or more of the following criteria:

* **High-Frequency Writes**: You require sustained write throughput exceeding 100,000 writes/second with zero disk write stalls.
* **Flash Memory Longevity**: You are deploying on edge appliances, microcontrollers, Raspberry Pi, or mobile devices where flash cell endurance and low power consumption matter.
* **Offline-First & P2P Replication**: You are building local-first applications where nodes replicate data peer-to-peer over intermittent wireless links (BLE, Wi-Fi Direct, LoRa) without internet access.
* **Crash-Resilience as a Hard Requirement**: You cannot tolerate database corruption caused by unexpected power disconnection, battery loss, or hard reboots.
* **Predictable Low-Latency Point Reads**: Your application needs guaranteed $O(1)$ point reads via direct in-memory coordinate seeks.
* **Strict Memory Budgets**: Your application operates within constrained mobile background memory budgets (under 15 MB RSS).

---

## When Not to Choose AeroLog

AeroLog is a low-level storage primitive, not an all-purpose relational database management system. You should choose an alternative storage engine (such as SQLite or PostgreSQL) if you require:

* **Complex Relational SQL Queries**: AeroLog does not feature a SQL parser, query planner, or declarative query language. If your application relies on `JOIN`, `GROUP BY`, or complex nested subqueries, implement them at the application layer or choose a relational engine.
* **Extensive Range Scans on Key-Value Data**: `KeyDir` uses an in-memory hash map. Scanning ordered key ranges (e.g., `WHERE key BETWEEN 'order_100' AND 'order_200'`) requires iterating the map rather than traversing an ordered B-Tree index.
* **Massive Datasets with Tens of Millions of Unique Point Keys on Low-RAM Devices**: Because `KeyDir` keeps an entry in RAM for every unique active key, storing 50 million unique point keys requires several hundred megabytes of RAM. *(Note: For large volumes of sequential event logs, use AeroLog Streams, which are sparse-indexed and bounded in memory).*
* **Full-Text Search (FTS)**: AeroLog stores raw byte payloads. Text tokenization, inverted indexing, and phonetic search must be implemented by downstream consumers.

---

## Core System Invariants

Every subsystem in AeroLog is designed and validated against four inviolable architectural rules:

1. **Sequential Disk I/O Invariant**: The database writer strictly appends to active segment files. In-place modification of existing disk bytes is forbidden.
2. **Dual-Tier Memory Invariant**: Stream partitions must never populate the `KeyDir`. Total RSS memory must remain bounded below 15 MB even when managing 100,000+ stream entries.
3. **Monotonic Causality Invariant**: Every written record carries a total ordering tuple `(physical_time, logical_counter, node_id_hash)` that ratchets forward deterministically upon local writes and remote sync events.
4. **Crash Isolation Invariant**: Unwritten, incomplete, or corrupted record fragments resulting from power cuts must be isolated and discarded during recovery without altering valid historical records.
