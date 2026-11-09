# T-002 — Evaluation harness skeleton

**Milestone:** M0 — Foundations · **Depends on:** T-001 merged · **Branch:** `task/T-002-eval-harness-skeleton`

## Read first
- `AGENTS.md`
- `docs/08-evaluation.md` (the whole evaluation design — this task builds its machinery)
- `docs/10-decisions.md` (D18)

## Goal
Build `inklore-eval`, a command-line harness that runs **task chains** against **arms** and produces a
report. A chain is a sequence of coding tasks on one repository, each run by an agent in a fresh
session, each followed by checks. This task builds the machinery and proves it end-to-end with a
**scripted fake agent** on throwaway Git repositories. Real agents (Claude Code, Codex) and real chains
on SuperAPI come in T-003.

## Concepts

| Term | Meaning |
|---|---|
| **Repo** | A source repository the chains run against, by local path or Git URL |
| **Chain** | An ordered list of tasks on one repo starting from a fixed commit. Code changes **carry over** from task to task; agent sessions do **not** |
| **Task** | A prompt plus checks |
| **Check** | Something that decides pass/fail after a task: a command, or a built-in file check |
| **Arm** | A named setup applied to the working copy before the chain starts (e.g. files to copy in). Baseline = nothing added |
| **Agent** | An adapter that runs one task: gets the prompt and working directory, returns an outcome with metrics |
| **Run** | One execution of one chain under one arm with one agent; a run number distinguishes repeats |

## Deliverables

### 1. Crate `crates/inklore-eval`
Library + binary (`[[bin]] name = "inklore-eval"`), `publish = false`, workspace lints. It does **not**
depend on the other Inklore crates.

### 2. File formats (TOML), all under `eval/`

`eval/repos.toml`:
```toml
[repos.superapi]
source = "https://github.com/MrEthical07/superapi.git"   # or a local path
```

`eval/arms/<name>.toml`:
```toml
name        = "baseline"
description = "The repository as-is, with its own AGENTS.md and docs."
# Optional: files copied into the working copy before the chain starts (relative to this file)
[[copy]]
from = "files/push/.claude/settings.json"
to   = ".claude/settings.json"
```

`eval/chains/<id>/chain.toml` (prompts live next to it):
```toml
id          = "example"
repo        = "superapi"
base_commit = "717ffae…"          # full SHA in real chains
description = "…"

[[tasks]]
id     = "t1"
prompt = "t1.md"                 # file in this chain's folder
timeout_secs = 1800              # agent time limit for this task

  [[tasks.checks]]
  name    = "unit tests"
  kind    = "command"
  command = ["go", "test", "./..."]     # argv array, never a shell string
  timeout_secs = 600

  [[tasks.checks]]
  name = "route registered"
  kind = "file_contains"
  path = "internal/modules/orders/routes.go"
  text = "RegisterRoutes"
```

Check kinds for this task: `command` (passes on exit code 0) and `file_contains` / `file_absent`
(portable, no shell). Unknown fields and unknown kinds are **errors**, not silently ignored.

### 3. Agent adapters

```rust
pub struct AgentRequest<'a> {
    pub task_id: &'a str,
    pub prompt: &'a str,
    pub workdir: &'a Path,
    pub timeout: Duration,
    pub log_dir: &'a Path,        // where the adapter may write transcripts and raw output
}

pub struct AgentOutcome {
    pub exit: AgentExit,          // Completed | TimedOut | Failed { message }
    pub metrics: AgentMetrics,
    pub agent_version: Option<String>,
}

pub struct AgentMetrics {         // every field optional: adapters report what they can measure
    pub turns: Option<u32>,
    pub input_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub compactions: Option<u32>,
    pub wall_time: Duration,      // always measured by the harness
}

pub trait Agent {
    fn name(&self) -> &str;
    fn run(&self, req: &AgentRequest<'_>) -> Result<AgentOutcome, EvalError>;
}
```

Implement two adapters:
- **`scripted`** — for tests. Configured with a list of actions per task id: write a file, append to a
  file, delete a file, sleep (to test timeouts), fail. Returns synthetic metrics. No external process.
- **`command`** — a generic adapter: runs a configured argv template in `workdir`, substituting
  `{prompt_file}` (the prompt written to a file in `log_dir`) and `{workdir}`; enforces the timeout
  by killing the **whole process tree** (real agents spawn children): on Unix start the child in its
  own process group (`std::os::unix::process::CommandExt::process_group(0)`) and signal the group with
  `nix::sys::signal::killpg`; on Windows run `taskkill /T /F /PID <pid>` (an executable, not a shell).
  `unsafe` stays denied. Captures stdout and stderr into `log_dir`; and if stdout's last
  line is a JSON object, reads metrics from it using these field names: `turns`, `input_tokens`,
  `cache_read_tokens`, `cache_write_tokens`, `output_tokens`, `cost_usd`, `compactions`,
  `agent_version`. Missing fields stay `None`. T-003 builds real adapters on top of this.

