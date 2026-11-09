//! The runner: clones a repository, applies an arm, runs a chain's tasks one by one, and records
//! everything as it happens.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::agent::{Agent, AgentExit, AgentOutcome, AgentRequest, load_agent};
use crate::checks::run_check;
use crate::config::{Arm, Chain, EvalConfig, TaskDef, safe_join};
use crate::error::EvalError;
use crate::git;
use crate::report::write_report;
use crate::results::{CheckRecord, ResultsWriter, TaskRecord};

/// What to run: the cross product of chains and arms, `runs` times each, with one agent.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Chain ids.
    pub chains: Vec<String>,
    /// Arm names.
    pub arms: Vec<String>,
    /// Agent name (`eval/agents/<name>.toml`).
    pub agent: String,
    /// How many times to repeat each chain × arm.
    pub runs: u32,
    /// Stop a chain after its first failed task.
    pub stop_on_fail: bool,
}

/// What a finished batch produced.
#[derive(Debug, Clone)]
pub struct BatchSummary {
    /// `eval/results/<batch-id>/`.
    pub batch_dir: PathBuf,
    /// Number of (chain, arm, run) executions.
    pub runs: usize,
    /// Number of task executions recorded.
    pub task_executions: usize,
    /// How many of those did not pass.
    pub failed_tasks: usize,
}

/// Run a batch described by `opts` against the eval directory, writing `runs.jsonl` and `report.md`.
///
/// `progress` receives one human-readable line per step; the library itself never prints.
pub fn run_batch(
    eval_dir: &Path,
    opts: &RunOptions,
    progress: &dyn Fn(&str),
) -> Result<BatchSummary, EvalError> {
    let config = EvalConfig::load(eval_dir)?;
    let agent = load_agent(eval_dir, &opts.agent)?;
    run_batch_with(&config, agent.as_ref(), opts, progress)
}

/// Like [`run_batch`], with the configuration and agent already loaded.
pub fn run_batch_with(
    config: &EvalConfig,
    agent: &dyn Agent,
    opts: &RunOptions,
    progress: &dyn Fn(&str),
) -> Result<BatchSummary, EvalError> {
    let chains = lookup(&config.chains, &opts.chains, "chain")?;
    let arms = lookup(&config.arms, &opts.arms, "arm")?;
    let batch_dir = create_batch_dir(&config.eval_dir)?;
    let mut writer = ResultsWriter::open(&batch_dir)?;
    progress(&format!("batch directory: {}", batch_dir.display()));

    let mut summary = BatchSummary {
        batch_dir: batch_dir.clone(),
        runs: 0,
        task_executions: 0,
        failed_tasks: 0,
    };
    for chain in &chains {
        for arm in &arms {
            for run_number in 1..=opts.runs.max(1) {
                let run = Run {
                    config,
                    agent,
                    chain,
                    arm,
                    run_number,
                    batch_dir: &batch_dir,
                    stop_on_fail: opts.stop_on_fail,
                };
                let (executed, failed) = run.execute(&mut writer, progress)?;
                summary.runs += 1;
                summary.task_executions += executed;
                summary.failed_tasks += failed;
            }
        }
    }
    write_report(&batch_dir)?;
    Ok(summary)
}

fn lookup<'a, T>(
    all: &'a std::collections::BTreeMap<String, T>,
    wanted: &[String],
    kind: &'static str,
) -> Result<Vec<&'a T>, EvalError> {
    wanted
        .iter()
        .map(|name| {
            all.get(name).ok_or_else(|| EvalError::Unknown {
                kind,
                name: name.clone(),
            })
        })
        .collect()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn create_batch_dir(eval_dir: &Path) -> Result<PathBuf, EvalError> {
    let results = eval_dir.join("results");
    fs::create_dir_all(&results)
        .map_err(|e| EvalError::io(format!("create {}", results.display()), e))?;
    let stamp = unix_seconds();
    for attempt in 0u32.. {
        let name = if attempt == 0 {
            format!("batch-{stamp}")
        } else {
            format!("batch-{stamp}-{attempt}")
        };
        let dir = results.join(name);
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(EvalError::io(format!("create {}", dir.display()), e)),
        }
    }
    unreachable_batch_dir()
}

