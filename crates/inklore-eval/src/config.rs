//! The TOML file formats under `eval/` and the loader/validator for them.
//!
//! Layout of an eval directory:
//!
//! ```text
//! eval/repos.toml
//! eval/arms/<name>.toml
//! eval/chains/<id>/chain.toml  (+ prompt files next to it)
//! eval/agents/<name>.toml
//! ```
//!
//! Every struct rejects unknown fields and unknown check kinds: a typo is an error, never silently
//! ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{ConfigIssue, EvalError};

/// Default time limit for one agent run, in seconds, when a task does not set `timeout_secs`.
// DECISION(T-002): the task file shows `timeout_secs` but does not say whether it is required or what
// the default is. 1800 s is the value in its example; checks default to the example's 600 s.
pub const DEFAULT_TASK_TIMEOUT_SECS: u64 = 1800;
/// Default time limit for one `command` check, in seconds.
pub const DEFAULT_CHECK_TIMEOUT_SECS: u64 = 600;

fn default_task_timeout() -> u64 {
    DEFAULT_TASK_TIMEOUT_SECS
}

fn default_check_timeout() -> u64 {
    DEFAULT_CHECK_TIMEOUT_SECS
}

/// Contents of `eval/repos.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReposFile {
    /// Repositories by name.
    #[serde(default)]
    pub repos: BTreeMap<String, RepoDef>,
}

/// One repository the chains run against.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoDef {
    /// A Git URL or a local path (a relative path is resolved against the eval directory).
    pub source: String,
}

/// Contents of `eval/arms/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmDef {
    /// Arm name; must equal the file name without `.toml`.
    pub name: String,
    /// What the arm sets up.
    #[serde(default)]
    pub description: String,
    /// Files (or directories) copied into the working copy before the chain starts.
    #[serde(default)]
    pub copy: Vec<CopyRule>,
}

/// One copy instruction of an arm.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopyRule {
    /// Source path, relative to the arm file.
    pub from: String,
    /// Destination path, relative to the working copy root.
    pub to: String,
}

/// Contents of `eval/chains/<id>/chain.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainDef {
    /// Chain id; must equal the directory name.
    pub id: String,
    /// Name of a repository in `repos.toml`.
    pub repo: String,
    /// Commit every run starts from.
    pub base_commit: String,
    /// What the chain exercises.
    #[serde(default)]
    pub description: String,
    /// The ordered tasks.
    pub tasks: Vec<TaskDef>,
}

/// One task of a chain: a prompt plus checks.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDef {
    /// Task id, unique within the chain.
    pub id: String,
    /// Prompt file, relative to the chain's folder.
    pub prompt: String,
    /// Agent time limit in seconds.
    #[serde(default = "default_task_timeout")]
    pub timeout_secs: u64,
    /// Checks that decide pass/fail after the agent has run.
    #[serde(default)]
    pub checks: Vec<CheckDef>,
}

/// A check that decides pass/fail after a task.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckDef {
    /// Runs an argv (never a shell string) in the working copy; passes on exit code 0.
    Command {
        /// Display name.
        name: String,
        /// Program and arguments.
        command: Vec<String>,
        /// Time limit in seconds.
        #[serde(default = "default_check_timeout")]
        timeout_secs: u64,
    },
    /// Passes when the file exists and contains the text.
    FileContains {
        /// Display name.
        name: String,
        /// File path relative to the working copy root.
        path: String,
        /// Text that must occur in the file.
        text: String,
    },
    /// Passes when the path does not exist.
    // DECISION(T-002): the task file names `file_absent` without defining it; it is taken to mean
    // "this path must not exist in the working copy".
    FileAbsent {
        /// Display name.
        name: String,
        /// Path relative to the working copy root.
        path: String,
    },
}

