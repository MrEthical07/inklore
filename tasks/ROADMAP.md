# Task roadmap

Milestones come from [docs/09-roadmap.md](../docs/09-roadmap.md). Tasks near the top are specified
in detail; later ones are outlines and get split or reshaped when we reach them.

Status: ⬜ not started · 🟦 in progress · 🟨 in review · ✅ approved

## M0 — Foundations

| Task | Status | What | Depends on |
|---|---|---|---|
| T-001 | 🟨 | Workspace scaffold: five crates, pinned toolchain, lints, licenses, CI on Windows/Linux/macOS | — |
| T-002 | ⬜ | Evaluation harness skeleton: chain format, runner that drives an agent CLI headlessly, report generator (no real runs yet) | T-001 |
| T-003 | ⬜ | Real agent adapters (Claude Code first); first three task chains on `superapi`, ideally replayed from its real release history; one baseline run | T-002 |

## M1 — Storage core (`inklore-store`)

| Task | Status | What | Depends on |
|---|---|---|---|
| T-004 | ⬜ | Order-preserving key encoding and value codec primitives (pure, no I/O), with property tests | T-001 |
| T-005 | ⬜ | I/O layer: a `FileIo` trait with a real implementation and a simulated one that can drop unsynced writes and tear pages | T-001 |
| T-006 | ⬜ | Page format (header, CRC32C checksum, slotted leaf/branch layout, overflow pages) and the pager with a CLOCK buffer pool | T-004, T-005 |
| T-007 | ⬜ | Copy-on-write B+tree: get, put, delete, range; splits and merges; overflow values | T-006 |
| T-008 | ⬜ | Meta pages, commit protocol, open and recovery, freelist with pending frees, read snapshots and pins | T-007 |
| T-009 | ⬜ | The `Store` trait, the redb adapter, property tests against `BTreeMap`, differential tests against redb | T-008 |
| T-010 | ⬜ | Crash-simulation sweep, corruption tests, fuzz targets | T-009 |
| T-011 | ⬜ | Task event log (append-only segments, CRC per record, torn-tail recovery) | T-005 |
| T-012 | ⬜ | Benchmarks against redb and SQLite; published results | T-009 |

## M2 — Ingest (`inklore-ingest`)

| Task | What |
|---|---|
| T-013 | Language pack format and loader; tree-sitter integration; Go pack: definitions and ranges |
| T-014 | Normalised interface/body hashing and Merkle hashes; golden tests that comments and formatting do not change hashes |
| T-015 | References and heuristic name resolution with confidence; unresolved-reference binding |
| T-016 | Rust and TypeScript/JavaScript packs |
| T-017 | Incremental sync into the store, change events, move detection; `inklore init`, `sync`, `map` |
| T-018 | Ingest performance pass against the 140k-line target |

## M3 — The book (`inklore-book`)

| Task | What |
|---|---|
| T-019 | Committed text format: entries, features, config — exact round-trip parser/writer |
| T-020 | Guards with structured errors; `schema()` |
| T-021 | Freshness evaluation; trust levels from git; `inklore check` with CI exit codes, secret scan, injection lint |

## M4 — Reading (`inklore-engine`)

| Task | What |
|---|---|
| T-022 | Feature seeds and computed membership |
| T-023 | `brief` with budget packing; `expand` |
| T-024 | `map`, `find`, `deps` |
| T-025 | `inklore serve` daemon and client protocol (socket / named pipe) |
| T-026 | MCP server |

## M5 — Tasks and hooks

| Task | What |
|---|---|
| T-027 | Task lifecycle, checkpoint/resume, mechanical event capture |
| T-028 | Scope check with attribution |
| T-029 | Claude Code hooks and the `inklore-task` skill |

## M6 — Bootstrap and verify

| Task | What |
|---|---|
| T-030 | Doc import workflow, seed suggestions, coverage |
| T-031 | `inklore-verify` and `inklore-check` skills; backfill workflow |
| T-032 | SCIP import for exact edges |

## M7 — Evaluate and release v0.1

| Task | What |
|---|---|
| T-033 | Full evaluation across repos and arms; written report |
| T-034 | Packaging, install docs, release |
