//! The agent interface and the two built-in adapters (`scripted` and `command`).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::command::CommandAgent;
use crate::config::{AgentDef, load_agent_def};
use crate::error::EvalError;
use crate::scripted::ScriptedAgent;

/// What the runner hands an agent for one task.
#[derive(Debug, Clone, Copy)]
pub struct AgentRequest<'a> {
    /// The task id.
    pub task_id: &'a str,
    /// The prompt text.
    pub prompt: &'a str,
    /// The working copy the agent must work in.
    pub workdir: &'a Path,
    /// How long the agent may run.
    pub timeout: Duration,
    /// Where the adapter may write transcripts and raw output.
    pub log_dir: &'a Path,
}

/// How an agent run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentExit {
    /// The agent finished by itself.
    Completed,
    /// The time limit was hit and the agent was killed.
    TimedOut,
    /// The agent failed.
    Failed {
        /// Why.
        message: String,
    },
}

/// What an adapter could measure; every field is optional except wall time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentMetrics {
    /// Conversation turns.
    #[serde(default)]
    pub turns: Option<u32>,
    /// Input tokens (not counting cache reads).
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Tokens read from the prompt cache.
    #[serde(default)]
    pub cache_read_tokens: Option<u64>,
    /// Tokens written to the prompt cache.
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// Cost in US dollars.
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// How many times the context was compacted.
    #[serde(default)]
    pub compactions: Option<u32>,
    /// Wall-clock time; always measured by the harness, serialized as milliseconds.
    #[serde(default, rename = "wall_time_ms", with = "duration_ms")]
    pub wall_time: Duration,
}

/// The result of running one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentOutcome {
    /// How the run ended.
    pub exit: AgentExit,
    /// What was measured.
    pub metrics: AgentMetrics,
    /// The agent's own version string, if it reports one.
    pub agent_version: Option<String>,
}

/// Something that can run one task in a fresh session.
pub trait Agent {
    /// The agent's name, as given on the command line.
    fn name(&self) -> &str;
    /// Run one task to completion, timeout or failure.
    fn run(&self, req: &AgentRequest<'_>) -> Result<AgentOutcome, EvalError>;
}

/// Build the agent called `name` from `eval/agents/<name>.toml`.
pub fn load_agent(eval_dir: &Path, name: &str) -> Result<Box<dyn Agent>, EvalError> {
    Ok(match load_agent_def(eval_dir, name)? {
        AgentDef::Command { argv } => Box::new(CommandAgent::new(name, argv)),
        AgentDef::Scripted { tasks } => Box::new(ScriptedAgent::new(name, tasks)),
    })
}

/// Serde helper: a [`Duration`] as whole milliseconds.
pub(crate) mod duration_ms {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(d)?))
    }
}