impl CheckDef {
    /// The check's display name.
    pub fn name(&self) -> &str {
        match self {
            Self::Command { name, .. }
            | Self::FileContains { name, .. }
            | Self::FileAbsent { name, .. } => name,
        }
    }
}

/// Contents of `eval/agents/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentDef {
    /// Runs an external program for each task.
    Command {
        /// Program and arguments; `{prompt_file}` and `{workdir}` are substituted.
        argv: Vec<String>,
    },
    /// A scripted fake agent for tests: actions per task id.
    Scripted {
        /// Actions by task id, as `[[tasks.<id>]]` tables.
        #[serde(default)]
        tasks: BTreeMap<String, Vec<ScriptAction>>,
    },
}

/// One step of a scripted agent.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScriptAction {
    /// Create or overwrite a file.
    WriteFile {
        /// Path relative to the working copy.
        path: String,
        /// New contents.
        content: String,
    },
    /// Append to a file, creating it if needed.
    AppendFile {
        /// Path relative to the working copy.
        path: String,
        /// Text to append.
        content: String,
    },
    /// Delete a file.
    DeleteFile {
        /// Path relative to the working copy.
        path: String,
    },
    /// Sleep; a sleep longer than the task timeout makes the run time out.
    Sleep {
        /// Seconds to sleep.
        secs: u64,
    },
    /// Stop with a failure.
    Fail {
        /// Failure message.
        message: String,
    },
}

/// A loaded arm and the directory its relative paths resolve against.
#[derive(Debug, Clone)]
pub struct Arm {
    /// The parsed file.
    pub def: ArmDef,
    /// Directory containing the arm file.
    pub dir: PathBuf,
}

/// A loaded chain and the directory holding its prompt files.
#[derive(Debug, Clone)]
pub struct Chain {
    /// The parsed file.
    pub def: ChainDef,
    /// Directory containing `chain.toml` and the prompts.
    pub dir: PathBuf,
}

/// Everything under an eval directory that is not an agent.
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// The eval directory itself.
    pub eval_dir: PathBuf,
    /// Repositories by name.
    pub repos: BTreeMap<String, RepoDef>,
    /// Arms by name.
    pub arms: BTreeMap<String, Arm>,
    /// Chains by id.
    pub chains: BTreeMap<String, Chain>,
}

impl EvalConfig {
    /// Load and validate an eval directory; any problem is returned as [`EvalError::Config`].
    pub fn load(eval_dir: &Path) -> Result<Self, EvalError> {
        let (config, issues) = Self::load_lenient(eval_dir);
        if issues.is_empty() {
            Ok(config)
        } else {
            Err(EvalError::Config(issues))
        }
    }

    /// Load as much as parses and collect *every* problem found, including in agent files.
    pub fn load_lenient(eval_dir: &Path) -> (Self, Vec<ConfigIssue>) {
        let mut issues = Vec::new();
        let repos = load_repos(eval_dir, &mut issues);
        let arms = load_arms(eval_dir, &mut issues);
        let chains = load_chains(eval_dir, &repos, &mut issues);
        check_agents(eval_dir, &mut issues);
        let config = Self {
            eval_dir: eval_dir.to_path_buf(),
            repos,
            arms,
            chains,
        };
        (config, issues)
    }
}

/// Load one agent configuration by name from `eval/agents/<name>.toml`.
pub fn load_agent_def(eval_dir: &Path, name: &str) -> Result<AgentDef, EvalError> {
    let path = eval_dir.join("agents").join(format!("{name}.toml"));
    if !path.is_file() {
        return Err(EvalError::Unknown {
            kind: "agent",
            name: name.to_string(),
        });
    }
    let mut issues = Vec::new();
    match parse_toml::<AgentDef>(&path, &mut issues) {
        Some(def) if issues.is_empty() => Ok(def),
        _ => Err(EvalError::Config(issues)),
    }
}

