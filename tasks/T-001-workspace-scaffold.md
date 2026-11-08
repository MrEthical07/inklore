# T-001 — Workspace scaffold

**Milestone:** M0 — Foundations · **Depends on:** nothing · **Branch:** `task/T-001-workspace-scaffold`

## Read first
- `AGENTS.md`
- `README.md`
- `docs/09-roadmap.md` (workspace layout)
- `docs/10-decisions.md` (D2, D12, D17, D19)

## Goal
Turn this folder into a Git repository with a Rust workspace that builds, lints and tests cleanly on
Windows, Linux and macOS, with licensing and CI in place. **No product code** — only the skeleton every
later task builds on.

## Steps

### 1. Git
1. If the folder is not yet a Git repository: `git init -b main`.
2. Add `.gitattributes` and `.gitignore` (below) **first**, then commit everything already in the folder
   (`README.md`, `AGENTS.md`, `CLAUDE.md`, `docs/`, `tasks/`) to `main` as
   `docs: initial design and task workflow`.
3. Create the branch `task/T-001-workspace-scaffold` from `main` and do the rest of the work there.

### 2. Repository files

| File | Contents |
|---|---|
| `.gitattributes` | `* text=auto eol=lf`; mark common binary types (`*.png`, `*.db`) as `binary` |
| `.gitignore` | `/target/`, `.inklore/local/`, editor folders (`.idea/`, `.vscode/` except a shared `settings.json` if you add one — don't add one), OS junk (`.DS_Store`, `Thumbs.db`) |
| `.editorconfig` | UTF-8, LF, final newline, trim trailing whitespace; 4-space indent for `*.rs` and `*.toml`; 2 spaces for `*.yml`, `*.yaml`, `*.json`, `*.md` |
| `rust-toolchain.toml` | `channel = "1.97.0"`, `components = ["rustfmt", "clippy"]`, `profile = "minimal"` |
| `rustfmt.toml` | `style_edition = "2024"` only — no unstable options |
| `clippy.toml` | `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-print-in-tests` all `true` |
| `LICENSE-MIT` | Standard MIT text, `Copyright (c) 2026 The Inklore Authors` |
| `LICENSE-APACHE` | The **exact** Apache License 2.0 text from <https://www.apache.org/licenses/LICENSE-2.0.txt> — do not retype it from memory |
| `CONTRIBUTING.md` | Short: how to build and test (the definition-of-done commands), branch and commit conventions (point to `AGENTS.md`), and the standard Rust dual-license contribution clause ("Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion … shall be dual licensed as above, without any additional terms or conditions.") |
| `CODE_OF_CONDUCT.md` | Contributor Covenant 2.1, verbatim from its official source; leave the contact line as `[CONTACT TO BE ADDED]` |

Update `README.md` only by appending a `## License` section with the standard dual-license wording
(MIT OR Apache-2.0, linking both files) and the contribution clause. Change nothing else in it.

### 3. Workspace

Root `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.0.0"
edition = "2024"
rust-version = "1.97"
license = "MIT OR Apache-2.0"
authors = ["The Inklore Authors"]
# repository: omit for now (not decided yet)

[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
dbg_macro = "warn"
todo = "warn"
print_stdout = "warn"

[workspace.dependencies]
# internal crates (path dependencies), and shared external versions, declared here
```

Five crates under `crates/`, each with `version`, `edition`, `rust-version`, `license`, `authors` taken
from the workspace (`*.workspace = true`), `publish = false` for now, and `[lints] workspace = true`:

| Crate | Kind | Depends on (internal) | Crate doc comment (`//!`) must say |
|---|---|---|---|
| `inklore-store` | lib | — | Embedded storage engine: pager, copy-on-write B+tree, task event log, and the `Store` trait. Knows nothing about code or knowledge. |
| `inklore-ingest` | lib | — | Turns source code into structure: parsing, symbol extraction, structural hashing, edges. Pure: does not touch storage. |
| `inklore-book` | lib | — | The book's data model, the committed `.inklore/` text format, and the guards that validate writes. Pure: does not touch storage. |
| `inklore-engine` | lib | store, ingest, book | The only entrance to the book: brief assembly, freshness, tasks, scope check. Orchestrates the other crates. |
| `inklore-cli` | bin `inklore` | engine | The `inklore` command: CLI, and later the `serve` daemon, MCP server and hook commands. |

- Each library's `lib.rs` contains **only** the crate doc comment (2–5 lines, expanding the table text
  and linking the relevant `docs/` file). No placeholder functions, no `todo!()`.
- Internal dependencies are declared in `[workspace.dependencies]` as path dependencies and used with
  `.workspace = true`. Add a one-line `use` or `extern crate` only if needed to keep the dependency;
  otherwise an unused path dependency is fine.
- `inklore-cli`: `[[bin]] name = "inklore"`, uses `clap` (derive) only. `inklore --version` prints
  `inklore 0.0.0`; `inklore --help` shows a one-line description: *"Inklore: a knowledge engine for AI
  coding agents."* No subcommands yet.
- Add one integration test in `crates/inklore-cli/tests/cli.rs` that runs the built binary
  (`env!("CARGO_BIN_EXE_inklore")`) with `--version` and asserts the output. No extra test crates.

### 4. CI — `.github/workflows/ci.yml`

- Triggers: `push` to `main`, and `pull_request`.
- `concurrency` group per ref, cancelling in-progress runs.
- `permissions: contents: read`.
- Jobs:
  - **fmt** (ubuntu-latest): `cargo fmt --all -- --check`
  - **clippy** (matrix: ubuntu-latest, windows-latest, macos-latest):
    `cargo clippy --workspace --all-targets -- -D warnings`
  - **test** (same matrix): `cargo test --workspace`
  - **doc** (ubuntu-latest): `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS: -D warnings`
- The toolchain comes from `rust-toolchain.toml` (rustup installs it automatically on first `cargo`
  call; a `rustup show` step makes that explicit). Do not hard-code the version in the workflow.
- Cache with `Swatinem/rust-cache`. Use the current major version of each action and list them in the
  report.
- `fail-fast: false` on the matrix.

## Out of scope
- Any product code, data types, subcommands, or dependencies beyond `clap`.
- `eval/`, `packs/`, `skills/` folders (later tasks create them).
- Release workflows, `cargo-deny`, coverage, badges.
- Pushing to GitHub.

## Acceptance criteria
1. All definition-of-done commands in `AGENTS.md` pass on your machine.
2. `cargo run -p inklore-cli -- --version` prints exactly `inklore 0.0.0`.
3. `cargo test --workspace` runs the CLI integration test and it passes.
4. `git log --oneline main` shows the single design commit; the task branch contains the scaffold.
5. `cargo tree --workspace --depth 1` shows `inklore-engine` depending on store, ingest and book, and
   `inklore-cli` depending on engine and clap.
6. `LICENSE-APACHE` matches the official text exactly (state in the report how you obtained it).

## Report — task-specific questions
Answer these in section 11 of the report:
1. Did rustup install 1.97.0 automatically from `rust-toolchain.toml`? Any problem on Windows?
2. Output of `cargo tree --workspace --depth 1`.
3. Which version of each GitHub Action did you use?
4. Where you obtained `LICENSE-APACHE` and `CODE_OF_CONDUCT.md`, and whether they are byte-for-byte
   copies.
5. Anything in `AGENTS.md` that was unclear or that you would change.
