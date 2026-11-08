# 12 — Prior art

Inklore sits next to two crowded categories — code-graph servers for agents and agent memory tools — and
borrows from storage and build-system engineering. This page records what exists, as of November 2026,
and how Inklore differs. Descriptions come from each project's public pages and may lag behind them.

## Code intelligence for agents

| Project | What it does | Difference |
|---|---|---|
| [codebase-memory-mcp](https://deusdata.github.io/codebase-memory-mcp/) | C binary; tree-sitter for 64 languages; SQLite-backed code graph; 14 MCP tools; file watcher | Structure only, no knowledge layer or staleness of notes |
| [tokensave](https://github.com/aovestdipaperino/tokensave) | Rust; libSQL graph + FTS5; 80+ tools; incremental sync; records decisions and session recall | Decisions are not anchored to code hashes; large tool surface |
| CodeGraph, GitNexus, Repowise | Code graphs exposed over MCP; Repowise flags when its index diverges from HEAD | No anchored, typed knowledge |
| Serena, Semble | Semantic / hybrid code search for agents | Retrieval, not memory |
| Sourcegraph / SCIP | Precise code intelligence; SCIP index format | We import SCIP indexes for exact edges |
| Aider repo map | Ranked map of repository symbols in the prompt | Per-session, no persistence |

## Agent memory

| Project | What it does | Difference |
|---|---|---|
| [bettermemory](https://pypi.org/project/bettermemory/) | Markdown memories; staleness verdict from age, cited paths and commits since confirmation | Staleness is heuristic (paths + commits), not structural hashes |
| repolore (emyann) | Claude Code plugin: code-derived LLM wiki with cited claims, blob-SHA drift checking, coverage auditing | Closest in spirit. File-level (blob) hashes, wiki pages rather than typed entries; no graph, features, tasks or scope check |
| lorebase (AndyShaman) | Self-maintaining wiki for Claude Code (LLM-wiki pattern) | Documentation-centric, no code anchoring |
| mem0, cognee, Zep/Graphiti, Supermemory | General agent memory, some with temporal graphs | Not code-aware |
| Sentra (commercial) | Bi-temporal context graph; facts record when they became true and stopped | Closest on time-validity; closed source |

## What Inklore combines that we have not found elsewhere

1. **Symbol-level structural anchoring with two hashes** (interface vs body), so contracts and behavior
   notes go stale for different reasons.
2. **Typed knowledge with scoped inheritance** — rules and decisions reach exactly the code they govern.
3. **Features as declared seeds plus computed membership**, making shared code and its impact explicit.
4. **Tasks held by the engine**, with mechanical capture and compaction recovery.
5. **Scope check from hash diffs** against a pinned snapshot.
6. **Team-first storage**: committed text that merges in git, plus a rebuildable local store.
7. **End-to-end, multi-session evaluation** as part of the project.

## Engineering we borrow from

- **LMDB, redb** — copy-on-write B+trees, alternating meta pages, reader snapshots
- **Salsa, Bazel** — incremental recomputation keyed by content hashes
- **Git** — content addressing and Merkle trees
- **Context-engineering research** — see [01](01-problem.md#evidence)
