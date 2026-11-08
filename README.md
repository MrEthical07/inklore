# Inklore

**A knowledge engine for AI coding agents.** Inklore keeps what agents (and people) learn about a
codebase — why code exists, what it should do, the rules it follows, the decisions behind it, the
plans in flight — and knows exactly when each piece of that knowledge stops matching the code.

> Status: design phase (v0 spec). Nothing is built yet. Start with [docs/01-problem.md](docs/01-problem.md).

## The problem in one paragraph

A coding agent's only memory is its context window. Every task starts by re-reading files to rebuild
an understanding the previous session already had. Large changes fill the window with whole files,
quality drops as it fills, and compaction summarises away the small details and the in-flight plan.
When the session ends, everything is lost. Notes in `.md` files help, but nothing tells the agent
which of them no longer match the code — and wrong memory is worse than no memory.

## The idea

```text
   agents / people
         │   (skills explain, hooks push, MCP/CLI pull)
         ▼
 ┌─────────────── Engine ───────────────┐   the only way in or out
 │ guards · brief assembly · freshness  │
 │ scope check · tasks · ingest         │
 └───────────────────┬──────────────────┘
                     ▼
 ┌─────────────── The Book ─────────────┐
 │ committed:  .inklore/  (text, merges │   knowledge: rules, decisions,
 │             cleanly in git)          │   contracts, gotchas, plans …
 │ local:      graph + index (derived,  │   structure: computed from code,
 │             rebuilt from code + text)│   never written by agents
 └──────────────────────────────────────┘
```

- **The book** holds facts. Structure (files, symbols, call graph) is computed from code. Knowledge is
  written by agents and people, pinned to specific code by structural hashes.
- **The engine** is the only entrance. It validates every write, tells the agent what it needs, and
  assembles a token-budgeted **brief** for whatever the agent is working on.
- **Freshness is exact.** Every symbol has an *interface hash* and a *body hash*. A note records the
  hash it was written against; when the code changes, the note is flagged — not guessed at.
- **Unopinionated.** The engine knows languages, not architectures. "Handlers never call repos" is
  data in the book, not code in the engine.
- **Built for teams.** The book is committed to the repo as small text files, reviewed in PRs, and
  checked in CI.

## Documentation

| Doc | Contents |
|---|---|
| [01 Problem](docs/01-problem.md) | What actually happens in an agent session, with evidence |
| [02 Concepts](docs/02-concepts.md) | Book and engine, kinds of knowledge, views, evidence, tasks |
| [03 Data model](docs/03-data-model.md) | Entities, hashing, edges, features, entries, on-disk text format |
| [04 Engine](docs/04-engine.md) | Operations, guards, brief assembly, freshness, scope check |
| [05 Storage](docs/05-storage.md) | The local store: pager, copy-on-write B+tree, event log, crash safety |
| [06 Ingest](docs/06-ingest.md) | Parsing, symbol hashing, edges, incremental updates |
| [07 Workflows](docs/07-workflows.md) | Skills, hooks, compaction, bootstrapping an existing codebase |
| [08 Evaluation](docs/08-evaluation.md) | How we prove it helps (or find out it doesn't) |
| [09 Roadmap](docs/09-roadmap.md) | Milestones M0–M7 |
| [10 Decisions](docs/10-decisions.md) | Decisions made so far, with reasons |
| [11 Open questions](docs/11-open-questions.md) | What is still undecided |
| [12 Prior art](docs/12-prior-art.md) | Existing tools and how Inklore differs |

## How it is built

Inklore is built by coding agents from written task prompts, each reviewed before the next begins.
The prompts, the agents' reports and the reviews are all in [tasks/](tasks/README.md).

## Tech

Rust. First languages indexed: Rust, Go, TypeScript/JavaScript. License: MIT OR Apache-2.0.
