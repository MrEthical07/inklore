# 04 — Engine

The engine is the single entrance to the book. Everything — CLI, MCP server, hooks — is a thin
adapter over one Rust API (`inklore-engine`).

## Operations

Kept deliberately small: every tool exposed to an agent costs tokens on every turn.

### Reading

| Operation | Purpose |
|---|---|
| `brief(target, budget?)` | The main read. Target: `file`, `entity`, `feature`, `path`, or `task`. |
| `map(at?)` | Table of contents: one line per child (features at the root, then dirs, files, symbols). Used to drill down instead of searching. |
| `find(text)` | Candidate features/symbols for a task description, each with **why** it matched and a confidence. Returns an explicit `no_match` when nothing is good enough. |
| `deps(entity, dir, depth?)` | Callers (`in`) or callees (`out`) as `path:line:content`, with edge confidence. |
| `expand(entry_id)` | Full text of an entry (briefs list titles only). |
| `schema(kind)` | What a write of this kind requires — the engine telling the agent what it needs. |

### Writing

| Operation | Purpose |
|---|---|
| `write(entry)` | Add an entry. Validated by guards. |
| `update(id, patch)` | Edit an entry (e.g. refresh a stale anchor after re-verifying). |
| `supersede(id, entry)` | Replace an entry with a new one; the old one leaves briefs but stays in history. |
| `retire(id, reason)` | Remove from use with a reason. |
| `feature(define \| update \| retire)` | Declare a feature by name and seeds. |

### Tasks

| Operation | Purpose |
|---|---|
| `task.start(goal, scope)` | Opens a task, takes a store snapshot, returns the brief for the scope. |
| `task.plan(steps)` / `task.step(i, status)` | Record and update the plan. |
| `task.decide(title, rationale, scope?)` | Record a decision as it is made. |
| `task.log(event)` | Append an event (normally called by hooks, not the agent). |
| `task.checkpoint()` / `task.resume()` | Save and re-inject state around compaction or a new session. |
| `task.finish(promote)` | Runs the scope check, promotes drafts to entries, updates plans. |

### Maintenance

| Operation | Purpose |
|---|---|
| `sync()` | Re-ingest changed code and changed `.inklore/` text. Cheap when nothing changed. |
| `check(scope?)` | Validate the book against the code: stale anchors, missing seeds, broken references, guard violations in hand-edited files. Exit code for CI. |
| `coverage(at)` | What fraction of symbols/files under `at` have entries, by kind. |

## Guards

Every write passes through guards before touching the book. Rejections are structured so an agent can
act on them without guessing.

| Code | Trigger | What the response tells the agent |
|---|---|---|
| `E_SCHEMA` | missing or malformed field | which field, what is expected (same as `schema(kind)`) |
| `E_TOO_LONG` | title > 100 chars or body over limit | the limits |
| `E_SCOPE` | missing/invalid scope | valid scope forms; suggested scope from the anchors' common ancestor |
| `E_ANCHOR_MISSING` | anchored entity does not exist | nearest matching entity IDs |
| `E_ANCHOR_STALE` | supplied hash ≠ current hash | "you are describing an older version"; current hash; the lines to re-read |
| `E_RATIONALE` | `decision` without rationale | — |
| `E_DUPLICATE` | near-identical entry in the same scope (title+body trigram similarity ≥ 0.8) | the existing entry ID; suggest `update` or `supersede` |
| `E_BUDGET` | scope already at `entries_per_scope_cap` | the weakest entries in that scope as merge/retire candidates |
| `E_SECRET` | body matches secret patterns (keys, tokens, connection strings) | which pattern; entry not written |

**Guards check form, not truth.** "This function retries three times" passes every guard even if
false. Truth is the job of evidence levels (`test`, `cross_checked`) and of freshness.

When an agent supplies anchors without hashes, the engine fills in the *current* hashes — but only
after the agent has read those lines in this task (checked against the task's event log when hooks are
active; without hooks, the agent must supply the hashes returned by `brief`/`deps`). This stops an agent
from pinning claims to code it never looked at.

## Brief assembly

Input: a target and a token budget. Output: compact Markdown with a fixed section order. Output is
**deterministic** for the same book and code — identical briefs keep prompt caches warm.

1. **Resolve the target** to a set of entities `T`.
   - feature → seeds + computed membership
   - file → the file and its symbols
   - symbol → the symbol
   - task → union of its declared scope