/// The attempt counter above is a `u32` loop that only ends by returning; this keeps the function total
/// without a panic.
fn unreachable_batch_dir() -> Result<PathBuf, EvalError> {
    Err(EvalError::io(
        "create batch directory",
        std::io::Error::other("no free batch directory name"),
    ))
}

/// One execution of one chain under one arm.
struct Run<'a> {
    config: &'a EvalConfig,
    agent: &'a dyn Agent,
    chain: &'a Chain,
    arm: &'a Arm,
    run_number: u32,
    batch_dir: &'a Path,
    stop_on_fail: bool,
}

impl Run<'_> {
    /// Returns (task executions recorded, how many failed).
    fn execute(
        &self,
        writer: &mut ResultsWriter,
        progress: &dyn Fn(&str),
    ) -> Result<(usize, usize), EvalError> {
        let chain = &self.chain.def;
        // DECISION(T-002): the run id ends with the batch id, not the current time. Batch directories are
        // unique by construction, so run ids are too (two batches in the same second used to collide).
        let batch_id = self
            .batch_dir
            .file_name()
            .map_or_else(|| "batch".to_string(), |n| n.to_string_lossy().into_owned());
        let run_id = format!(
            "{}__{}__{}__r{}__{}",
            chain.id,
            self.arm.def.name,
            self.agent.name(),
            self.run_number,
            batch_id
        );
        progress(&format!("run {run_id}"));
        let repo_dir = self.prepare_working_copy(&run_id)?;
        apply_arm(self.arm, &repo_dir)?;

        let mut executed = 0;
        let mut failed = 0;
        for task in &chain.tasks {
            let record = self.run_task(&run_id, task, &repo_dir)?;
            progress(&format!(
                "  task {}: {}",
                record.task_id,
                if record.passed { "passed" } else { "FAILED" }
            ));
            writer.write(&record)?;
            executed += 1;
            if !record.passed {
                failed += 1;
                if self.stop_on_fail {
                    break;
                }
            }
        }
        Ok((executed, failed))
    }

    fn prepare_working_copy(&self, run_id: &str) -> Result<PathBuf, EvalError> {
        let chain = &self.chain.def;
        let work_parent = self.config.eval_dir.join(".work");
        fs::create_dir_all(&work_parent)
            .map_err(|e| EvalError::io(format!("create {}", work_parent.display()), e))?;
        // Never reuse a working copy: `create_dir` fails if the folder already exists.
        let work_root = work_parent.join(run_id);
        fs::create_dir(&work_root).map_err(|e| {
            EvalError::io(
                format!(
                    "create working copy {} (it must not exist yet)",
                    work_root.display()
                ),
                e,
            )
        })?;
        let repo_dir = work_root.join("repo");
        let source = self
            .config
            .repos
            .get(&chain.repo)
            .map(|repo| resolve_source(&self.config.eval_dir, &repo.source))
            .ok_or_else(|| EvalError::Unknown {
                kind: "repo",
                name: chain.repo.clone(),
            })?;
        git::clone(&work_root, &source, &repo_dir)?;
        if !git::commit_exists(&repo_dir, &chain.base_commit) {
            return Err(EvalError::MissingCommit {
                commit: chain.base_commit.clone(),
                repo: chain.repo.clone(),
            });
        }
        git::checkout_detached(&repo_dir, &chain.base_commit)?;
        Ok(repo_dir)
    }

    fn run_task(
        &self,
        run_id: &str,
        task: &TaskDef,
        repo_dir: &Path,
    ) -> Result<TaskRecord, EvalError> {
        let prompt_path = safe_join(&self.chain.dir, &task.prompt)?;
        let prompt = fs::read_to_string(&prompt_path)
            .map_err(|e| EvalError::io(format!("read {}", prompt_path.display()), e))?;
        let log_dir = self.batch_dir.join("logs").join(run_id).join(&task.id);
        fs::create_dir_all(&log_dir)
            .map_err(|e| EvalError::io(format!("create {}", log_dir.display()), e))?;

        let started_unix = unix_seconds();
        let request = AgentRequest {
            task_id: &task.id,
            prompt: &prompt,
            workdir: repo_dir,
            timeout: std::time::Duration::from_secs(task.timeout_secs),
            log_dir: &log_dir,
        };
        let clock = Instant::now();
        let mut outcome = self.agent.run(&request)?;
        outcome.metrics.wall_time = clock.elapsed();

        // DECISION(T-002): the diff is taken right after the agent, before the checks run, so build
        // artefacts that checks leave behind never count as the agent's changes. They are committed
        // with this task, so they do not show up in the next task's diff either.
        git::stage_all(repo_dir)?;
        let diff_stat = git::staged_diff_stat(repo_dir)?;
        let changed_paths = git::staged_changed_paths(repo_dir)?;

        let checks: Vec<CheckRecord> = task
            .checks
            .iter()
            .map(|check| run_check(check, repo_dir))
            .collect();
        git::stage_all(repo_dir)?;
        git::commit(repo_dir, &format!("eval: {}", task.id))?;

        Ok(self.record(
            run_id,
            task,
            outcome,
            checks,
            changed_paths,
            diff_stat,
            started_unix,
        ))
    }

    #[allow(clippy::too_many_arguments)] // plain data assembly; a builder would add nothing
    fn record(
        &self,
        run_id: &str,
        task: &TaskDef,
        outcome: AgentOutcome,
        checks: Vec<CheckRecord>,
        changed_paths: Vec<String>,
        diff_stat: String,
        started_unix: u64,
    ) -> TaskRecord {
        // DECISION(T-002): a task passes only if the agent completed *and* every check passed. Checks
        // always run, even after a timeout or failure, so their results are on record.
        let passed = outcome.exit == AgentExit::Completed && checks.iter().all(|c| c.passed);
        TaskRecord {
            run_id: run_id.to_string(),
            chain: self.chain.def.id.clone(),
            arm: self.arm.def.name.clone(),
            agent: self.agent.name().to_string(),
            run_number: self.run_number,
            task_id: task.id.clone(),
            passed,
            agent_exit: outcome.exit,
            metrics: outcome.metrics,
            agent_version: outcome.agent_version,
            checks,
            changed_paths,
            diff_stat,
            harness_version: env!("CARGO_PKG_VERSION").to_string(),
            base_commit: self.chain.def.base_commit.clone(),
            os: std::env::consts::OS.to_string(),
            started_unix,
        }
    }
}

