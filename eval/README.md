# Evaluation harness

`inklore-eval` runs **task chains** against **arms** with an **agent** and writes a report. It exists to
answer one question with data: does an agent do better on multi-session work with Inklore than without?
See [docs/08-evaluation.md](../docs/08-evaluation.md) for the design and
[D18](../docs/10-decisions.md) for why it is built first.

This folder holds the *data* (repos, arms, chains, agent configs). The code is the crate
[`crates/inklore-eval`](../crates/inklore-eval). Nothing here is Inklore-specific: the harness does not
depend on any other Inklore crate.

## Concepts

| Term | Meaning |
|---|---|
| **Repo** | A source repository the chains run against, by local path or Git URL |
| **Chain** | Ordered tasks on one repo, starting from a fixed commit. Code changes carry over from task to task; agent sessions do not |
| **Task** | A prompt plus checks |
| **Check** | Decides pass/fail after a task: a command, or a built-in file check |
| **Arm** | A named setup applied to the working copy before the chain starts. Baseline adds nothing |
| **Agent** | An adapter that runs one task and returns an outcome with metrics |
| **Run** | One execution of one chain under one arm with one agent; the run number tells repeats apart |

## Layout

```text
eval/
  repos.toml                 repositories
  arms/<name>.toml           arms
  chains/<id>/chain.toml     chains (prompt files live next to it)
  agents/<name>.toml         agent configurations
  .work/                     working copies of runs        (git-ignored)
  results/<batch-id>/        runs.jsonl, report.md, logs/  (git-ignored)
```

All files are TOML. Unknown fields and unknown check kinds are **errors**.

### `repos.toml`

```toml
[repos.superapi]
source = "https://github.com/MrEthical07/superapi.git"   # or a local path
```

A relative local path is resolved against the eval directory.

### `arms/<name>.toml`

```toml
name        = "baseline"                 # must equal the file name
description = "The repository as-is, with its own AGENTS.md and docs."

# Optional: files or folders copied into the working copy before the chain starts.
[[copy]]
from = "files/push/.claude/settings.json"   # relative to this file
to   = ".claude/settings.json"              # relative to the working copy
```

Copied files are committed before task 1, so they never show up as the agent's changes.

### `chains/<id>/chain.toml`

```toml
id          = "example"                  # must equal the folder name
repo        = "superapi"
base_commit = "717ffae…"                 # full commit hash
description = "…"

[[tasks]]
id           = "t1"
prompt       = "t1.md"                   # file in this chain's folder
timeout_secs = 1800                      # agent time limit (default 1800)

  [[tasks.checks]]
  name         = "unit tests"
  kind         = "command"
  command      = ["go", "test", "./..."]   # argv array, never a shell string
  timeout_secs = 600                       # default 600

  [[tasks.checks]]
  name = "route registered"
  kind = "file_contains"                   # passes if the file exists and contains the text
  path = "internal/modules/orders/routes.go"
  text = "RegisterRoutes"

  [[tasks.checks]]
  name = "no leftover"
  kind = "file_absent"                     # passes if the path does not exist
  path = "tmp/debug.log"
```

A task **passes** when the agent completed and every check passed. Checks always run, even after an
agent timeout or failure.

### `agents/<name>.toml`

```toml
# A real program. {prompt_file} and {workdir} are substituted in every argument.
kind = "command"
argv = ["my-agent", "--prompt-file", "{prompt_file}"]
```

stdout and stderr go to `results/<batch>/logs/`. If the **last non-empty stdout line** is a JSON object, metrics
are read from it: `turns`, `input_tokens`, `cache_read_tokens`, `cache_write_tokens`, `output_tokens`,
`cost_usd`, `compactions`, `agent_version`. Missing fields are recorded as "not reported". On timeout the
whole process tree is killed.

```toml
# A scripted fake, for tests: actions per task id.
kind = "scripted"

[[tasks.t1]]
action  = "write_file"      # write_file | append_file | delete_file | sleep | fail
path    = "hello.txt"
content = "hi\n"
```

## Commands

Run from the repository root; `--eval-dir <path>` points at a different eval folder.

```text
cargo run -p inklore-eval -- validate
cargo run -p inklore-eval -- run --chain <id>... --arm <name>... --agent <name> [--runs N] [--stop-on-fail]
cargo run -p inklore-eval -- report eval/results/<batch-id>
```

* `validate` parses every repo, arm, chain and agent file and prints **all** problems, each with file
  and, where possible, line and column. Exit code 1 if there are any.
* `run` clones each repo into `eval/.work/<run-id>/repo`, checks out the chain's base commit, applies the
  arm, then runs the tasks in order. After each task it records `git diff --stat` and the changed paths
  and commits, so the next task's diff starts from there. Results are appended to `runs.jsonl` as they
  happen. `report.md` is written at the end.
* `report` regenerates `report.md` from `runs.jsonl` alone. The output is deterministic.

Run IDs are `<chain>__<arm>__<agent>__r<N>__<batch-id>`, where the batch id is the name of the batch folder
under `eval/results/` (for example `batch-1794216936`). Batch folders are unique, so run IDs are too. A
working copy is never reused: if `eval/.work/<run-id>` already exists the run stops with an error. Working
copies are kept after a run so you can inspect them; delete `eval/.work/` to reclaim the space.

## Results

`runs.jsonl` has one JSON object per task execution: run id, chain, arm, agent, run number, task id,
pass/fail, the agent's exit and metrics, each check (name, result, duration, last 50 lines of output),
changed paths, diff stat, harness version, base commit and OS.

`report.md` has one table per chain × arm × agent: per task the pass rate and, for each metric, the
`median / min / max` over runs; then chain totals (each run's tasks summed, then median / min / max across
runs). A metric no run reported shows as `–`.
