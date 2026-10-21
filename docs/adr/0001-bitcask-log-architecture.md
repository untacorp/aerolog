# ADR 0001: Bitcask Log-Structured Architecture for Mobile Flash Memory

## Status
Accepted

## Context
Traditional mobile databases such as SQLite rely on B-Tree page layouts (typically 4 KB per page). On smartphone NAND flash memory, out-of-order writes and frequent random small mutations incur significant write amplification, leading to premature flash cell wear, elevated battery consumption, and latency spikes during P2P wireless database replication.

## Decision
We implement a pure append-only Bitcask segment log (`.aerolog`) architecture:
1. All write operations (`put`, `delete`, `append`) are strictly sequential appends to the active segment file.
2. In-place overwriting of disk bytes is strictly prohibited.
3. Every record is prefixed with a fixed 32-byte binary header containing a 32-bit hardware-accelerated CRC32 checksum.
4. Sealed segment files are merged in the background by `LogCompactor` to reclaim disk space occupied by overwritten records and soft-deleted tombstones.

## Consequences
- **Positive**: $\mathcal{O}(1)$ sequential write throughput exceeding 200,000 ops/sec; zero random I/O during writes; battery-friendly; complete immunity to page corruption.
- **Negative**: Requires background compaction to reclaim disk space from obsolete records.
