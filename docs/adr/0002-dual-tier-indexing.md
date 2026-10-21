# ADR 0002: Dual-Tier Memory Indexing (KeyDir vs Sparse Stream Index)

## Status
Accepted

## Context
Standard Bitcask implementations store every key in an in-memory hash table (`KeyDir`). While practical on backend servers with tens of gigabytes of RAM, storing every message key of an active chat system in RAM on low-end mobile devices inevitably triggers operating system Out-Of-Memory (OOM) process termination.

## Decision
We separate in-memory indexing into two distinct tiers:
1. **KeyDir Index**: Dedicated exclusively to mutable Key-Value entities. KeyDir maps `(collection, key)` to disk coordinates `(file_id, offset, length, hlc)`.
2. **Sparse Stream Index**: Dedicated to append-only streams (chat messages, sensor logs, audit trails). Stream entries are never inserted into the KeyDir. Instead, the engine records sparse checkpoints (e.g., 1 pointer per 100 entries). Sequential range scans read contiguous disk blocks starting from the nearest checkpoint.

## Consequences
- **Positive**: Strict memory boundary guaranteed: RAM footprint remains under 15 MB even when holding hundreds of thousands of stream records on disk. Single-seek $\mathcal{O}(1)$ random reads preserved for Key-Value data.
- **Negative**: Stream point reads by arbitrary non-sequential keys are not supported; streams are accessed sequentially or by recent slices.
