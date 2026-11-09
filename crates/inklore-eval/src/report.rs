//! Generating `report.md` from `runs.jsonl`. The output depends only on the records, in file order:
//! no clock, no randomness, no hash-map iteration.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::agent::AgentMetrics;
use crate::error::EvalError;
use crate::results::{REPORT_FILE, RUNS_FILE, TaskRecord, read_records};
use crate::stats::{Summary, sum_present, summarize};

/// One reported metric: its column header, how to read it, and how to print it.
struct Metric {
    header: &'static str,
    get: fn(&AgentMetrics) -> Option<f64>,
    format: fn(f64) -> String,
}

const METRICS: [Metric; 8] = [
    Metric {
        header: "Turns",
        get: |m| m.turns.map(f64::from),
        format: whole,
    },
    Metric {
        header: "Input tokens",
        get: |m| m.input_tokens.map(to_f64),
        format: whole,
    },
    Metric {
        header: "Cache read",
        get: |m| m.cache_read_tokens.map(to_f64),
        format: whole,
    },
    Metric {
        header: "Cache write",
        get: |m| m.cache_write_tokens.map(to_f64),
        format: whole,
    },
    Metric {
        header: "Output tokens",
        get: |m| m.output_tokens.map(to_f64),
        format: whole,
    },
    Metric {
        header: "Cost (USD)",
        get: |m| m.cost_usd,
        format: dollars,
    },
    Metric {
        header: "Compactions",
        get: |m| m.compactions.map(f64::from),
        format: whole,
    },
    Metric {
        header: "Wall time",
        get: |m| Some(seconds(m.wall_time)),
        format: secs_text,
    },
];

/// Placeholder for a metric no run reported.
const MISSING: &str = "–";