/// `true` for names that are safe in file names and run ids: ASCII letters, digits, `-`, `_`,
/// at most 64 characters, and no `__` (which separates the parts of a run id).
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.contains("__")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `true` when `path` is non-empty, relative, and has no `..` component.
pub fn is_safe_relative(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Join a relative path from a config file onto `base`, rejecting absolute and `..` paths.
pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, EvalError> {
    if is_safe_relative(relative) {
        Ok(base.join(relative))
    } else {
        Err(EvalError::UnsafePath(relative.to_string()))
    }
}

fn issue(file: &Path, message: impl Into<String>) -> ConfigIssue {
    ConfigIssue {
        file: file.to_path_buf(),
        line: None,
        column: None,
        message: message.into(),
    }
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = text.as_bytes().get(..offset).unwrap_or_default();
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let line_start = before
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |i| i + 1);
    let column = String::from_utf8_lossy(before.get(line_start..).unwrap_or_default())
        .chars()
        .count()
        + 1;
    (line, column)
}

fn parse_toml<T: DeserializeOwned>(path: &Path, issues: &mut Vec<ConfigIssue>) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            issues.push(issue(path, format!("cannot read file: {error}")));
            return None;
        }
    };
    match toml::from_str::<T>(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            let location = error.span().map(|span| line_col(&text, span.start));
            issues.push(ConfigIssue {
                file: path.to_path_buf(),
                line: location.map(|(line, _)| line),
                column: location.map(|(_, column)| column),
                message: error.message().to_string(),
            });
            None
        }
    }
}

/// Sorted entries of a directory; a missing directory is an empty list.
fn list_dir(dir: &Path, issues: &mut Vec<ConfigIssue>) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut entries = match fs::read_dir(dir) {
        Ok(read) => read
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>(),
        Err(error) => {
            issues.push(issue(dir, format!("cannot list directory: {error}")));
            return Vec::new();
        }
    };
    entries.sort();
    entries
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn load_repos(eval_dir: &Path, issues: &mut Vec<ConfigIssue>) -> BTreeMap<String, RepoDef> {
    let path = eval_dir.join("repos.toml");
    if !path.is_file() {
        issues.push(issue(
            &path,
            "missing file (an empty `[repos]` table is fine)",
        ));
        return BTreeMap::new();
    }
    let Some(file) = parse_toml::<ReposFile>(&path, issues) else {
        return BTreeMap::new();
    };
    for (name, repo) in &file.repos {
        if !is_valid_name(name) {
            issues.push(issue(
                &path,
                format!("repo name `{name}` is not a valid name"),
            ));
        }
        if repo.source.trim().is_empty() {
            issues.push(issue(&path, format!("repo `{name}`: `source` is empty")));
        }
    }
    file.repos
}

fn load_arms(eval_dir: &Path, issues: &mut Vec<ConfigIssue>) -> BTreeMap<String, Arm> {
    let mut arms = BTreeMap::new();
    for path in list_dir(&eval_dir.join("arms"), issues) {
        if path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let Some(def) = parse_toml::<ArmDef>(&path, issues) else {
            continue;
        };
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        check_arm(&path, &dir, &def, issues);
        arms.insert(def.name.clone(), Arm { def, dir });
    }
    arms
}

fn check_arm(path: &Path, dir: &Path, def: &ArmDef, issues: &mut Vec<ConfigIssue>) {
    if !is_valid_name(&def.name) {
        issues.push(issue(
            path,
            format!("arm name `{}` is not a valid name", def.name),
        ));
    }
    if def.name != file_stem(path) {
        issues.push(issue(
            path,
            format!(
                "arm name `{}` must equal the file name `{}`",
                def.name,
                file_stem(path)
            ),
        ));
    }
    for rule in &def.copy {
        if !is_safe_relative(&rule.from) {
            issues.push(issue(
                path,
                format!("copy `from` `{}` must be relative, without `..`", rule.from),
            ));
        } else if !dir.join(&rule.from).exists() {
            issues.push(issue(
                path,
                format!("copy `from` `{}` does not exist", rule.from),
            ));
        }
        if !is_safe_relative(&rule.to) {
            issues.push(issue(
                path,
                format!("copy `to` `{}` must be relative, without `..`", rule.to),
            ));
        }
    }
}

