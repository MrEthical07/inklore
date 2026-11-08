# 10 — Decisions

Decisions made so far, newest last. Once Inklore runs, these become `decision` entries in its own book.

| # | Decision | Why | Alternatives considered |
|---|---|---|---|
| D1 | Build a new project instead of extending RATA DB | RATA DB was a general document store with no niche; its designs (whole-file index rewrites, no crash safety) would need replacing anyway | Extend RATA DB |
| D2 | Rust | Control over memory and I/O for the store; strong ecosystem for developer tools (tree-sitter, redb, SCIP); doubles as a demonstration of building in a new language with AI | C++, Go |
| D3 | The engine is the only way in or out of the book | Keeps the book consistent; lets guards enforce structure; agents never need to guess the format | Agents read/write files directly |
| D4 | The engine is unopinionated: it knows languages, not architectures | A tool built around one architecture breaks when that architecture changes and cannot serve other projects; architecture belongs in the book as data | Built-in support for layered/hexagonal conventions |
| D5 | Structure is computed from code, never written by agents | Agents building trees is unreliable; parsing is deterministic and free | Agent-built feature trees |
| D6 | Features = declared seeds + computed membership | Seeds are small and verifiable; membership is never wrong relative to the graph | Hand-maintained member lists; fully automatic features |
| D7 | The book is shared with the team from day one: knowledge is committed to the repo as one text file per entry; everything computable lives in a local, rebuildable store | Shared knowledge is the most valuable form; one-file-per-entry makes git merges trivial; freshness is computed locally, so branches just work | Personal-only database; committing the binary store |
| D8 | Freshness is computed, never stored | Stored freshness would churn files on every code change and be wrong on other branches | Write freshness status into entries |
| D9 | Two structural hashes per symbol (interface and body), with entries choosing which one matters | Lets contracts survive implementation changes while behavior notes do not | Single content hash; file-level hashes only |
| D10 | Evidence levels instead of "confirmed by" | Nobody reviews every agent action; recording *how* something was checked stays honest | Human confirmation step |
| D11 | Push through hooks first, MCP second; keep the tool surface to about six tools | Agents rarely choose unfamiliar tools; tool schemas cost tokens on every turn | MCP-only |
| D12 | Write our own storage engine behind a `Store` trait, with redb as a second backend | The store is core learning and differentiation; the trait keeps it honest through differential tests and benchmarks and makes swapping cheap | Use redb, SQLite or sled directly |
| D13 | Copy-on-write B+tree with alternating meta pages; no write-ahead log for the main store; an append-only log for task events | Read-heavy workload, atomic commits and free snapshots (needed for the scope check); events are append-only by nature | WAL + in-place B+tree; LSM tree |
| D14 | A daemon (`inklore serve`) owns the store; CLI, hooks and MCP are clients | Hooks are short-lived processes; one owner avoids cross-process reader tracking and keeps the cache warm | Every process opens the file |
| D15 | Positioned I/O instead of mmap in v0 | Windows cannot grow a mapped file; clean error handling | mmap from the start |
| D16 | Summaries are lazy and keyed by Merkle hash | Eager whole-repo summarisation burns the tokens we exist to save | Summarise everything at init |
| D17 | First languages: Rust, Go, TypeScript/JavaScript | The maintainer's own projects and Inklore itself | Python first (benchmark coverage) |
| D18 | Evaluation harness is built before features | Token savings per call do not imply per-session gains; decisions need measurements | Evaluate at the end |
| D19 | License: MIT OR Apache-2.0 | The Rust ecosystem standard; permissive with a patent grant | MIT, Apache-2.0, AGPL-3.0 |
| D20 | Name: Inklore | Short, pronounceable, fits the book metaphor; free on crates.io and npm | Lorewell, Codicum, Apostil |
| D21 | The book stays plain text; protection comes from **trust levels**, not encryption. v0: entries on the protected default branch are *trusted*, entries new or changed relative to the merge base are *unreviewed* (shown, labelled, and their `must` rules never enforced). Later: CI signs a Merkle root of `.inklore/entries/` on every merge to main (verifiable in forks and offline); optional `private` entries whose bodies are encrypted to each member's own public key | The danger is unauthorised *writes* (prompt injection), which encryption does not address; a shared repo key gives write access to everyone who holds it, and must be public in open source; encrypted files cannot be diffed, reviewed or merged | Encrypt the whole book with a repo-specific key |