/// A relative local path in `repos.toml` is resolved against the eval directory; URLs and absolute
/// paths are used as they are.
// DECISION(T-002): the task file does not say what a relative `source` is relative to.
fn resolve_source(eval_dir: &Path, source: &str) -> String {
    let remote = source.contains("://") || source.starts_with("git@");
    if remote || Path::new(source).is_absolute() {
        source.to_string()
    } else {
        eval_dir.join(source).to_string_lossy().into_owned()
    }
}

/// Copy the arm's files into the working copy and commit them, so task 1's diff starts clean.
fn apply_arm(arm: &Arm, repo_dir: &Path) -> Result<(), EvalError> {
    if arm.def.copy.is_empty() {
        return Ok(());
    }
    for rule in &arm.def.copy {
        let from = safe_join(&arm.dir, &rule.from)?;
        let to = safe_join(repo_dir, &rule.to)?;
        copy_recursively(&from, &to)?;
    }
    git::stage_all(repo_dir)?;
    git::commit(repo_dir, &format!("eval: arm {}", arm.def.name))
}

fn copy_recursively(from: &Path, to: &Path) -> Result<(), EvalError> {
    let kind = fs::symlink_metadata(from)
        .map_err(|e| EvalError::io(format!("stat {}", from.display()), e))?
        .file_type();
    if kind.is_dir() {
        fs::create_dir_all(to).map_err(|e| EvalError::io(format!("create {}", to.display()), e))?;
        let entries =
            fs::read_dir(from).map_err(|e| EvalError::io(format!("list {}", from.display()), e))?;
        for entry in entries {
            let entry = entry.map_err(|e| EvalError::io(format!("list {}", from.display()), e))?;
            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else if kind.is_file() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| EvalError::io(format!("create {}", parent.display()), e))?;
        }
        fs::copy(from, to)
            .map(drop)
            .map_err(|e| EvalError::io(format!("copy {} to {}", from.display(), to.display()), e))
    } else {
        Err(EvalError::io(
            format!("copy {}", from.display()),
            std::io::Error::other("symbolic links are not supported in arm files"),
        ))
    }
}
