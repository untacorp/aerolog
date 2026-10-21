# ADR 0003: Native HLC Causality at the Storage Header Layer

## Status
Accepted

## Context
In partitioned edge and off-grid wireless environments (e.g. disaster relief camps, field logistics), devices have no reliable access to NTP servers. Wall clocks drift and can reset to epoch zero upon battery exhaustion. Storing HLC timestamps as formatted strings (e.g. `"1710928300000-0-nodeId"`) in database columns introduces string allocation overhead and slow lexicographical parsing during replication.

## Decision
We embed Hybrid Logical Clock (HLC) timestamps directly as native integer fields in the 32-byte binary record header:
```text
[physical_time: 8B Int64 BE] [logical_counter: 2B UInt16 BE] [node_id_hash: 8B Int64 BE]
```
Conflict resolution follows total order Last-Write-Wins (LWW):
```math
\text{HLC}_A > \text{HLC}_B \iff (pt_A > pt_B) \lor (pt_A = pt_B \land lc_A > lc_B) \lor (pt_A = pt_B \land lc_A = lc_B \land node_A > node_B)
```

## Consequences
- **Positive**: Zero string allocation or parsing overhead during replication scans; deterministic convergence across all distributed replicas; monotonic progression even across physical clock resets.
- **Negative**: Last-Write-Wins operates at the record level; fine-grained column-level conflict resolution requires field-level CRDTs.
