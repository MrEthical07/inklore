//! Tests of the `command` adapter. The child process is this very test binary, re-invoked with
//! `--ignored --exact <helper>`, so no shell scripts or external programs are needed. The helpers are
//! `#[ignore]`d so a normal `cargo test` never runs them directly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may unwrap (AGENTS.md)

use std::path::Path;
use std::time::{Duration, Instant};

use inklore_eval::{Agent, AgentExit, AgentRequest, CommandAgent};

/// Child: prints a metrics line as its last stdout line and exits.
#[test]
#[ignore = "helper: re-invoked as a child process by the command adapter tests"]
fn child_emit_json() {
    // libtest has already printed "test child_emit_json ... " without a newline; start a fresh line.
    println!();
    println!("some progress output");
    println!(
        r#"{{"turns": 4, "input_tokens": 1200, "cache_read_tokens": 800, "output_tokens": 90, "cost_usd": 0.25, "agent_version": "fake-1.0"}}"#
    );
    // Exit now so libtest cannot print its summary after our line.
    std::process::exit(0);
}

/// Child: ignores everything and sleeps far longer than any test timeout.
#[test]
#[ignore = "helper: re-invoked as a child process by the command adapter tests"]
fn child_sleep() {
    std::thread::sleep(Duration::from_secs(600));
}

/// Child: exits with a failure status.
#[test]
#[ignore = "helper: re-invoked as a child process by the command adapter tests"]
fn child_exit_three() {
    std::process::exit(3);
}

/// Grandchild: writes `survivor.txt` in its working directory after a few seconds, unless killed first.
#[test]
#[ignore = "helper: re-invoked as a child process by the command adapter tests"]
fn child_late_writer() {
    std::thread::sleep(Duration::from_secs(4));
    std::fs::write("survivor.txt", "I was not killed").unwrap();
}

/// Child: starts a grandchild (a second copy of this binary) and then sleeps.
#[test]
#[ignore = "helper: re-invoked as a child process by the command adapter tests"]
#[allow(clippy::zombie_processes)] // the grandchild is meant to be left running
fn child_spawns_grandchild() {
    let exe = std::env::current_exe().unwrap();
    std::process::Command::new(exe)
        .args(["--ignored", "--exact", "child_late_writer", "--nocapture"])
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(600));
}

fn agent_for(helper: &str) -> CommandAgent {
    let exe = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let args = [
        "--ignored",
        "--exact",
        helper,
        "--nocapture",
        "--test-threads=1",
    ];
    let argv = std::iter::once(exe).chain(args.map(String::from)).collect();
    CommandAgent::new("child", argv)
}

fn request<'a>(workdir: &'a Path, log_dir: &'a Path, timeout: Duration) -> AgentRequest<'a> {
    AgentRequest {
        task_id: "t1",
        prompt: "do the thing",
        workdir,
        timeout,
        log_dir,
    }
}

#[test]
fn runs_a_real_child_and_parses_metrics_from_the_last_stdout_line() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    let outcome = agent_for("child_emit_json")
        .run(&request(dir.path(), &logs, Duration::from_secs(120)))
        .unwrap();

    assert_eq!(outcome.exit, AgentExit::Completed);
    assert_eq!(outcome.metrics.turns, Some(4));
    assert_eq!(outcome.metrics.input_tokens, Some(1200));
    assert_eq!(outcome.metrics.cache_read_tokens, Some(800));
    assert_eq!(outcome.metrics.output_tokens, Some(90));
    assert_eq!(outcome.metrics.cost_usd, Some(0.25));
    assert_eq!(
        outcome.metrics.cache_write_tokens, None,
        "missing fields stay None"
    );
    assert_eq!(outcome.metrics.compactions, None);
    assert_eq!(outcome.agent_version.as_deref(), Some("fake-1.0"));

    let stdout = std::fs::read_to_string(logs.join("t1.stdout.log")).unwrap();
    assert!(stdout.contains("some progress output"));
    assert_eq!(
        std::fs::read_to_string(logs.join("t1.prompt.md")).unwrap(),
        "do the thing"
    );
    assert!(logs.join("t1.stderr.log").is_file());
}

#[test]
fn a_nonzero_exit_is_a_failure_with_no_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = agent_for("child_exit_three")
        .run(&request(
            dir.path(),
            &dir.path().join("logs"),
            Duration::from_secs(120),
        ))
        .unwrap();
    assert!(
        matches!(outcome.exit, AgentExit::Failed { .. }),
        "{:?}",
        outcome.exit
    );
    assert_eq!(outcome.metrics.turns, None);
}

#[test]
fn a_child_over_its_timeout_is_killed_and_reported_as_timed_out() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let outcome = agent_for("child_sleep")
        .run(&request(
            dir.path(),
            &dir.path().join("logs"),
            Duration::from_secs(2),
        ))
        .unwrap();
    assert_eq!(outcome.exit, AgentExit::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "the child must be killed, not waited for: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_program_that_cannot_start_is_an_error_not_an_agent_failure() {
    let dir = tempfile::tempdir().unwrap();
    let agent = CommandAgent::new(
        "ghost",
        vec!["definitely-not-a-real-program-xyz".to_string()],
    );
    let error = agent
        .run(&request(
            dir.path(),
            &dir.path().join("logs"),
            Duration::from_secs(5),
        ))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("definitely-not-a-real-program-xyz")
    );
}

#[test]
fn the_whole_process_tree_is_killed_on_timeout_not_just_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = agent_for("child_spawns_grandchild")
        .run(&request(
            dir.path(),
            &dir.path().join("logs"),
            Duration::from_secs(2),
        ))
        .unwrap();
    assert_eq!(outcome.exit, AgentExit::TimedOut);
    // The grandchild would write its file 4 s after it started; wait well past that.
    std::thread::sleep(Duration::from_secs(7));
    assert!(
        !dir.path().join("survivor.txt").exists(),
        "the grandchild survived the timeout"
    );
}
