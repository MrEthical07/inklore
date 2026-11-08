# 03 — Data model

This document defines what the book stores. [05](05-storage.md) covers how the local store encodes it;
this document covers the logical model and the committed text format.

## What is committed and what is local

| Data | Committed (`.inklore/`) | Local (`.inklore/local/`, gitignored) |
|---|---|---|
| Config | ✅ `config.toml` | |
| Features (name + seeds) | ✅ `features/*.toml` | computed membership |
| Knowledge entries | ✅ `entries/*.md` | indexed copy + computed freshness |
| Code graph (entities, edges, hashes) | | ✅ derived from code |
| Snapshots of code bodies (for diffs) | | ✅ content-addressed blobs |
| Tasks and event logs | | ✅ private working state |

Rules of thumb:

- **Committed = assertions made by people or agents.** Small text files, one per item, so different
  people's additions never conflict in git.
- **Local = anything computable** from the code plus the committed text. It can be deleted and rebuilt.
- **Freshness is never committed.** An entry stores the hash it was written against; whether that still
  matches is computed locally against whatever code is checked out. Switching branches just works.

## Identifiers

| Thing | ID format | Example |
|---|---|---|
| Entity (file/dir) | `path` relative to repo root, `/`-separated | `internal/modules/users/handler.go` |
| Entity (symbol) | `lang:path#qualified.name` | `go:internal/modules/users/handler.go#Handler.Create` |
| Feature | slug | `user-invites` |
| Entry | ULID (sortable, merge-safe); short form = last 8 chars | `01JBX9W8Q4M2T7K3ZJ5R6N0P1C` / `R6N0P1C` |
| Task | ULID | |

When a SCIP index is available, its symbol strings are recorded as an alias for precise resolution.
Overloaded or anonymous symbols get a disambiguating suffix (`#fn@L42` for closures).

## Entities

```text
Entity {
  id          : EntityId
  kind        : Dir | File | Symbol | Artifact
  sym_kind?   : Function | Method | Type | Interface | Trait | Const | Var | Module   (symbols only)
  lang?       : rust | go | ts | js
  path        : repo-relative path
  range?      : (start_line, end_line)                     (symbols)
  content_hash: blake3(raw bytes)                          (files, artifacts)
  iface_hash  : structural hash of the public shape
  body_hash   : structural hash of the whole normalised node
  merkle_hash : hash over children                         (dirs, files)
  exported    : bool
}
```

`Artifact` covers non-code files that knowledge may need to pin to — config, SQL migrations, OpenAPI
specs, `.env.example`. v0 tracks them at file level with `content_hash` only.

## Hashing

All hashes are BLAKE3, truncated to 128 bits, written as 32 hex characters in text.

**Normalisation** (for `iface_hash` and `body_hash`): parse with tree-sitter, then hash the sequence of
`(node_kind, token_text)` for leaf tokens, **skipping comments and whitespace**. Formatting changes and
comment edits therefore never invalidate knowledge.

**Interface hash** — what callers depend on:

| Symbol kind | Included |
|---|---|
| function / method | name, receiver/self type, parameter types, return types, generics, visibility, `async` |
| struct / class | name, generics, field names and types, visibility |
| interface / trait | name, method signatures |
| file | sorted interface hashes of its exported symbols |

**Body hash** — the entire normalised node, including the interface.

**Merkle hash** — `H(sorted list of (child_name, child_hash))` where a file's child hash is its
symbols' body hashes and a dir's is its children's Merkle hashes. A feature's Merkle hash is over its
member symbols' body hashes.

**Moves.** During one ingest, if a symbol disappears and a symbol with the same `body_hash` appears
elsewhere, the engine records a `moved` event and re-anchors entries automatically when the match is
unique. Ambiguous matches are reported, not guessed.

## Edges

```text
Edge {
  from, to   : EntityId
  kind       : Contains | Calls | Implements | Imports | References | Tests
  confidence : Exact | Heuristic
  site?      : (path, line)        // where the edge occurs, for path:line:content output
}
```

- `Exact` edges come from SCIP indexes (compiler-backed). `Heuristic` edges come from name resolution
  on tree-sitter output. Briefs mark heuristic links so the agent knows they may be wrong.
- `Implements` matters for interface-heavy designs (ports and adapters): a call to an interface method
  reaches its implementations only through these edges.
- `Tests` links a test function to the symbols it exercises (heuristic in v0: calls from test files).

## Features

Committed as `.inklore/features/<slug>.toml`:

