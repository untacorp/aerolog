# Contributing to AeroLog

Thank you for your interest in contributing to **AeroLog**! We welcome contributions from developers worldwide to build the fastest, most reliable local-first storage engine for off-grid and edge systems.

---

## Code of Conduct

All contributors and participants must adhere to our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Development Environment Setup

### Prerequisites
- **Rust Toolchain**: Rust 1.80+ (stable). Install via [rustup.rs](https://rustup.rs).
- **Cargo**: Standard package manager included with Rust.

### Clone and Build
```bash
git clone https://github.com/untacorp/aerolog.git
cd aerolog
cargo build
```

### Running Tests
```bash
cargo test --all
```

### Code Formatting and Linting
All pull requests must pass linter and formatting checks:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

---

## Commit Guidelines

AeroLog enforces **Conventional Commits v1.0.0**:

```text
<type>(<scope>): <short imperative description>

[optional body explaining motivation and technical details]

[optional footer(s)]
```

### Allowed Types:
- `feat`: A new feature or capability.
- `fix`: A bug fix.
- `docs`: Documentation updates only.
- `perf`: Code changes improving execution speed or memory efficiency.
- `refactor`: Code changes that neither fix a bug nor add a feature.
- `test`: Adding missing tests or correcting existing tests.
- `ci`: Changes to CI/CD workflows and configuration.
- `chore`: Maintenance tasks, dependency bumps.

---

## Zero-Emoji Policy

AeroLog strictly enforces a **Zero-Emoji Policy**. Do not include emojis in source code, comments, docstrings, commit messages, PR titles, PR descriptions, or documentation.

---

## License

By contributing to AeroLog, you agree that your contributions will be licensed under the [Apache License, Version 2.0](LICENSE).
