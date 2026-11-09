//! `inklore-eval`: the evaluation harness.
//!
//! The harness runs **task chains** against **arms** with an **agent** and produces a report. A chain is
//! an ordered list of coding tasks on one repository, each run by an agent in a fresh session and each
//! followed by checks; code changes carry over from task to task, agent sessions do not.
//!
//! This crate is deliberately independent of the other Inklore crates: it must be able to measure them.
//!
//! * [`config`]: the TOML formats under `eval/` and their validation.
//! * [`agent`], [`scripted`], [`command`]: the agent interface and the two built-in adapters.
//! * [`runner`]: clones the repository, applies the arm, runs the tasks, records results.
//! * [`results`], [`report`], [`stats`]: `runs.jsonl` and the generated `report.md`.

pub mod agent;
pub mod checks;
pub mod command;
pub mod config;
pub mod error;
mod git;
mod proc;
pub mod report;
pub mod results;
pub mod runner;
pub mod scripted;
pub mod stats;

pub use agent::{Agent, AgentExit, AgentMetrics, AgentOutcome, AgentRequest, load_agent};
pub use command::CommandAgent;
pub use config::EvalConfig;
pub use error::{ConfigIssue, EvalError};
pub use report::{render_report, write_report};
pub use results::{CheckRecord, TaskRecord, read_records};
pub use runner::{BatchSummary, RunOptions, run_batch, run_batch_with};
pub use scripted::ScriptedAgent;
