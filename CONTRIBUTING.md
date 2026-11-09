# Contributing to Inklore

Thank you for your interest. This project is built from written task files (see
[tasks/README.md](tasks/README.md)); the rules for anyone writing code here, human or agent, are in
[AGENTS.md](AGENTS.md).

## Prerequisites

- [rustup](https://rustup.rs/). The pinned toolchain installs itself on the first `cargo` call.
- A C linker for your platform:
  - **Windows:** Visual Studio Build Tools with the *Desktop development with C++* workload
    (MSVC and a Windows SDK).
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Linux:** `gcc` or `clang` from your package manager.

## Build and test

The toolchain is pinned by `rust-toolchain.toml`; `rustup` installs it on the first `cargo` call.
Before opening a pull request, all of these must pass from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps          (with RUSTDOCFLAGS="-D warnings")
```

## Branches and commits

- Work on a branch named `task/T-NNN-<name>`, created from `main`.
- Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat(store): …`, `test(store): …`,
  `docs: …`, `chore: …`, `ci: …`).
- The full conventions (Rust style, tests, dependencies) are in [AGENTS.md](AGENTS.md).

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md).

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above (MIT OR Apache-2.0),
without any additional terms or conditions.
