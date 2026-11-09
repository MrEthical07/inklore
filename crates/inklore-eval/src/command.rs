//! The `command` agent: runs a configured argv template as a child process.
//!
//! `{prompt_file}` and `{workdir}` are substituted in every argument. stdout and stderr are saved to the
//! log directory. If the last non-empty line of stdout is a JSON object, metrics are read from it with
//! the field names `turns`, `input_tokens`, `cache_read_tokens`, `cache_write_tokens`,
//! `output_tokens`, `cost_usd`, `compactions` and `agent_version`; missing fields stay `None`.

use std::fs;
use std::process::Command;

use serde::Deserialize;

use crate::agent::{Agent, AgentExit, AgentMetrics, AgentOutcome, AgentRequest};
use crate::error::EvalError;
use crate::proc::run_with_timeout;

/// An agent that shells out (without a shell) to an external program.
#[derive(Debug, Clone)]
pub struct CommandAgent {
    name: String,
    argv: Vec<String>,
}

impl CommandAgent {
    /// Create a command agent from an argv template.
    pub fn new(name: impl Into<String>, argv: Vec<String>) -> Self {
        Self {
            name: name.into(),
            argv,
        }
    }
}

/// The JSON object an agent program may print as its last stdout line.
#[derive(Debug, Default, Deserialize)]
struct MetricsLine {
    turns: Option<u32>,
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
    compactions: Option<u32>,
    agent_version: Option<String>,
}

impl Agent for CommandAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, req: &AgentRequest<'_>) -> Result<AgentOutcome, EvalError> {
        fs::create_dir_all(req.log_dir)
            .map_err(|e| EvalError::io(format!("create {}", req.log_dir.display()), e))?;
        let prompt_file = req.log_dir.join(format!("{}.prompt.md", req.task_id));
        fs::write(&prompt_file, req.prompt)
            .map_err(|e| EvalError::io(format!("write {}", prompt_file.display()), e))?;

        let substitute = |arg: &str| {
            arg.replace("{prompt_file}", &prompt_file.to_string_lossy())
                .replace("{workdir}", &req.workdir.to_string_lossy())
        };
        let mut words = self.argv.iter().map(|arg| substitute(arg));
        let Some(program) = words.next() else {
            return Ok(failed("agent argv is empty"));
        };
        let mut cmd = Command::new(&program);
        cmd.args(words).current_dir(req.workdir);

        let output = run_with_timeout(cmd, req.timeout).map_err(|source| EvalError::Spawn {
            program: program.clone(),
            source,
        })?;
        save_log(req, "stdout", &output.stdout)?;
        save_log(req, "stderr", &output.stderr)?;

        let line = parse_metrics_line(&output.stdout);
        let exit = if output.timed_out {
            AgentExit::TimedOut
        } else {
            match output.status {
                Some(status) if status.success() => AgentExit::Completed,
                Some(status) => AgentExit::Failed {
                    message: format!("agent exited with {status}"),
                },
                None => AgentExit::Failed {
                    message: "agent ended without an exit status".to_string(),
                },
            }
        };
        Ok(AgentOutcome {
            exit,
            metrics: AgentMetrics {
                turns: line.turns,
                input_tokens: line.input_tokens,
                cache_read_tokens: line.cache_read_tokens,
                cache_write_tokens: line.cache_write_tokens,
                output_tokens: line.output_tokens,
                cost_usd: line.cost_usd,
                compactions: line.compactions,
                wall_time: std::time::Duration::ZERO,
            },
            agent_version: line.agent_version,
        })
    }
}

fn failed(message: &str) -> AgentOutcome {
    AgentOutcome {
        exit: AgentExit::Failed {
            message: message.to_string(),
        },
        metrics: AgentMetrics::default(),
        agent_version: None,
    }
}

fn save_log(req: &AgentRequest<'_>, stream: &str, bytes: &[u8]) -> Result<(), EvalError> {
    let path = req.log_dir.join(format!("{}.{stream}.log", req.task_id));
    fs::write(&path, bytes).map_err(|e| EvalError::io(format!("write {}", path.display()), e))
}

/// Read metrics from the last non-empty stdout line, if it is a JSON object.
fn parse_metrics_line(stdout: &[u8]) -> MetricsLine {
    let text = String::from_utf8_lossy(stdout);
    let Some(last) = text.lines().rev().map(str::trim).find(|l| !l.is_empty()) else {
        return MetricsLine::default();
    };
    if !last.starts_with('{') {
        return MetricsLine::default();
    }
    serde_json::from_str(last).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_and_leaves_missing_ones_none() {
        let out =
            b"working...\n{\"turns\": 7, \"output_tokens\": 42, \"agent_version\": \"1.2\"}\n";
        let m = parse_metrics_line(out);
        assert_eq!(m.turns, Some(7));
        assert_eq!(m.output_tokens, Some(42));
        assert_eq!(m.input_tokens, None);
        assert_eq!(m.agent_version.as_deref(), Some("1.2"));
    }

    #[test]
    fn ignores_non_json_and_bad_json() {
        assert!(parse_metrics_line(b"hello\n").turns.is_none());
        assert!(parse_metrics_line(b"{not json}\n").turns.is_none());
        assert!(parse_metrics_line(b"").turns.is_none());
        // A wrongly typed field makes the whole line unusable rather than half-read.
        assert!(
            parse_metrics_line(b"{\"turns\": \"many\"}\n")
                .turns
                .is_none()
        );
    }
}