```toml
slug        = "user-invites"
name        = "User invites"
description = "Invite a user to a tenant by email and accept the invite."
status      = "live"                  # live | planned | retired
seeds       = [
  "go:internal/modules/invites/handler.go#Handler.Create",
  "go:internal/modules/invites/handler.go#Handler.Accept",
]
exclude     = ["internal/platform/logging/**"]   # optional: never traverse into these
```

**Membership** (local, computed): breadth-first from seeds along `Calls`, `Implements`, `References`,
up to `max_depth` (default 6). A node reached by features ≥ `shared_threshold` (default 3) is marked
**shared**: included, but traversal does not continue past it. `exclude` globs and the config's global
excludes (vendored and generated code) are never entered.

A `planned` feature may have seeds that do not exist yet; when a matching symbol appears, it binds.

## Entries

One file per entry: `.inklore/entries/<ULID>.md`. TOML front matter between `+++` lines, then a
Markdown body.

```markdown
+++
id        = "01JBX9W8Q4M2T7K3ZJ5R6N0P1C"
kind      = "gotcha"
title     = "Invite tokens must be compared in constant time"
scope     = "feature:user-invites"
evidence  = "asserted"
author    = "agent"                     # agent | human; humans also attributed via git
session   = "01JBX9V…"                  # task that wrote it
created   = 2026-11-02T10:15:00Z
supersedes = []                         # entry IDs this one replaces
tags      = ["security"]

[[anchors]]
entity = "go:internal/modules/invites/service.go#Service.Accept"
on     = "body"                         # iface | body | file | exists
hash   = "9f2c4e1a0b7d3c58e6a1f40d2b9c7e13"
lines  = [41, 58]                       # lines the claim cites, at write time
+++

`Accept` compares the presented token with `subtle.ConstantTimeCompare`. A plain `==` here
reintroduces a timing side channel that the security review flagged in #212.
```

### Common fields

| Field | Required | Notes |
|---|---|---|
| `id`, `kind`, `title`, `scope`, `evidence`, `author`, `created` | yes | `title` ≤ 100 chars, one line |
| body | yes | ≤ `max_body_chars` (default 1200) |
| `anchors` | per kind | see below |
| `supersedes` | no | superseded entries stay in history, hidden from briefs |
| `status` | no | `active` (default), `retired` (with `retired_reason`) |
| `tags` | no | free-form |

`scope` is one of `root`, `path:<glob>`, `feature:<slug>`, `entity:<id>`.

### Kind-specific fields

| Kind | Extra fields | Anchors |
|---|---|---|
| `purpose` | — | ≥1, `on = "exists"` |
| `summary` | `of_hash` (the Merkle hash it summarises) | the summarised entity |
| `behavior` | — | ≥1, usually `on = "body"` |
| `contract` | `test?` (entity ID of a test) | ≥1, usually `on = "iface"` |
| `decision` | `rationale` (required), `alternatives?` | optional |
| `rule` | `key?` (for overriding), `severity` = `must` \| `should`, `applies_to` (glob) | optional |
| `gotcha` | — | ≥1 |
| `plan` | `steps` = [{ text, status = todo \| doing \| done \| dropped }], `owner_task?` | optional |
| `todo` | — | optional |

## Tasks (local only)

```text
Task {
  id, goal, declared_scope: [Scope], status: Active | Finished | Abandoned
  started_at, start_snapshot: SnapshotId          // store snapshot for the scope check
  plan_steps: [Step]
  decisions:  [DecisionDraft]                     // promoted to entries on finish
  open_questions: [String]
  injected: Set<EntryId | EntityId>               // what has already been pushed this session
}

Event {                                           // appended to the task's event log
  task, seq, at, kind: Read | Edit | Write | Command | Error | Note
  target?: path or entity, detail: String         // exact text: commands, error messages, line ranges
}
```

## Config

`.inklore/config.toml`:

```toml
[project]
languages = ["go", "ts"]
exclude   = ["vendor/**", "**/*.pb.go", "dist/**"]

[brief]
default_budget_tokens = 2500
max_body_chars        = 1200
entries_per_scope_cap = 40

[features]
max_depth        = 6
shared_threshold = 3

[scip]                                  # optional precise indexes
go = "scip-go"                          # command to run, or path to an index.scip
ts = "scip-typescript index"

[trust]
default_branch    = "main"              # entries unchanged from here are `trusted` (D21)
show_unreviewed   = true                # show new/changed entries, labelled `unreviewed`
enforce_unreviewed_rules = false        # never treat `must` rules from unreviewed entries as mandatory
```
