//! The `scripted` agent: a fake that performs a fixed list of file actions per task. For tests.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::{Agent, AgentExit, AgentMetrics, AgentOutcome, AgentRequest};
use crate::config::{ScriptAction, safe_join};
use crate::error::EvalError;

/// A fake agent that performs scripted actions and reports synthetic metrics.
#[derive(Debug, Clone)]
pub struct ScriptedAgent {
    name: String,
    tasks: BTreeMap<String, Vec<ScriptAction>>,
}

impl ScriptedAgent {
    /// Create a scripted agent with the given actions per task id. Tasks without an entry do nothing.
    pub fn new(name: impl Into<String>, tasks: BTreeMap<String, Vec<ScriptAction>>) -> Self {
        Self {
            name: name.into(),
            tasks,
        }
    }
}

impl Agent for ScriptedAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, req: &AgentRequest<'_>) -> Result<AgentOutcome, EvalError> {
        let actions = self.tasks.get(req.task_id).map_or(&[][..], Vec::as_slice);
        let started = Instant::now();
        let mut exit = AgentExit::Completed;
        for action in actions {
            if let Some(early) = perform(action, req, started)? {
                exit = early;
                break;
            }
        }
        Ok(AgentOutcome {
            exit,
            metrics: synthetic_metrics(actions.len()),
            agent_version: Some("scripted-1".to_string()),
        })
    }
}

/// Perform one action; `Some(exit)` ends the run early.
fn perform(
    action: &ScriptAction,
    req: &AgentRequest<'_>,
    started: Instant,
) -> Result<Option<AgentExit>, EvalError> {
    match action {
        ScriptAction::WriteFile { path, content } => {
            let target = safe_join(req.workdir, path)?;
            make_parent(&target)?;
            fs::write(&target, content).map_err(|e| EvalError::io(format!("write {path}"), e))?;
        }
        ScriptAction::AppendFile { path, content } => {
            let target = safe_join(req.workdir, path)?;
            make_parent(&target)?;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target)
                .map_err(|e| EvalError::io(format!("open {path}"), e))?;
            file.write_all(content.as_bytes())
                .map_err(|e| EvalError::io(format!("append {path}"), e))?;
        }
        ScriptAction::DeleteFile { path } => {
            let target = safe_join(req.workdir, path)?;
            fs::remove_file(&target).map_err(|e| EvalError::io(format!("delete {path}"), e))?;
        }
        ScriptAction::Sleep { secs } => {
            let wanted = Duration::from_secs(*secs);
            let remaining = req.timeout.saturating_sub(started.elapsed());
            if wanted > remaining {
                thread::sleep(remaining);
                return Ok(Some(AgentExit::TimedOut));
            }
            thread::sleep(wanted);
        }
        ScriptAction::Fail { message } => {
            return Ok(Some(AgentExit::Failed {
                message: message.clone(),
            }));
        }
    }
    Ok(None)
}

fn make_parent(target: &std::path::Path) -> Result<(), EvalError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| EvalError::io(format!("create {}", parent.display()), e))?;
    }
    Ok(())
}

/// Deterministic fake numbers that grow with the number of actions. `cache_write_tokens` and
/// `compactions` stay `None` on purpose, so `None` handling is exercised end to end.
fn synthetic_metrics(actions: usize) -> AgentMetrics {
    let turns = u32::try_from(actions).unwrap_or(u32::MAX).saturating_add(1);
    let n = u64::from(turns);
    AgentMetrics {
        turns: Some(turns),
        input_tokens: Some(1000 * n),
        cache_read_tokens: Some(500 * n),
        cache_write_tokens: None,
        output_tokens: Some(100 * n),
        cost_usd: Some(0.001 * f64::from(turns)),
        compactions: None,
        wall_time: Duration::ZERO,
    }
}
