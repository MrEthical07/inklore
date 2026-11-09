//! The on-disk result records: one JSON object per task execution in `runs.jsonl`.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent::{AgentExit, AgentMetrics};
use crate::error::EvalError;

/// File name of the per-task results inside a batch directory.
pub const RUNS_FILE: &str = "runs.jsonl";
/// File name of the generated report inside a batch directory.
pub const REPORT_FILE: &str = "report.md";

/// The outcome of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRecord {
    /// The check's name.
    pub name: String,
    /// Whether it passed.
    pub passed: bool,
    /// How long it took, in milliseconds.
    pub duration_ms: u64,
    /// The last (up to 50) lines of its output.
    pub output_tail: Vec<String>,
}

/// Everything recorded about one task execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    /// `<chain>__<arm>__<agent>__r<N>__<unix-seconds>`.
    pub run_id: String,
    /// Chain id.
    pub chain: String,
    /// Arm name.
    pub arm: String,
    /// Agent name.
    pub agent: String,
    /// Repeat number, starting at 1.
    pub run_number: u32,
    /// Task id.
    pub task_id: String,
    /// `true` when the agent completed and every check passed.
    pub passed: bool,
    /// How the agent run ended.
    pub agent_exit: AgentExit,
    /// What the agent reported plus the measured wall time.
    pub metrics: AgentMetrics,
    /// The agent's version, if it reported one.
    pub agent_version: Option<String>,
    /// Every check, in order.
    pub checks: Vec<CheckRecord>,
    /// Paths changed by this task, relative to the previous task's state.
    pub changed_paths: Vec<String>,
    /// `git diff --stat` of those changes.
    pub diff_stat: String,
    /// Version of this harness.
    pub harness_version: String,
    /// The commit the chain started from.
    pub base_commit: String,
    /// Operating system the harness ran on.
    pub os: String,
    /// Unix seconds when the task started.
    pub started_unix: u64,
}

/// Appends records to `runs.jsonl`, one flushed line at a time, so a crash loses nothing finished.
#[derive(Debug)]
pub struct ResultsWriter {
    file: File,
    path: PathBuf,
}

impl ResultsWriter {
    /// Open (creating if needed) `runs.jsonl` inside `batch_dir`.
    pub fn open(batch_dir: &Path) -> Result<Self, EvalError> {
        let path = batch_dir.join(RUNS_FILE);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| EvalError::io(format!("open {}", path.display()), e))?;
        Ok(Self { file, path })
    }

    /// Append one record.
    pub fn write(&mut self, record: &TaskRecord) -> Result<(), EvalError> {
        let mut line = serde_json::to_string(record).map_err(|e| EvalError::Results {
            file: self.path.display().to_string(),
            line: 0,
            message: e.to_string(),
        })?;
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .and_then(|()| self.file.flush())
            .map_err(|e| EvalError::io(format!("write {}", self.path.display()), e))
    }
}

/// Read every record from a `runs.jsonl` file.
pub fn read_records(path: &Path) -> Result<Vec<TaskRecord>, EvalError> {
    let text = fs::read_to_string(path)
        .map_err(|e| EvalError::io(format!("read {}", path.display()), e))?;
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(line).map_err(|e| EvalError::Results {
            file: path.display().to_string(),
            line: index + 1,
            message: e.to_string(),
        })?;
        records.push(record);
    }
    Ok(records)
}
