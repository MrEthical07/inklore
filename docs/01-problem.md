# 01 — The problem

**Agents lose knowledge about the codebases they work on.** This document describes what actually
happens in a session, why it happens, and what the evidence says. Every design choice in Inklore
should trace back to something here.

## What happens in a session

1. The session starts with a task brief. Sometimes it names the files to change; often it doesn't.
2. The agent reads those files, then convention and architecture docs, then whatever the code points
   to.
3. Good design principles (layered, hexagonal, modular) spread one feature across many files: a
   route, a handler, a DTO, a core service, a repository. A bug can originate in any of them, so the
   agent walks the whole chain — reading **whole files** when it needs a few functions.
4. Everything it reads stays in context, relevant or not. As the window fills, quality drops.
5. When the window is full, the harness **compacts**: an LLM summarises the history. Exact details,
   the plan, and rules given mid-session are paraphrased or dropped.
6. The task ends. The agent understood a lot about the code. **All of it is thrown away.**
7. The next task, in a new session, on a different part of the code, starts again from step 1.

## Three failures, one root cause

| Failure | Where it hurts |
|---|---|
| **Exploration cost** — reading far more than needed to find what matters | tokens, time, distraction |
| **In-session decay** — context rot as the window fills, then lossy compaction | quality, lost plans and rules |
| **Cross-session amnesia** — every session re-learns; old notes silently go stale | tokens, repeated mistakes, misleading memory |

**Root cause:** the context window is the agent's only memory. It is append-only, unstructured (raw
file text rather than understanding), ephemeral (dies with the session), and lossy when compressed.
Hand-written `.md` pointer files try to fix this but nothing keeps them in sync with the code.

## Evidence

**Agents read far more than they need, and precision matters most.** In SWE-Explore, ground-truth
reading requirements averaged 4.3 files per issue while fixes touched 1.4. Claude Code found the
right files 67% of the time but recalled only 15% of the right lines. *Context efficiency* (share of
read lines that were relevant) predicted resolve rate with r = 0.95 — the strongest predictor tested.
[SWE-Explore](https://mllog.dev/en/posts/swe-explore-benchmarking-coding-agents-repository-exploration/)

**Input tokens drive cost, and much of it is repetition.** Exploration is ~30% of rounds; input (mostly
cache reads of the growing history) dominates cost; for several models about half of all file actions
are repeats on a file already seen; runs on the same problem vary up to 30× in tokens.
[arXiv 2604.22750](https://arxiv.org/html/2604.22750v2)

**More context makes models worse.** Across 18 models, performance falls as input grows; similar-but-
irrelevant content (distractors) hurts most; a focused ~300-token prompt beat the full ~113k-token
version for every model family. [Chroma — Context Rot](https://trychroma.com/research/context-rot)

**Compaction loses details and progress.** LLM summaries retain at most ~16% of recurring details;
mechanical extraction retains ~79%. Losing task *progress* is distinct from losing information, and
some tasks fail under every summariser.
[Compact-Bench](https://arxiv.org/abs/2610.31710)

**Compaction drops rules.** Policy violations went from 0% with full context to ~30% after compaction
(up to 59% for some models), and back to 0% when constraints were pinned outside the summarised history.
[ConstraintRot / constraint pinning](https://codex.danielvaughan.com/2026/07/03/governance-decay-self-compacting-agents-context-compaction-safety-constraints-codex-cli-constraint-pinning/)

**Storing instead of summarising works better.** Replacing large tool results with references to a
store kept grounding at 92% after two compactions versus 74% for summarisation.
[Lossless compaction](https://joshtuddenham.dev/blog/compaction/)

**Memory helps only when it is right.** Supplying the *correct* summary of related past work raised
resolution from 26% to 34%; letting the agent pick from a pool dropped it to 22% — below no memory.
[SWE-ContextBench](https://arxiv.org/html/2602.08316)
Retrieving more memories can lower useful-memory precision; superseded facts are a trap.
[DreamBench-SWE](https://pith.science/paper/2608.20664)
Frontier models resolve conflicts between old memory and new observations only ~55% of the time.
[TianPan — stale memory](https://tianpan.co/blog/2026/05/17/agent-memorized-bug-stale-workaround)

**Agents ignore tools they are not trained on.** For localisation, models chose an LSP tool over grep
0–6% of the time; returning `path:line:content` instead of bare locations cut follow-up reads sharply.
[AgentConnect](https://www.agentconnect.md/blog/grep-beat-lsp-harness/)
Per-search token savings often fail to become per-session savings; tool schemas alone cost ~875
tokens per turn in one test.
[MCP vs grep](https://dev.to/pranav_raj_dae81effb8b57d/mcp-retrieval-cost-4x-more-tokens-than-grep-until-repo-size-flipped-it-5cfj)

## What this means for the design

| Finding | Design consequence |
|---|---|
| Precision predicts success | Deliver small, exact slices (symbols, `path:line`), never whole files by default |
| Wrong memory is worse than none | Freshness must be exact; stale knowledge is marked, unknown is shown as unknown |
| Compaction loses progress and rules | Task state and rules live outside the context window and are re-injected |
| Mechanical capture beats summaries | Record facts from tool calls automatically; agents write only intent and decisions |
| Agents ignore optional tools | Push context through hooks; keep the tool surface tiny |
| Token savings per call ≠ per session | Evaluate end-to-end, multi-session, against a baseline |

## Problem statement

> Give coding agents a memory outside the context window that is **structured**, **provably current**,
> **shared by the team**, and delivered **within a token budget** for exactly the work at hand.
