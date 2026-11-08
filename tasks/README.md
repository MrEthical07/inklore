# How Inklore is built

Inklore is built by coding agents working from written task prompts, with every task reviewed before
the next one starts. The prompts, reports and reviews are kept in this folder on purpose: they are the
record of how the project was built with AI.

## The loop

1. **Task.** A task file `tasks/T-NNN-<name>.md` is written: context, exact scope, specifications,
   acceptance criteria and what the report must contain. Supporting files, if any, sit next to it.
2. **Build.** The coding agent is given one line:

   > Read `AGENTS.md` and `tasks/T-NNN-<name>.md`, complete the task, and write your report to
   > `tasks/reports/T-NNN.md`.

   It works on a branch `task/T-NNN-<name>` and commits there.
3. **Review.** The reviewer reads the report and the code, builds and runs the tests independently,
   checks every acceptance criterion and the design docs, and writes `tasks/reviews/T-NNN.md` with a
   verdict:
   - **approved** → merge the branch into `main`
   - **changes requested** → a numbered list of fixes; the agent is given
     > Read `AGENTS.md` and `tasks/reviews/T-NNN.md`, make the requested changes on the same branch,
     > and append a *Revision* section to `tasks/reports/T-NNN.md`.
4. **Next.** Tasks are written one at a time, after the previous review, so each one absorbs what the
   last one taught us. [ROADMAP.md](ROADMAP.md) is the plan; it changes as we learn.

## Folders

| Path | Contents |
|---|---|
| `tasks/T-NNN-*.md` | task prompts |
| `tasks/reports/` | the coding agent's reports |
| `tasks/reviews/` | review verdicts |
| `tasks/REPORT_TEMPLATE.md` | the structure every report follows |
