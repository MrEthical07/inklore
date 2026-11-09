//! Error types for the evaluation harness.

use std::fmt;
use std::path::PathBuf;

/// One problem found in a configuration file, with as much location as is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigIssue {
    /// The file the problem is in.
    pub file: PathBuf,
    /// 1-based line, when the problem can be pinned to one.
    pub line: Option<usize>,
    /// 1-based column, when the problem can be pinned to one.
    pub column: Option<usize>,
    /// Human-readable description.
    pub message: String,
}

impl fmt::Display for ConfigIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(column) = self.column {
                write!(f, ":{column}")?;
            }
        }
        write!(f, ": {}", self.message)
    }
}

/// Everything that can go wrong while validating, running or reporting.
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    /// A file-system operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being attempted.
        context: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// One or more configuration files are invalid.
    #[error("{}", format_issues(.0))]
    Config(Vec<ConfigIssue>),
    /// A requested chain, arm or agent does not exist.
    #[error("unknown {kind} `{name}`")]
    Unknown {
        /// What kind of thing was looked up (`chain`, `arm`, `agent`).
        kind: &'static str,
        /// The name that was not found.
        name: String,
    },
    /// A `git` invocation failed.
    #[error("git {args}: {message}")]
    Git {
        /// The git arguments, for the message.
        args: String,
        /// What went wrong (usually git's own stderr).
        message: String,
    },
    /// The base commit of a chain does not exist in the cloned repository.
    #[error("base commit {commit} does not exist in repository `{repo}`")]
    MissingCommit {
        /// The commit that was asked for.
        commit: String,
        /// The repository name from `repos.toml`.
        repo: String,
    },
    /// An agent process could not be started.
    #[error("cannot start agent process `{program}`: {source}")]
    Spawn {
        /// The program that failed to start.
        program: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A path from a configuration file would leave its sandbox directory.
    #[error("path `{0}` must be relative and must not contain `..`")]
    UnsafePath(String),
    /// A line of `runs.jsonl` could not be parsed.
    #[error("{file}:{line}: {message}")]
    Results {
        /// The results file.
        file: String,
        /// 1-based line number.
        line: usize,
        /// What went wrong.
        message: String,
    },
}

impl EvalError {
    /// Build an [`EvalError::Io`] from a context string and an I/O error.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

fn format_issues(issues: &[ConfigIssue]) -> String {
    let mut out = format!("{} configuration problem(s):", issues.len());
    for issue in issues {
        out.push_str("\n  ");
        out.push_str(&issue.to_string());
    }
    out
}
