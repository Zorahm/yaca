//! `yaca-evals`: runs tasks from `evals/tasks` against a real model.
//!
//! ```text
//! cargo run -p yaca-evals -- run                      # every task, the provider the app configured
//! cargo run -p yaca-evals -- run --task fix-median    # one task, by directory name (repeatable)
//! cargo run -p yaca-evals -- run --model <id> --provider <id>
//! cargo run -p yaca-evals -- run --out <dir> --keep-workdirs
//! ```
//!
//! Exit status: 0 when every task is solved, 1 when any is not, 2 when the
//! run could not start (bad task, no provider, no key). Status 2 is the case
//! that matters most: without a real model there is nothing to measure, and
//! the runner refuses rather than print numbers for a run that never happened.
//!
//! Output, under `--out` (default `target/yaca-evals`, which git already ignores):
//!
//! ```text
//! run-<stamp>/
//!     summary.json          every task's result plus totals
//!     <task>/result.json    one task's result (the same object as in summary.json)
//!     <task>/events.jsonl   the event stream of that run, one event per line
//! ```
//!
//! Model precedence: `--model`, then the task's `model`, then whatever the
//! provider resolves to. `--provider` picks the provider; without it the
//! provider list decides, as `yaca` does.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use yaca_evals::provider::{load_providers, real_plan, Plan};
use yaca_evals::result::{render_table, write_json, Summary, TaskResult};
use yaca_evals::run::{run_task, RunOptions};
use yaca_evals::task::{load_tasks, Task};
use yaca_evals::workdir::create_unique;

/// `evals/tasks` in this repository, found from the crate's own location so the
/// command works from any working directory. `--tasks-dir` overrides it.
const DEFAULT_TASKS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../evals/tasks");

#[derive(Parser, Debug)]
#[command(name = "yaca-evals", about = "Run Yaca against real tasks with a real model")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run the tasks and write the results.
    Run(RunArgs),
}

#[derive(Args, Debug)]
struct RunArgs {
    /// A task directory name. Repeat for several; none means all.
    #[arg(long = "task", value_name = "ID")]
    tasks: Vec<String>,

    /// Model id for every task. Overrides a task's own `model`.
    #[arg(long)]
    model: Option<String>,

    /// Provider id from `providers.json`.
    #[arg(long)]
    provider: Option<String>,

    /// Where the results go. A new `run-<stamp>` directory is made inside it.
    #[arg(long, default_value = "target/yaca-evals")]
    out: PathBuf,

    /// The tasks directory.
    #[arg(long, default_value = DEFAULT_TASKS_DIR)]
    tasks_dir: PathBuf,

    /// Keep each task's working tree after its run, for looking at what the
    /// agent changed. Off by default: trees are copies and can be large.
    #[arg(long)]
    keep_workdirs: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let Cmd::Run(args) = cli.command;
    match run(args).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("yaca-evals: {e:#}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(true)` when every task was solved.
async fn run(args: RunArgs) -> Result<bool> {
    let tasks = load_tasks(&args.tasks_dir, &args.tasks)?;

    // Every plan is built before the first run starts. A task that names a
    // model no provider has, or a provider with no key, stops the suite here
    // rather than after the tasks before it have spent money.
    let providers = load_providers()?;
    let provider_flag = args.provider.clone().unwrap_or_default();
    let mut plans: Vec<Plan> = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let model = args
            .model
            .clone()
            .or_else(|| task.model.clone())
            .unwrap_or_default();
        let plan = real_plan(&providers, &provider_flag, &model)
            .with_context(|| format!("task `{}`", task.id))?;
        plans.push(plan);
    }

    let started_ms = now_ms();
    let out_dir = create_unique(&args.out, "run").context("cannot create the output directory")?;
    let work_root = std::env::temp_dir().join("yaca-evals");
    let options = RunOptions { work_root: &work_root, keep_workdirs: args.keep_workdirs };

    eprintln!(
        "yaca-evals: {} task(s) -> {}",
        tasks.len(),
        out_dir.display()
    );

    let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());
    for (index, (task, plan)) in tasks.iter().zip(&plans).enumerate() {
        eprintln!("[{}/{}] {} ({}) ...", index + 1, tasks.len(), task.id, plan.model);
        let record = run_task(task, plan, &options).await;
        persist(&out_dir, task, &record.result, &record.events)?;
        eprintln!(
            "[{}/{}] {} {}",
            index + 1,
            tasks.len(),
            task.id,
            if record.result.solved { "solved".to_string() } else {
                format!("FAILED: {}", record.result.failure.clone().unwrap_or_default())
            }
        );
        results.push(record.result);
    }

    let summary = Summary::new(started_ms, &provider_label(&plans), results);
    write_json(&out_dir.join("summary.json"), &summary)?;
    print!("{}", render_table(&summary));
    println!("results: {}", out_dir.display());
    Ok(summary.all_solved())
}

/// Written after each task, not at the end: a run that is interrupted halfway
/// keeps what it finished.
fn persist(out_dir: &std::path::Path, task: &Task, result: &TaskResult, events: &[String]) -> Result<()> {
    let dir = out_dir.join(&task.id);
    write_json(&dir.join("result.json"), result)?;
    let mut log = events.join("\n");
    if !log.is_empty() {
        log.push('\n');
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("events.jsonl"), log)
        .with_context(|| format!("cannot write the events of `{}`", task.id))?;
    Ok(())
}

/// The providers the run used, once each. Usually one.
fn provider_label(plans: &[Plan]) -> String {
    let mut ids: Vec<&str> = plans.iter().map(|p| p.provider_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.join(",")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