fn load_chains(
    eval_dir: &Path,
    repos: &BTreeMap<String, RepoDef>,
    issues: &mut Vec<ConfigIssue>,
) -> BTreeMap<String, Chain> {
    let mut chains = BTreeMap::new();
    for dir in list_dir(&eval_dir.join("chains"), issues) {
        if !dir.is_dir() {
            continue;
        }
        let file = dir.join("chain.toml");
        if !file.is_file() {
            issues.push(issue(&file, "missing `chain.toml`"));
            continue;
        }
        let Some(def) = parse_toml::<ChainDef>(&file, issues) else {
            continue;
        };
        check_chain(&file, &dir, &def, repos, issues);
        chains.insert(def.id.clone(), Chain { def, dir });
    }
    chains
}

fn check_chain(
    file: &Path,
    dir: &Path,
    def: &ChainDef,
    repos: &BTreeMap<String, RepoDef>,
    issues: &mut Vec<ConfigIssue>,
) {
    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !is_valid_name(&def.id) {
        issues.push(issue(
            file,
            format!("chain id `{}` is not a valid name", def.id),
        ));
    }
    if def.id != dir_name {
        issues.push(issue(
            file,
            format!(
                "chain id `{}` must equal its folder name `{dir_name}`",
                def.id
            ),
        ));
    }
    if !repos.contains_key(&def.repo) {
        issues.push(issue(
            file,
            format!("repo `{}` is not defined in repos.toml", def.repo),
        ));
    }
    let commit = &def.base_commit;
    if commit.len() < 7 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        issues.push(issue(
            file,
            format!("base_commit `{commit}` must be a commit hash (7 or more hex digits)"),
        ));
    }
    if def.tasks.is_empty() {
        issues.push(issue(file, "a chain needs at least one task"));
    }
    let mut seen = BTreeSet::new();
    for task in &def.tasks {
        if !seen.insert(task.id.as_str()) {
            issues.push(issue(file, format!("duplicate task id `{}`", task.id)));
        }
        check_task(file, dir, task, issues);
    }
}

fn check_task(file: &Path, dir: &Path, task: &TaskDef, issues: &mut Vec<ConfigIssue>) {
    let id = &task.id;
    if !is_valid_name(id) {
        issues.push(issue(file, format!("task id `{id}` is not a valid name")));
    }
    if task.timeout_secs == 0 {
        issues.push(issue(
            file,
            format!("task `{id}`: `timeout_secs` must be greater than 0"),
        ));
    }
    if !is_safe_relative(&task.prompt) {
        issues.push(issue(
            file,
            format!(
                "task `{id}`: prompt `{}` must be relative, without `..`",
                task.prompt
            ),
        ));
    } else if !dir.join(&task.prompt).is_file() {
        issues.push(issue(
            file,
            format!(
                "task `{id}`: prompt file `{}` not found next to chain.toml",
                task.prompt
            ),
        ));
    }
    let mut names = BTreeSet::new();
    for check in &task.checks {
        if !names.insert(check.name()) {
            issues.push(issue(
                file,
                format!("task `{id}`: duplicate check name `{}`", check.name()),
            ));
        }
        check_check(file, id, check, issues);
    }
}