2. **Collect ancestors** `A`: every containment path from `T` to the root (merged, de-duplicated) plus
   every feature that contains a member of `T`.
3. **Gather candidates.**
   - rules and decisions homed on `A` (more specific rules override by `key`)
   - `purpose` / `summary` for each level of `A` (one line each)
   - `contract`, `behavior`, `gotcha` anchored to `T`
   - open `plan` and `todo` entries on `A ∪ T`
   - **shared-with**: other features containing members of `T`
   - **recent changes**: last N commits touching `T`
   - **stale warnings**: entries on `T` whose freshness is not `fresh`, with a short diff from the
     stored body snapshot when available
   - **coverage** of `T`
4. **Pack to budget.**
   - *Mandatory* (always included, up to a cap): `must` rules, open plans in scope, stale warnings.
   - Everything else ranked by `score = kind_weight × proximity × evidence_weight × recency`, then added
     greedily by `score / tokens` until the budget is used.
   - Anything that does not fit is listed as `title (id)` so the agent can `expand` it.
5. **Code slice.** Symbols in `T` appear as signature + `path:start-end`. Bodies are included only for
   the top-ranked symbols when the budget allows, or when asked (`include_code`).

### Example (abridged)

```markdown
## Brief · feature:user-invites · 2,310 / 2,500 tokens · coverage 62% · 1 stale

Purpose: Invite a user to a tenant by email; accept with a one-time token.

### Rules (inherited)
- [must] Handlers never call repositories directly (path:internal/modules/**)  R6N0P1C
- [must] All tenant-scoped queries filter by tenant_id (root)                  K2D8L4X

### Decisions
- Invite tokens are single-use and expire after 72h (feature)                  M9Q1T3B

### Slice
- Handler.Create   internal/modules/invites/handler.go:22-61
- Service.Invite   internal/core/invites/service.go:30-88
- Repo.Insert      internal/modules/invites/repo.go:14-39
- mailer.Send      internal/platform/mail/send.go:10-44   [shared: auth, billing]

### Gotchas
- Invite tokens must be compared in constant time (Service.Accept) [asserted]  P1C7Y2W

### Open plan
- Move invite email to the job queue — 2/4 done (next: retry policy)          H4V8N6E

### Stale
- ⚠ "Accept returns 410 for expired tokens" — Service.Accept body changed (+3 −1)  Z8K2R1D

### Not covered
- Repo.Insert, mailer.Send have no behavior or contract entries.
```

## Freshness evaluation

For each anchor: compare the stored hash with the current hash of the kind named in `on`.

| Current state | Freshness |
|---|---|
| hash equal | `fresh` |
| `on = body`, body differs | `body_changed` |
| `on = iface` (or `body`), interface differs | `iface_changed` |
| entity gone, unique body-hash match elsewhere | `moved` → re-anchored automatically, reported once |
| entity gone, no match | `deleted` |

An entry with several anchors takes the **worst** state of its anchors (policy may become configurable;
see [11](11-open-questions.md)). Plans are treated separately: the engine reports a plan as *possibly
progressed* when its anchors changed since its last update, and *possibly abandoned* when nothing in
its scope changed for a configurable number of commits.

## Scope check

1. `task.start` records a store snapshot.
2. `task.finish` (or `scope_check` on demand) diffs entity hashes between that snapshot and now.
3. Each changed entity is classified:
   - **in scope** — member of the declared scope
   - **ripple** — outside the scope, but within one dependency hop of an in-scope entity
   - **out of scope** — anything else
4. Changes are attributed using the task's event log: edits made through the agent's own tools belong
   to the task; other changes (a person editing, `git pull`) are reported as **external**.
5. Out-of-scope task changes must be justified (`task.decide`) before `finish` succeeds. The engine
   flags; it does not revert.

## Interfaces

| Adapter | Role |
|---|---|
| **CLI** (`inklore`) | People and scripts: `init`, `sync`, `brief`, `map`, `check`, `coverage`, `show`, `edit`. |
| **Hooks** | The primary agent path: context is *pushed* at session start, on file access, and around compaction. See [07](07-workflows.md). |
| **MCP server** (`inklore mcp`) | The secondary agent path for explicit queries and writes. At most ~6 tools (`brief`, `map`, `find`, `deps`, `write`, `task`), each with an `action` argument where needed. |

Output to agents is always `path:line`-addressed and never whole files by default.

Hooks deduplicate per session: an entity's or entry's details are pushed once per task (tracked in
`Task.injected`); later accesses get only what changed since.
