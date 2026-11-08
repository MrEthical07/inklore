# 11 — Open questions

Undecided points, roughly in the order they will block work.

## Needed before M2 (ingest)

1. **Local variable renames.** Should the body hash ignore local identifier names (alpha-normalisation),
   so renaming a loop variable doesn't stale behavior notes? More robust, but more work per language and
   occasionally hides meaningful renames.
2. **Symbol identity for overloads and closures.** Is `#name@L42` acceptable for anonymous functions
   given that line numbers shift? Alternatives: enclosing symbol + ordinal.

## Needed before M3 (book)

3. **Multi-anchor staleness policy.** An entry about a flow anchors to five symbols; one changes. Worst
   state (current default), or per-anchor weight, or "stale only if a `primary` anchor changes"?
4. **Entry body size.** Is 1,200 characters right? Too small forces splitting; too large defeats briefs.
5. **Human edits to entries.** When a person edits an entry's text by hand without re-anchoring, does
   `evidence` become `human`, and do anchors keep their old hashes?

## Needed before M4 (reading)

6. **Feature closure defaults.** `max_depth = 6` and `shared_threshold = 3` are guesses; tune on real
   repos during eval.
7. **`find` without embeddings.** v0 plans lexical matching (identifier tokens, titles, feature names)
   plus graph proximity. Is that enough, or do we need a small local embedding model?
8. **Brief format.** Markdown is assumed; would a tighter structured format save tokens without hurting
   comprehension? Measure in eval.

## Needed before M5 (tasks)

9. **Agents other than Claude Code.** Which hook equivalents exist in Codex CLI, Gemini CLI, Cursor,
   OpenCode? Where none exist, MCP plus instructions only.
10. **Concurrent agents on one repo.** Two agents in separate worktrees share the committed book but each
    has its own local store. Should tasks declare scopes visible to each other (claims) to avoid
    collisions? Likely post-v0.

## Team and trust

11. ~~**Trust policy for entries.**~~ Resolved by D21 (trust levels from git review; signed manifest
    and private entries later). Still open within it: the exact patterns for the CI injection lint, and
    which signing format to use for the manifest (SSH signatures via `ssh-keygen -Y`, or minisign).
12. **Summaries in the repo.** Committed summaries save every teammate the tokens to regenerate them, but
    they are LLM output and add PR noise. Commit them, keep them local, or regenerate in CI?
13. **Monorepos.** One `.inklore/` at the root or one per package? Cross-package features?

## Project

14. **Repository home.** GitHub user or an organisation named `inklore`?
15. **Release channels.** `cargo install` only for v0.1, or also Homebrew/Scoop/npm wrapper?
