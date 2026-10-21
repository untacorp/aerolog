# AeroLog Documentation

Welcome to the technical documentation for **AeroLog**, an ultra-fast, local-first, multi-model storage engine engineered from first principles in pure Rust.

## Purpose & Scope
AeroLog serves as a drop-in high-performance embedded database designed for:
1. **Edge & Off-Grid Devices**: Low-power consumption, zero garbage collection (Zero-GC), and sequential flash-friendly I/O.
2. **P2P Wireless Mesh Synchronization**: Built-in monotonic causality (Hybrid Logical Clocks) and zero-NTP Last-Write-Wins convergence.
3. **Multi-Model Workloads**: Supporting both mutable Key-Value entities and append-only event streams (such as offline chat history).

## Architectural Foundations
Explore our formal Architecture Decision Records:
- [ADR 0001: Bitcask Log-Structured Architecture](adr/0001-bitcask-log-architecture.md)
- [ADR 0002: Dual-Tier Indexing (KeyDir vs Sparse Stream Index)](adr/0002-dual-tier-indexing.md)
- [ADR 0003: Native HLC Causality](adr/0003-native-hlc-causality.md)
- [ADR 0004: Pluggable Retention Policy](adr/0004-pluggable-retention-policy.md)
