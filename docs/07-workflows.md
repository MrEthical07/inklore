# 07 — Workflows

How agents and people actually use Inklore. Three layers, each doing what it is reliable at:

| Layer | Reliable at | Not reliable at |
|---|---|---|
| **Skills** (instructions loaded into the agent) | explaining *when* and *why* to do something | enforcement — an agent can skim or skip a skill |
| **Hooks** (run by the agent harness on events) | doing things every time, without the agent choosing to | judgement |
| **Engine guards** | rejecting malformed or unanchored writes | judging whether a claim is true |

Anything that must always happen is a hook or a guard. Skills explain; they never enforce.

## Hooks

Claude Code is the first target (it exposes the events below); other agents get the MCP server plus
instructions until equivalent hooks exist.

| Event | Hook action |
|---|---|
| Session start | Resume the active task if one exists (goal, plan, decisions, rules); otherwise push the repo map (root purpose, feature list, root rules). |
| Session start after compaction | Re-inject the active task's state and the `must` rules for its scope. |
| Before a file is read or edited | Push the file's brief **delta**: rules, gotchas, stale warnings, and contracts not yet injected this task. |
| After an edit, write or command | `task.log` the event mechanically: path and line ranges, the exact command, exit code, first lines of error output. |
| Before compaction | `task.checkpoint()` — nothing in flight exists only in the context window. |
| Agent stops | If a task is active and unfinished, remind the agent to `task.finish` (or leave it open intentionally). |

Every injection is deduplicated against `Task.injected`, so re-reading a file does not re-push the same
text.

## Skills

Each skill is short and points at the engine. The engine's `schema()` and guard responses carry the
details, so the skills stay small and cannot drift from the engine's actual rules.

### `inklore-task` — doing a piece of work

1. `task.start(goal, scope)`. If the scope is unclear: `map()` from the root and drill down, or
   `find(goal)`. If `find` returns `no_match`, the work is probably a new feature (below).
2. Read the brief before reading code. Read code through the slice, not whole files.
3. As things happen, record them immediately — not at the end:
   - a decision → `task.decide`
   - a plan, or a change to it → `task.plan` / `task.step`
   - something that would bite the next person → draft a `gotcha`
4. `task.finish`: resolve out-of-scope flags, promote drafts to entries, mark plan steps, and refresh or
   retire stale entries in scope that this task proved wrong.

### New feature (no matching code yet)

1. `map()` → pick the most similar existing feature.
2. `brief(feature, mode = "shape")`: its file roles, how it is registered and wired, how it is tested —
   without its code.
3. `feature(define, status = "planned", seeds = [the symbols you intend to create])` and attach a `plan`.
   Rules and decisions for the target location come through inheritance automatically.
4. Work as a normal task with the planned feature as scope. When the seed symbols appear, the feature
   binds and becomes `live`.

### `inklore-verify` — checking claims

Run by a **separate** agent with a fresh context, so it does not share the writer's assumptions.

1. Receive one claim plus only the lines it cites (from the anchors).
2. Answer per claim: `supported`, `partially supported`, or `unsupported`, citing line numbers.
3. The engine records `evidence = cross_checked` for supported claims and opens a `todo` for the rest.

The verifier is asked "does line 42 support this sentence?", never "does this look right?".

### `inklore-check` — keeping the book healthy

`check()` → for each stale entry in scope: re-read the cited lines, then `update` (re-anchor at the new
hash) if still true, `supersede` if partly true, or `retire` if false.

## Fighting compaction

| Kind of state | Where it lives | How it survives compaction |
|---|---|---|
| Facts about the code | the book | re-fetched on demand via briefs |
| What the agent did (reads, edits, commands, errors) | the task event log | captured mechanically by hooks |
| Intent: plan, decisions, open questions | the task | written by the agent *as they happen* |
| Rules | the book | pinned: re-injected after every compaction |

The context window becomes a **view** of state held by the engine. Losing the summary costs little,
because nothing important existed only in the summary. Mechanical capture is used wherever possible
because it keeps far more detail than summaries (see [01](01-problem.md#evidence)).

## Bootstrapping an existing codebase

Structure needs no agent: `inklore init` parses the repo and the full graph exists in minutes. What is
missing is knowledge — and it should **not** be produced all at once. A full upfront crawl of a large
codebase is exactly the token burn Inklore exists to prevent; most of it would never be read, and parts
would go stale before anyone used them.

### Stage 1 — automatic (minutes, almost no tokens)

1. `inklore init` → config, `.gitignore` for `.inklore/local/`, full ingest.
2. **Import existing docs.** An agent reads one document at a time (`CLAUDE.md`, `AGENTS.md`, ADRs,
   architecture and conventions docs, decision trackers) and splits it into entries through the guards.
   Repo-wide decisions go to `root`; module-specific ones to their path or feature. A single
   "everything" decision file becomes many scoped entries.
3. **Features.** The engine suggests seeds ([06](06-ingest.md#seed-suggestions)); a person or agent
   confirms, names and describes them.

### Stage 2 — fill in as you work (the main mechanism)

Every task records what it learns in the area it touched. The areas actually worked on fill in first.
`coverage()` shows what is known and what is blank — blank reads as *unknown*, never as *nothing to
know*.

### Stage 3 — targeted backfill (optional)

For important areas only (git history shows which files change most):

- **one subtree per agent session**, so nothing is lost to compaction mid-way
- **bottom-up**: symbols, then files, then the feature — each parent summary is built from children
  that have already been checked
- every written entry goes through `inklore-verify` before it counts as more than `asserted`

## Team use

- `.inklore/` is committed. Entries arrive in PRs like code and can be reviewed like code.
- **CI runs `inklore check`**: stale anchors, invalid seeds and guard violations in hand-edited entries
  are reported (warning by default, failing if configured).
- Entries are one file each, so parallel additions never conflict. Edits to the same entry conflict as
  small text conflicts. Plans that several agents update should prefer appending steps over rewriting
  them.
- **Trust.** Entries are data shown to agents, which makes them a prompt-injection channel in open
  repos. The book stays readable by everyone; the engine decides how far to trust each entry (D21 in
  [10](10-decisions.md)):

  | Level | When | Effect |
  |---|---|---|
  | `trusted` | entry is unchanged from the protected default branch | shown normally |
  | `unreviewed` | entry is new or changed relative to the merge base with the default branch | shown with a label; its `must` rules are not enforced |
  | `signed` (later) | entry is covered by the CI-signed Merkle root of `.inklore/entries/` | verifiable in forks and offline |
  | `private` (later) | body encrypted to each team member's own public key | readable only by members |

  Even trusted entries are data, not commands: briefs wrap entry text in clear delimiters and label it
  as repository content.
- **CI checks** for the book: guards on hand-edited entries, a secret scan, an injection lint (entries
  that read as instructions to the agent — "ignore previous instructions", commands to run, URLs to
  fetch — are flagged for a human), and `CODEOWNERS` protection on `.inklore/`.
