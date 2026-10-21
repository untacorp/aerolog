# ADR 0004: Pluggable Declarative Stream Retention Policies

## Status
Accepted

## Context
Mobile edge devices possess finite disk storage. Unbounded append-only streams (such as chat rooms or sensor updates) will eventually exhaust flash capacity if historical entries are never evicted. Hardcoding fixed retention limits directly into the storage engine violates developer flexibility, as medical triage logs require indefinite preservation while temporary telemetry logs only require short-term buffering.

## Decision
We implement a pluggable, declarative retention contract (`RetentionPolicy`):
- `RetentionPolicy::KeepAll`: Retains all entries indefinitely (default).
- `RetentionPolicy::MaxEntries(N)`: Bounded historical window retaining at most the latest $N$ entries per stream.
- `RetentionPolicy::TimeToLive(T)`: Time-based expiration evicting entries older than $T$ milliseconds.
Retention policies are evaluated and applied during segment compaction cycles by `LogCompactor`.

## Consequences
- **Positive**: Product owners and application developers retain full control over disk quotas; obsolete stream entries are reclaimed cleanly without interrupting concurrent operations.
- **Negative**: Eviction occurs at compaction boundaries rather than instantaneously upon append.