#[allow(clippy::cast_precision_loss)] // token counts stay far below 2^53
fn to_f64(value: u64) -> f64 {
    value as f64
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

fn whole(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn dollars(value: f64) -> String {
    format!("${value:.4}")
}

fn secs_text(value: f64) -> String {
    format!("{value:.1}s")
}

fn cell(summary: Option<Summary>, format: fn(f64) -> String) -> String {
    match summary {
        None => MISSING.to_string(),
        Some(s) => format!(
            "{} / {} / {}",
            format(s.median),
            format(s.min),
            format(s.max)
        ),
    }
}

fn pass_rate(passed: usize, total: usize) -> String {
    if total == 0 {
        return MISSING.to_string();
    }
    let percent = (passed * 100 + total / 2) / total;
    format!("{passed}/{total} ({percent}%)")
}

/// Read `<batch_dir>/runs.jsonl` and write `<batch_dir>/report.md`.
pub fn write_report(batch_dir: &Path) -> Result<(), EvalError> {
    let records = read_records(&batch_dir.join(RUNS_FILE))?;
    let path = batch_dir.join(REPORT_FILE);
    fs::write(&path, render_report(&records))
        .map_err(|e| EvalError::io(format!("write {}", path.display()), e))
}

/// Records of one (chain, arm, agent) group, split by run.
struct Group<'a> {
    /// Task ids in order of first appearance.
    task_order: Vec<&'a str>,
    /// Records per run id (sorted by run id).
    runs: BTreeMap<&'a str, Vec<&'a TaskRecord>>,
}

/// Render the whole report for `records`.
pub fn render_report(records: &[TaskRecord]) -> String {
    let mut groups: BTreeMap<(&str, &str, &str), Group<'_>> = BTreeMap::new();
    for record in records {
        let key = (
            record.chain.as_str(),
            record.arm.as_str(),
            record.agent.as_str(),
        );
        let group = groups.entry(key).or_insert_with(|| Group {
            task_order: Vec::new(),
            runs: BTreeMap::new(),
        });
        if !group.task_order.contains(&record.task_id.as_str()) {
            group.task_order.push(&record.task_id);
        }
        group.runs.entry(&record.run_id).or_default().push(record);
    }

    let mut out = String::from("# Evaluation report\n\n");
    let _ = writeln!(
        out,
        "{} task execution(s) in {} group(s).\n",
        records.len(),
        groups.len()
    );
    out.push_str(
        "Each metric cell reads `median / min / max` over runs; `–` means no run reported it.\n",
    );
    for ((chain, arm, agent), group) in &groups {
        render_group(&mut out, chain, arm, agent, group);
    }
    out
}

fn render_group(out: &mut String, chain: &str, arm: &str, agent: &str, group: &Group<'_>) {
    let _ = writeln!(
        out,
        "\n## chain `{chain}` × arm `{arm}` (agent `{agent}`)\n"
    );
    let first = group.runs.values().next().and_then(|run| run.first());
    if let Some(first) = first {
        let _ = writeln!(
            out,
            "Runs: {} · base commit `{}` · harness {} · {}\n",
            group.runs.len(),
            first.base_commit,
            first.harness_version,
            first.os
        );
    }
    out.push_str("| Task | Pass |");
    for metric in &METRICS {
        let _ = write!(out, " {} |", metric.header);
    }
    out.push_str("\n|---|---|");
    for _ in &METRICS {
        out.push_str("---|");
    }
    out.push('\n');

    for task_id in &group.task_order {
        let executions: Vec<&TaskRecord> = group
            .runs
            .values()
            .filter_map(|run| run.iter().find(|r| r.task_id == *task_id).copied())
            .collect();
        let passed = executions.iter().filter(|r| r.passed).count();
        let _ = write!(
            out,
            "| {task_id} | {} |",
            pass_rate(passed, executions.len())
        );
        for metric in &METRICS {
            let summary = summarize(executions.iter().map(|r| (metric.get)(&r.metrics)));
            let _ = write!(out, " {} |", cell(summary, metric.format));
        }
        out.push('\n');
    }

    // Chain totals: sum each run's tasks, then summarise across runs.
    let fully_passed = group
        .runs
        .values()
        .filter(|run| run.iter().all(|r| r.passed))
        .count();
    let _ = write!(
        out,
        "| **Chain total** | {} |",
        pass_rate(fully_passed, group.runs.len())
    );
    for metric in &METRICS {
        let totals = group
            .runs
            .values()
            .map(|run| sum_present(run.iter().map(|r| (metric.get)(&r.metrics))));
        let _ = write!(out, " {} |", cell(summarize(totals), metric.format));
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentExit;

    fn record(run: &str, task: &str, passed: bool, turns: Option<u32>) -> TaskRecord {
        TaskRecord {
            run_id: run.to_string(),
            chain: "c".to_string(),
            arm: "baseline".to_string(),
            agent: "fake".to_string(),
            run_number: 1,
            task_id: task.to_string(),
            passed,
            agent_exit: AgentExit::Completed,
            metrics: AgentMetrics {
                turns,
                ..AgentMetrics::default()
            },
            agent_version: None,
            checks: Vec::new(),
            changed_paths: Vec::new(),
            diff_stat: String::new(),
            harness_version: "0.0.0".to_string(),
            base_commit: "abc1234".to_string(),
            os: "testos".to_string(),
            started_unix: 0,
        }
    }

    #[test]
    fn pass_rate_rounds_and_handles_zero() {
        assert_eq!(pass_rate(2, 3), "2/3 (67%)");
        assert_eq!(pass_rate(0, 0), "–");
        assert_eq!(pass_rate(3, 3), "3/3 (100%)");
    }

    #[test]
    fn number_formats() {
        assert_eq!(whole(5.0), "5");
        assert_eq!(whole(2.5), "2.5");
        assert_eq!(dollars(0.0123), "$0.0123");
        assert_eq!(secs_text(1.25), "1.2s");
    }

    #[test]
    fn totals_sum_per_run_then_summarise() {
        let records = [
            record("r1", "t1", true, Some(2)),
            record("r1", "t2", true, Some(4)),
            record("r2", "t1", true, Some(10)),
            record("r2", "t2", false, Some(20)),
        ];
        let text = render_report(&records);
        // per task: t1 turns 2 and 10 -> median 6 / min 2 / max 10
        assert!(text.contains("| t1 | 2/2 (100%) | 6 / 2 / 10 |"), "{text}");
        // chain totals: run1 = 6, run2 = 30 -> median 18 / 6 / 30; one of two runs fully passed
        assert!(
            text.contains("| **Chain total** | 1/2 (50%) | 18 / 6 / 30 |"),
            "{text}"
        );
    }

    #[test]
    fn none_shows_as_dash_and_output_is_stable() {
        let records = [record("r1", "t1", true, None)];
        let a = render_report(&records);
        let b = render_report(&records);
        assert_eq!(a, b);
        assert!(a.contains("| t1 | 1/1 (100%) | – |"), "{a}");
    }
}
