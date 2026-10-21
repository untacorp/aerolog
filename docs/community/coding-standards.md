# Coding & Engineering Standards

All contributions to the AeroLog codebase must adhere to the engineering standards detailed below.

---

## 1. Zero-Emoji Policy

Strictly zero emojis across all code, tests, docstrings, commit messages, PR descriptions, and markdown documentation files.

---

## 2. Invariants & Defensive Programming

1. **No Panics in Engine Code**: Never use `unwrap()` or `expect()` in core engine logic. All failure conditions must return an explicit `Result<T, AeroLogError>`.
2. **Bounds Checking**: Always validate slice bounds, length prefixes, and CRC32 checksums before parsing byte buffers to prevent memory corruption.
3. **Hardware Parity**: Data generated on one platform must parse byte-for-byte identically on all other supported platforms.

---

## 3. Formatting & Linting

Before submitting any code changes, ensure:
```bash
# Code formatting must pass with zero diffs
cargo fmt --all -- --check

# Clippy linter must pass with zero warnings
cargo clippy --all-targets -- -D warnings

# All unit and integration tests must pass
cargo test --all
```

---

## 4. Git Commit Messages

Follow the **Conventional Commits v1.0.0** specification:
```text
<type>(<scope>): <short description>
```
Examples:
* `feat(core): implement segment file manager with crc32 verification`
* `fix(compactor): handle zero-byte tombstone records properly`
* `docs(readme): add architecture diagram and benchmark numbers`
