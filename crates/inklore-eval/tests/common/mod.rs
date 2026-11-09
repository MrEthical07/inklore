//! Shared helpers: throwaway Git repositories and eval directories in temp folders.
#![allow(dead_code)] // each test binary uses a different subset
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may unwrap (AGENTS.md)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use inklore_eval::{RunOptions, TaskRecord};
use tempfile::TempDir;

/// Run `git` in `dir` with a fixed identity; panics on failure (tests may).
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git must be installed to run these tests");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A TOML basic string for `text`, escaped properly (Windows paths contain backslashes).
pub fn toml_str(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

/// Create a Git repository at `dir` with one commit containing `files`; returns the commit hash.
pub fn init_repo(dir: &Path, files: &[(&str, &str)]) -> String {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "--quiet"]);
    for (path, content) in files {
        let full = dir.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, content).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", "initial"]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A temp directory holding a source repository and an eval directory that points at it.
pub struct Fixture {
    pub tmp: TempDir,
    pub eval: PathBuf,
    pub repo: PathBuf,
    pub base: String,
}

impl Fixture {
    /// Source repo with `README.md`, eval dir with `repos.toml` (repo `r`) and the `baseline` arm.
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("source-repo");
        let base = init_repo(&repo, &[("README.md", "hello\n")]);
        let eval = tmp.path().join("eval");
        fs::create_dir_all(eval.join("arms")).unwrap();
        fs::write(
            eval.join("repos.toml"),
            format!(
                "[repos.r]\nsource = {}\n",
                toml_str(&repo.to_string_lossy())
            ),
        )
        .unwrap();
        fs::write(
            eval.join("arms").join("baseline.toml"),
            "name = \"baseline\"\ndescription = \"as is\"\n",
        )
        .unwrap();
        Self {
            tmp,
            eval,
            repo,
            base,
        }
    }

    /// Write a file below the eval directory.
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.eval.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// Add chain `id` on repo `r` at the fixture's base commit. `tasks_toml` is the `[[tasks]]` part.
    pub fn add_chain(&self, id: &str, tasks_toml: &str, prompts: &[&str]) {
        let header = format!(
            "id = \"{id}\"\nrepo = \"r\"\nbase_commit = \"{}\"\n\n",
            self.base
        );
        self.write(
            &format!("chains/{id}/chain.toml"),
            &format!("{header}{tasks_toml}"),
        );
        for prompt in prompts {
            self.write(&format!("chains/{id}/{prompt}"), "do the thing\n");
        }
    }

    /// Add a scripted agent named `name`; `body` is everything after `kind = "scripted"`.
    pub fn add_scripted_agent(&self, name: &str, body: &str) {
        self.write(
            &format!("agents/{name}.toml"),
            &format!("kind = \"scripted\"\n{body}"),
        );
    }

    /// Options for one run of `chain` under `baseline` with `agent`.
    pub fn options(&self, chain: &str, agent: &str) -> RunOptions {
        RunOptions {
            chains: vec![chain.to_string()],
            arms: vec!["baseline".to_string()],
            agent: agent.to_string(),
            runs: 1,
            stop_on_fail: false,
        }
    }
}

/// Read `runs.jsonl` of a finished batch.
pub fn records(batch_dir: &Path) -> Vec<TaskRecord> {
    inklore_eval::read_records(&batch_dir.join("runs.jsonl")).unwrap()
}
