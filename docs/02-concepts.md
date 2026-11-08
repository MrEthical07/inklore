# 02 — Concepts

## Book and engine

- **The book** is the collection of every fact and observation about a codebase.
- **The engine** is the only way to read from or write to it. Agents never touch the book directly.

The engine validates writes (guards), tells callers what a write requires, computes freshness, and
assembles answers sized to a token budget. Because everything passes through it, the book stays
consistent and agents never have to guess how to use it.

The book has two parts with different sources of truth:

| Part | Contents | Source of truth | Where it lives |
|---|---|---|---|
| **Structure** | directories, files, symbols, edges between them | the code (computed by the engine) | local store, rebuilt any time |
| **Knowledge** | rules, decisions, contracts, gotchas, plans, summaries… | people and agents, through the engine | `.inklore/` text files, committed |

A third, private part — **working state** of in-progress tasks — lives only in the local store.

## Unopinionated by design

The engine knows **languages** (through parser packs), never **architectures**. It has no idea what a
"handler", "repository" or "hexagonal layer" is. Those concepts enter the book as *data*:

- a rule entry scoped to `internal/modules/**`: "handlers never call repositories directly"
- a decision entry at the root: "we use a layered hexagonal architecture because …"
- a feature declared by its seed symbol

If a project changes its architecture, it edits its book. The engine is unchanged. The same engine
serves a Go API template, a Django monolith, or a React app.

## Kinds of knowledge

Each kind has its own source and goes out of date in its own way, so each is a distinct entry type.

| Kind | Answers | Typical source | Goes stale when |
|---|---|---|---|
| `purpose` | Why does this exist / which feature does it serve? | people, agents | anchor deleted or moved |
| `summary` | What is in here, in a few lines? (files, dirs, features) | engine-requested, agent-written | Merkle hash of the subtree changes |
| `behavior` | What does it actually do? | agents (from code and tests) | body hash changes |
| `contract` | What *should* it do? Invariants, guarantees | people, tests, specs | interface hash changes, or linked test changes/fails |
| `decision` | Why was it done this way? What was rejected? | people, agents, commit history | never wrong; can be **superseded** |
| `rule` | How must code here be written? | people | only when edited |
| `gotcha` | What will bite you here? | agents, from experience | anchored code changes |
| `plan` | What change is in progress, and how far along? | agents, people | **completed or abandoned** |
| `todo` | A known gap to address later | agents, people | resolved or retired |

Two properties fall out of this split:

- **Intent vs behavior.** Because `contract` (should) and `behavior` (does) are separate, the engine can
  surface mismatches — often a bug.
- **Plans go stale in reverse.** A note goes stale when its code changes. A plan goes stale when its code
  *doesn't* change toward it, and is resolved when it does.

## Views of the code

One structure cannot answer every question, so the book holds **one graph with several views**, linked
to each other. Knowledge can attach to an entity in any view.

| View | Shape | Answers |
|---|---|---|
| **Containment** | strict tree: repo → dir → file → symbol | What is this file / folder for? |
| **Dependencies** | graph: symbol → symbol (calls, implements, imports, references) | What does this call? What depends on it? |
| **Features** | many-to-many: feature ↔ symbols and files | Everything involved in *invites* |

**Features are declared small and computed large.** A person or agent declares only a *seed* ("feature
*invites* starts at `InviteHandler.Create`"). The engine computes membership: everything reachable from
the seeds along dependency edges. Seeds are cheap to verify; the closure is never wrong relative to the
graph.

**Shared code is explicit, not confusing.** A utility reached by two features belongs to both. Its own
entries describe what it does; its "purpose" is *computed* from which features reach it. A brief for
either feature says: "shared with *billing* — changing it affects them."

## Scopes and inheritance

Every entry has a **home**: the scope it belongs to (the repo root, a path, a feature, a file, a symbol).
Entries apply to everything below their home, like `.gitignore` or CSS:

- a repo-wide decision lives at the root and reaches every brief
- a rule at `internal/modules/**` applies to every module; a rule at `internal/modules/billing/**` can
  add to it or override it (by sharing a rule `key`)
- a feature decision reaches only that feature's brief

Walking from any entity up to the root collects exactly the rules and decisions that apply to it —
nothing more, nothing missing. An entry pinned to several entities is homed at their lowest common
ancestor, never copied.

## Evidence instead of "confirmed by"

Nobody reviews every agent action, so "confirmed by a human" would be empty or false. Instead each entry
records **how it was checked**:

| Level | Meaning |
|---|---|
| `derived` | Computed from code by the engine. Always true; recomputed. |
| `test` | Linked to a test that passes. Re-checked automatically when code changes. |
| `cross_checked` | A second agent, with a fresh context, checked the claim against the cited lines. |
| `asserted` | Written by an agent with anchors but no further evidence. |
| `human` | Written or edited by a person (attributed through git). |

Briefs show the level next to each claim so the agent knows how far to trust it.

## Freshness

Every symbol has two structural hashes (see [03](03-data-model.md)):

- an **interface hash** — its signature or public shape
- a **body hash** — its whole normalised implementation

An entry anchors to code *and the hash it was written against*, and says which hash matters
(`on = "iface" | "body" | "file" | "exists"`). A contract survives a body rewrite; a behavior note does
not. Freshness is **computed, never stored**: `fresh`, `body_changed`, `iface_changed`, `moved`
(followed automatically when unambiguous), `deleted`.

Directories and features get **Merkle hashes** built from their children, so a change invalidates only
the summaries on the path from the change up to the root.

## Tasks

A **task** is a unit of agent work held by the engine, not by the context window:

- goal and **declared scope** (features or paths)
- plan steps with status
- decisions taken, open questions
- an event log captured automatically from tool calls (files read, edits, commands, errors)

On compaction, the task is already saved; the engine re-injects goal, plan, recent decisions and rules.
On finish, durable parts are **promoted** to the book (a decision, a gotcha, a plan update).

## Scope check

When a task finishes, the engine compares hashes against the snapshot taken when it started. Changes
are classified:

- **in scope** — inside the declared scope
- **expected ripple** — outside it but connected through dependency edges
- **out of scope** — unrelated code changed

Out-of-scope changes are **flagged, not blocked**. The agent must give a reason, which is recorded as a
decision. This is the review that would otherwise not happen.

## The brief

The main output. One push, sized to a token budget, for whatever is being worked on:

purpose · entry points · the dependency slice (relevant symbols only, as `path:line`) · contracts ·
inherited rules and decisions · gotchas · open plans · shared-with impact · recent changes ·
stale warnings · coverage (what is *not* known).

Details are loaded on request: the brief lists entry titles and IDs; the agent expands what it needs.
