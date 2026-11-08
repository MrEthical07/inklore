# Instructions for coding agents

You are implementing **Inklore**, a knowledge engine for AI coding agents, written in Rust.
Read [README.md](README.md) for what it is. The design lives in [docs/](docs/); the decisions behind it
are in [docs/10-decisions.md](docs/10-decisions.md). Do not contradict them silently.

## How work arrives

- Every piece of work is a task file: `tasks/T-NNN-<name>.md`. Do **exactly** what it asks.
- Before starting, read: this file, the task file, and every doc the task lists under *Read first*.
- Anything the task does not mention is out of scope. If something outside the scope looks wrong,
  note it in your report instead of changing it.
- If the task conflicts with the docs, or a design question comes up that the task does not answer,
  **do not guess**: pick the smallest reasonable option, mark it clearly in the code with
  `// DECISION(T-NNN): …`, and list it in the report under *Decisions made*. If the question blocks
  the task, stop and report it as *blocked*.
- When finished, write your report to `tasks/reports/T-NNN.md` using
  [tasks/REPORT_TEMPLATE.md](tasks/REPORT_TEMPLATE.md), plus any task-specific questions the task asks.

## Git

- Work on a branch named `task/T-NNN-<name>` created from `main`.
- Conventional Commits (`feat(store): …`, `test(store): …`, `docs: …`, `chore: …`, `ci: …`).
  Small commits are fine; the final state must build and pass.
- Do not push, merge, rebase `main`, or rewrite history unless the task says so.
- Do not edit `docs/` unless the task says so. Propose doc changes in the report.

## Rust conventions

- Toolchain is pinned by `rust-toolchain.toml`; do not change it. Edition 2024.
- Library crates use `thiserror` for error types; only binaries may use `anyhow`.
- No `unwrap()`, `expect()` or `panic!` in library code paths reachable from input (files on disk,
  user input, agent requests). Return errors. Tests may unwrap.
- `unsafe` is denied workspace-wide. If a task seems to need it, stop and ask in the report.
- Every public item has a doc comment. Crate-level docs explain the crate's responsibility.
- Logging through `tracing`, never `println!` in libraries.
- Keep functions small and named for what they do. Prefer plain data types and free functions over
  deep trait hierarchies until a second implementation actually exists.
- Integer types and byte layouts on disk are explicit (`u32`, big- or little-endian stated in a
  comment). Never write a Rust struct's memory to disk directly.

## Tests

- Unit tests next to the code (`#[cfg(test)] mod tests`), integration tests in `crates/<crate>/tests/`.
- Use `proptest` for anything with an invariant (encodings round-trip, a tree matches a `BTreeMap`).
- A bug fix comes with a test that failed before the fix.
- Tests must pass on Windows, Linux and macOS: use `std::path`, `tempfile`, no shell commands, no
  hard-coded `/` or `\` separators, no reliance on file-system case sensitivity.

## Dependencies

- Add a dependency only when it clearly earns its place. List every new one in the report with
  version, license and reason.
- Allowed licenses: MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 (MPL only for
  unmodified dependencies). Anything else: ask first.
- Declare shared versions in `[workspace.dependencies]` and use `dep.workspace = true` in crates.

## Definition of done

All of these pass from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps          (with RUSTDOCFLAGS="-D warnings")
```

…plus the task's own acceptance criteria, and the report is written.