fn check_check(file: &Path, task_id: &str, check: &CheckDef, issues: &mut Vec<ConfigIssue>) {
    let name = check.name();
    match check {
        CheckDef::Command {
            command,
            timeout_secs,
            ..
        } => {
            if command.first().is_none_or(|program| program.is_empty()) {
                issues.push(issue(
                    file,
                    format!("task `{task_id}`, check `{name}`: `command` must not be empty"),
                ));
            }
            if *timeout_secs == 0 {
                issues.push(issue(
                    file,
                    format!(
                        "task `{task_id}`, check `{name}`: `timeout_secs` must be greater than 0"
                    ),
                ));
            }
        }
        CheckDef::FileContains { path, .. } | CheckDef::FileAbsent { path, .. } => {
            if !is_safe_relative(path) {
                issues.push(issue(file, format!("task `{task_id}`, check `{name}`: path `{path}` must be relative, without `..`")));
            }
        }
    }
}

fn check_agents(eval_dir: &Path, issues: &mut Vec<ConfigIssue>) {
    for path in list_dir(&eval_dir.join("agents"), issues) {
        if path.extension().is_some_and(|ext| ext == "toml") {
            let _ = parse_toml::<AgentDef>(&path, issues);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths() {
        assert!(is_valid_name("superapi-1"));
        assert!(!is_valid_name("a__b"));
        assert!(!is_valid_name("has space"));
        assert!(!is_valid_name(""));
        assert!(is_safe_relative("a/b.txt"));
        assert!(!is_safe_relative("../a"));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("a/../../b"));
        assert!(!is_safe_relative(""));
    }

    #[test]
    fn line_and_column_are_one_based() {
        assert_eq!(line_col("abc\ndef", 0), (1, 1));
        assert_eq!(line_col("abc\ndef", 5), (2, 2));
    }

    #[test]
    fn check_kinds_parse_and_unknowns_are_rejected() {
        let ok: ChainDef = toml::from_str(
            r#"
            id = "c"
            repo = "r"
            base_commit = "abcdef1"
            [[tasks]]
            id = "t1"
            prompt = "t1.md"
              [[tasks.checks]]
              name = "a"
              kind = "command"
              command = ["go", "test"]
              [[tasks.checks]]
              name = "b"
              kind = "file_contains"
              path = "x"
              text = "y"
              [[tasks.checks]]
              name = "c"
              kind = "file_absent"
              path = "z"
            "#,
        )
        .unwrap();
        assert_eq!(ok.tasks[0].checks.len(), 3);
        assert_eq!(ok.tasks[0].timeout_secs, DEFAULT_TASK_TIMEOUT_SECS);

        let unknown_kind = toml::from_str::<ChainDef>(
            "id='c'\nrepo='r'\nbase_commit='abcdef1'\n[[tasks]]\nid='t'\nprompt='p'\n[[tasks.checks]]\nname='n'\nkind='magic'\n",
        );
        assert!(unknown_kind.is_err());

        let unknown_field = toml::from_str::<ChainDef>(
            "id='c'\nrepo='r'\nbase_commit='abcdef1'\nbogus=1\ntasks=[]\n",
        );
        assert!(unknown_field.is_err());

        let unknown_check_field = toml::from_str::<ChainDef>(
            "id='c'\nrepo='r'\nbase_commit='abcdef1'\n[[tasks]]\nid='t'\nprompt='p'\n[[tasks.checks]]\nname='n'\nkind='file_absent'\npath='p'\nextra='x'\n",
        );
        assert!(unknown_check_field.is_err());
    }

    #[test]
    fn agent_configs_parse() {
        let scripted: AgentDef = toml::from_str(
            "kind='scripted'\n[[tasks.t1]]\naction='write_file'\npath='a.txt'\ncontent='hi'\n[[tasks.t1]]\naction='sleep'\nsecs=3\n",
        )
        .unwrap();
        assert!(matches!(scripted, AgentDef::Scripted { ref tasks } if tasks["t1"].len() == 2));
        let command: AgentDef =
            toml::from_str("kind='command'\nargv=['x','{prompt_file}']\n").unwrap();
        assert!(matches!(command, AgentDef::Command { .. }));
        assert!(toml::from_str::<AgentDef>("kind='nope'\n").is_err());
    }
}
