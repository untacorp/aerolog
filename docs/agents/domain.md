# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root (if it exists).
- **`docs/community/adr/`**: read ADRs that touch the area you're about to work in (e.g. `0001-bitcask-log-architecture.md`, `0002-dual-tier-indexing.md`, `0003-native-hlc-causality.md`, `0004-pluggable-retention-policy.md`).

If any of these files don't exist, **proceed silently**. Don't flag their absence; don't suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates them lazily when terms or decisions actually get resolved.

## File structure

Single-context repo:

```
/
├── CONTEXT.md
├── docs/community/adr/
│   ├── 0001-bitcask-log-architecture.md
│   ├── 0002-dual-tier-indexing.md
│   ├── 0003-native-hlc-causality.md
│   └── 0004-pluggable-retention-policy.md
└── crates/
    ├── aerolog-core/
    ├── aerolog-cbor/
    ├── aerolog-cli/
    └── aerolog-uniffi/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0001 (Bitcask log architecture), but worth reopening because…_
