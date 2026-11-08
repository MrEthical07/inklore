# 06 — Ingest

Ingest turns code into the structural part of the book: entities, hashes and edges. It is
deterministic and never involves an LLM — this is what makes the graph trustworthy.

## Pipeline

```text
discover ─► detect change ─► parse ─► extract symbols ─► hash ─► extract references
   │                                                                     │
   │                                     resolve names ◄──────────────────┘
   │                                          │
   └──────────────► diff against store ◄──────┘ ─► events ─► Merkle up ─► invalidate memberships
                          │
                          └─► commit (one transaction per sync)
```

1. **Discover** files, honouring `.gitignore` and `config.exclude` (the `ignore` crate).
2. **Detect change** cheaply from `(mtime, size)` in `file_state`; confirm with the BLAKE3 content hash.
   Unchanged files are skipped entirely.
3. **Parse** changed files with tree-sitter.
4. **Extract symbols** (definitions) with ranges, kinds and export status.
5. **Hash** each symbol: interface hash and body hash, per [03](03-data-model.md#hashing).
6. **Extract references** — call sites, imports, type references — with their `path:line`.
7. **Resolve names** to entity IDs (below). Unresolved references are kept in an `unresolved` list
   keyed by name, so they can bind later when the target appears.
8. **Diff** against the store, producing events: `added`, `removed`, `body_changed`, `iface_changed`,
   `moved`.
9. **Propagate**: recompute Merkle hashes from changed files up to the root; mark affected feature
   memberships for recomputation (lazily, on the next read).
10. **Commit** everything in one transaction.

The committed text (`.inklore/**`) is synced the same way: changed files (by content hash in
`text_state`) are re-parsed and re-indexed.

## Language packs

Each language is a **pack**: a tree-sitter grammar plus query files (`.scm`). Adding a language means
adding a pack, not changing the core.

```text
packs/go/
  defs.scm        # captures: @def.function @def.method @def.type @def.interface … with @name
  refs.scm        # captures: @ref.call @ref.type @ref.import
  iface.scm       # which child nodes form the interface (params, results, receiver, type params)
  resolve.toml    # name-resolution hints: package = directory, exported = Uppercase, etc.
```

v0 packs: **Rust**, **Go**, **TypeScript/JavaScript**.

## Name resolution

Tree-sitter gives syntax, not meaning: it cannot tell which of three `Save` methods a call refers to.
Two sources of edges:

| Source | Confidence | How |
|---|---|---|
| **SCIP index** | `Exact` | Compiler-backed indexers (`scip-go`, `scip-typescript`, `rust-analyzer scip`) produce precise definitions and references. If configured, ingest runs the indexer (or reads `index.scip`) and uses it for covered files. |
| **Heuristic resolver** | `Heuristic` | Per-language rules over tree-sitter output: import bindings, package/module scoping, receiver types where visible, same-file preference. Ambiguous matches produce multiple heuristic edges, never a silent guess. |

SCIP indexing is slower and needs a working build, so it runs on demand (`inklore sync --precise`) or in
CI; heuristic resolution runs on every incremental sync. Where both exist, exact edges replace heuristic
ones for the same file.

**Known blind spots**, reported rather than hidden: dependency-injection wiring, reflection, dynamic
dispatch through maps of functions, string-based routing, code generation at runtime. Interface →
implementation links are exact with SCIP and heuristic otherwise (structural matching of method sets in
Go; `implements` clauses in TypeScript; `impl Trait for` in Rust).

## Non-code artifacts

Files matching `artifact` globs in config (SQL migrations, OpenAPI specs, config files) become
`Artifact` entities tracked by content hash, so knowledge can be pinned to them. Parsing their insides
(e.g. tables in a migration) is out of scope for v0.

## When ingest runs

| Trigger | Behaviour |
|---|---|
| `inklore sync` | explicit, full change detection |
| git hooks (`post-checkout`, `post-merge`, `post-rewrite`, `post-commit`) | installed by `inklore init --hooks`; sync after branch switches and pulls |
| any engine call | stat-based check of recently touched paths with a short cooldown; full sync if the git HEAD moved |
| file watcher | optional (`inklore serve --watch`) |

A branch switch can change thousands of files. That is just a large incremental sync: unchanged files
are skipped by content hash, and nothing in the committed book is rewritten — freshness is recomputed
against the new code.

## Feature membership

Computed lazily when a brief or check needs it and the relevant Merkle hashes changed: breadth-first
from the feature's seeds over `Calls`, `Implements` and `References` edges, honouring `max_depth`,
`shared_threshold` and `exclude` (see [03](03-data-model.md#features)). Results are cached in
`feature_member` keyed by the graph version.

## Seed suggestions

To help bootstrap features without assuming an architecture, the engine can **suggest** seed candidates
from structure alone: exported symbols with no callers inside the repo, `main` functions, test
entry points, and symbols referenced from artifact files (such as route tables). Optional
framework plugins (e.g. "routes registered with Gin/Express/axum") can add better candidates; they live
outside the core. Suggestions are only suggestions — a person or agent confirms and names them.
