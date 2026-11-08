# 09 — Roadmap

Each milestone ends with something that runs and is tested. Order matters: the evaluation harness comes
first so later work is measured, and the store comes before anything that depends on it.

## Workspace layout

```text
inklore/
  crates/
    inklore-store/    # pager, COW B+tree, event log, Store trait (+ redb adapter)
    inklore-ingest/   # tree-sitter packs, hashing, resolution, SCIP import
    inklore-book/     # data model, committed text format, guards
    inklore-engine/   # operations: brief, map, find, deps, tasks, freshness, scope check
    inklore-cli/      # `inklore` binary: CLI + `serve` daemon + `mcp` + hook commands
  packs/              # language packs (grammar queries)
  skills/             # agent skills (inklore-task, inklore-verify, inklore-check, bootstrap)
  eval/               # evaluation harness and task chains
  docs/
```

## Milestones

### M0 — Foundations
- Cargo workspace, CI on Windows, Linux and macOS, `rustfmt`/`clippy` gates
- `LICENSE-MIT`, `LICENSE-APACHE`, `CONTRIBUTING.md`, code of conduct
- Evaluation harness skeleton with 3 task chains on `superapi` and a baseline run
- **Done when:** CI is green on all three OSes and a baseline eval report exists.

### M1 — Storage core (`inklore-store`)
- Pager, buffer pool, page checksums
- Copy-on-write B+tree: get/put/delete/range, overflow pages
- Meta pages, commit protocol, freelist with pinned snapshots
- Task event log (append-only segments)
- Property, differential (vs redb), crash-simulation and fuzz tests; benchmarks vs redb and SQLite
- **Done when:** the crash sweep passes and benchmarks are published in the repo.

### M2 — Ingest (`inklore-ingest`)
- Packs for Rust, Go, TypeScript/JavaScript: definitions, references, interface parts
- Normalised interface/body hashing, Merkle hashes, move detection
- Heuristic name resolution with confidence; unresolved-reference binding
- Incremental sync; `inklore init`, `inklore sync`, `inklore map`
- **Done when:** a 140k-line repo ingests within the target and a one-file change syncs in < 50 ms.

### M3 — The book (`inklore-book`)
- Committed text format (entries, features, config), parser and writer that round-trip exactly
- Guards with structured errors; `schema()`
- Freshness evaluation; `inklore check` with CI exit codes
- Trust levels from git (`trusted` / `unreviewed`, D21); secret scan and injection lint in `check`
- **Done when:** hand-edited and engine-written entries validate, and `check` flags stale anchors after
  code edits.

### M4 — Reading (`inklore-engine`)
- Feature seeds and computed membership (shared nodes, excludes)
- `brief` with budget packing, `map`, `find`, `deps`, `expand`
- `inklore serve` daemon and `inklore mcp` server
- **Done when:** briefs meet latency targets and the first "pull" eval arm runs.

### M5 — Tasks and hooks
- Task lifecycle, mechanical event capture, checkpoint/resume
- Scope check with attribution
- Claude Code hooks and the `inklore-task` skill
- **Done when:** the "push" arm runs, including tasks that compact.

### M6 — Bootstrap and verify
- Doc import workflow, seed suggestions, coverage
- `inklore-verify` and `inklore-check` skills; backfill workflow
- SCIP import for exact edges
- **Done when:** `superapi` is bootstrapped end-to-end with its existing docs imported.

### M7 — Evaluate and release v0.1
- Full evaluation across repos and arms; written report
- Install docs, `cargo install inklore`, prebuilt binaries
- **Done when:** v0.1 is tagged with its evaluation report.

### After v0.1
- CI-signed Merkle manifest of `.inklore/entries/` (trust level `signed`, D21)
- `private` entries encrypted to members' own public keys
- Concurrent-agent claims; more language packs; read-only `mmap` path

The task-by-task breakdown of these milestones, with the prompts given to the coding agent, lives in
[`tasks/ROADMAP.md`](../tasks/ROADMAP.md).