### 4. The runner
For each requested (chain, arm, run number):
1. Create a fresh working copy under `eval/.work/<run-id>/repo`: `git clone` the repo's source, then
   `git checkout --detach <base_commit>`. Fail clearly if the commit does not exist.
2. Apply the arm (copy files).
3. For each task in order: call the agent; then run every check (with its own timeout); record
   everything. Tasks continue after a failed task unless `--stop-on-fail`.
4. After each task, record `git diff --stat` against the previous task's state and the list of changed
   paths (needed later for out-of-scope metrics), then `git add -A && git commit` inside the working
   copy so the next task's diff is measured from here. Use a fixed author/committer identity via
   environment variables so commits work on machines without a Git identity.
5. Write results as they happen, so a crash mid-run loses nothing finished.

Run IDs: `<chain>__<arm>__<agent>__r<N>__<unix-seconds>`. All subprocesses are argv arrays — no shell —
so the harness works the same on Windows, Linux and macOS. Git is invoked as the `git` executable.

### 5. Results and report
- `eval/results/<batch-id>/runs.jsonl`: one JSON object per **task execution**, with: run id, chain,
  arm, agent, run number, task id, agent outcome and metrics, each check's name, pass/fail, duration
  and the last 50 lines of its output, the changed paths, harness version, base commit, OS.
- `eval/results/<batch-id>/report.md` generated from `runs.jsonl`: for each chain × arm, per task:
  pass rate, and for each metric median / min / max over runs; then chain totals (sum of each run's
  tasks, then median / min / max across runs). Metrics that are `None` show as `–`. Deterministic
  output for the same input.
- `inklore-eval report <batch dir>` regenerates `report.md` from `runs.jsonl` alone.

### 6. CLI
```text
inklore-eval validate                         # parse every repo, arm and chain file; report all errors
inklore-eval run --chain <id>... --arm <name>... --agent <name> [--runs N] [--stop-on-fail]
inklore-eval report <batch-dir>
```
Agent configurations live in `eval/agents/<name>.toml` (`kind = "command"` with its argv template, or
`kind = "scripted"` with actions). `--eval-dir` overrides the default `eval/` location (tests use it).

### 7. Repository files
- `.gitignore`: add `eval/.work/` and `eval/results/`. (Published reports will go to `eval/reports/` in
  a later task; do not create it now.)
- `eval/README.md`: what the harness is, the file formats above, how to run `validate` and a run.
- `eval/arms/baseline.toml` and an `eval/repos.toml` with an empty `[repos]` table. **No real repos,
  chains or agent configs** — T-003 adds them.

## Dependencies you may add
`serde` (derive), `toml`, `serde_json`, `clap` (already in the workspace), `anyhow` (binary only),
`thiserror`, `wait-timeout` or equivalent for process timeouts, `nix` (Unix-only target dependency,
`signal` feature), `tempfile` (dev). Anything else: justify
in the report. No date/time crates — Unix seconds are enough.

## Tests (required)
Integration tests in `crates/inklore-eval/tests/` that build **throwaway Git repositories** in temp
directories (via the `git` executable) and run the harness against them with the `scripted` agent:
1. A two-task chain where both tasks pass; task 2's diff contains only task 2's changes.
2. A failing check marks that task failed; the next task still runs; with `--stop-on-fail` it does not.
3. A task that exceeds its timeout is recorded as `TimedOut` and the run continues.
4. The `command` adapter runs a real child process and parses metrics from its last stdout line. To
   stay portable, use the test binary itself as the child (re-invoke `std::env::current_exe()` with an
   argument that makes it print a JSON line and exit) — no shell scripts.
5. `validate` reports an unknown field, an unknown check kind, and a missing prompt file — each as a
   separate, clearly located error.
6. `report` is deterministic: the same `runs.jsonl` produces byte-identical `report.md`.

Plus unit tests for parsing and for the aggregation maths (median of even/odd counts, `None` handling).

## Out of scope
Real agents, real chains, SuperAPI, MCP, hooks, cost dashboards, parallel runs, any Inklore crate.

## Acceptance criteria
1. Definition of done in `AGENTS.md` passes.
2. All six required integration tests pass on your machine **and** in CI on all three OSes.
3. `cargo run -p inklore-eval -- validate` succeeds on the committed `eval/` folder.
4. Pull request opened per `AGENTS.md`.

## Report — task-specific questions
1. How did you kill the agent's whole process tree on timeout on Windows and on Unix? Any limitation?
2. Paste one sample line from a `runs.jsonl` produced by a test, and the `report.md` it produced.
3. What did you find awkward in the formats above that T-003 should change before real chains exist?
