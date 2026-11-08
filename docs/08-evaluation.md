# 08 — Evaluation

Most tools in this space report token savings per search. Nobody has published end-to-end,
multi-session results showing agents actually do better. Inklore's evaluation is part of the product:
it is built first (milestone M0) and every design decision answers to it. Negative results get published
too.

## Questions

1. Do agents **succeed more often** on multi-session work with Inklore than without?
2. Do they spend **fewer total tokens and turns** across a whole task chain — not per search?
3. Do they **break rules less** and make **fewer out-of-scope edits**?
4. Do they **recover better after compaction**?
5. Do agents **actually use** what Inklore provides?

## Task chains

The unit of evaluation is a **chain**: 3–5 related tasks on the same repository, each run in a fresh
session, where later tasks benefit from what earlier ones learned. Examples:

- add a field end-to-end, then add validation for it, then expose it in a list endpoint with filtering
- refactor an interface used by three modules, then fix a bug that the refactor reveals elsewhere
- build a new module modelled on an existing one, then add a cross-cutting rule and apply it

Each task has a hidden checker: tests that must pass plus rule checks (e.g. "no handler imports a
repository package").

**Repositories** (v0): `superapi` (Go), one mid-sized open-source TypeScript service, and Inklore itself
(Rust). At least one long task per chain is sized to force a compaction.

## Arms

| Arm | Setup |
|---|---|
| **Baseline** | the repository's existing docs (`CLAUDE.md`/`AGENTS.md`, convention files) as-is |
| **Pull** | baseline + Inklore MCP server only |
| **Push** | baseline + Inklore hooks + MCP server + skills (the intended setup) |

The same model, harness version, and budget across arms. At least 3 runs per task per arm; report
spread, not only means (single runs vary by up to 30× in tokens).

## Metrics

| Metric | How |
|---|---|
| Success | hidden checker passes |
| Total input tokens (incl. cache reads), output tokens, cost | provider-reported, summed over the chain |
| Turns, wall time | harness logs |
| Rule violations | rule checkers on the final diff |
| Out-of-scope edits | diff vs the task's intended scope |
| Post-compaction recovery | success on tasks that compacted vs those that did not |
| Adoption | share of turns where the agent used a brief or engine call; reads of files already covered by an injected brief |
| Brief precision | share of brief content that falls in the regions the solution actually needed (SWE-Explore-style context efficiency) |

## Engine benchmarks

Separate from agent evaluation, run in CI on every change: ingest throughput, incremental sync latency,
brief latency (p50/p99), store benchmarks against redb and SQLite (see [05](05-storage.md#targets-v0-on-a-laptop)).

## Harness

`eval/` in the repository: chain definitions as data (repository, commit, task prompts, checkers), a
runner that drives the agent CLI headlessly per arm, and a report generator (Markdown + JSON). Runs are
reproducible from a pinned commit.
