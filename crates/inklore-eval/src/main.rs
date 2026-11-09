//! Command-line entry point of the evaluation harness.
// The binary talks to the user through stdout/stderr; the library never prints.
#![allow(clippy::print_stdout)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use inklore_eval::config::EvalConfig;
use inklore_eval::{RunOptions, run_batch, write_report};

/// Inklore evaluation harness: run task chains against arms and report the results.
#[derive(Debug, Parser)]
#[command(name = "inklore-eval", version)]
struct Cli {
    /// Location of the eval directory.
    #[arg(long, global = true, default_value = "eval")]
    eval_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse every repo, arm, chain and agent file and report all errors.
    Validate,
    /// Run chains against arms with an agent.
    Run {
        /// Chain ids to run (repeatable, or several values).
        #[arg(long, required = true, num_args = 1..)]
        chain: Vec<String>,
        /// Arm names to run (repeatable, or several values).
        #[arg(long, required = true, num_args = 1..)]
        arm: Vec<String>,
        /// Agent name (`eval/agents/<name>.toml`).
        #[arg(long)]
        agent: String,
        /// Repeats of each chain × arm.
        #[arg(long, default_value_t = 1)]
        runs: u32,
        /// Stop a chain after its first failed task.
        #[arg(long)]
        stop_on_fail: bool,
    },
    /// Regenerate report.md from the runs.jsonl in a batch directory.
    Report {
        /// A directory under `eval/results/`.
        batch_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate => validate(&cli.eval_dir),
        Command::Run {
            chain,
            arm,
            agent,
            runs,
            stop_on_fail,
        } => {
            let options = RunOptions {
                chains: chain,
                arms: arm,
                agent,
                runs,
                stop_on_fail,
            };
            let summary = run_batch(&cli.eval_dir, &options, &|line| println!("{line}"))?;
            println!(
                "{} run(s), {} task execution(s), {} failed. Report: {}",
                summary.runs,
                summary.task_executions,
                summary.failed_tasks,
                summary.batch_dir.join("report.md").display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Report { batch_dir } => {
            write_report(&batch_dir).context("generating the report")?;
            println!("wrote {}", batch_dir.join("report.md").display());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn validate(eval_dir: &std::path::Path) -> Result<ExitCode> {
    let (config, issues) = EvalConfig::load_lenient(eval_dir);
    if issues.is_empty() {
        println!(
            "ok: {} repo(s), {} arm(s), {} chain(s)",
            config.repos.len(),
            config.arms.len(),
            config.chains.len()
        );
        return Ok(ExitCode::SUCCESS);
    }
    for issue in &issues {
        eprintln!("{issue}");
    }
    eprintln!("{} problem(s) found", issues.len());
    Ok(ExitCode::FAILURE)
}
