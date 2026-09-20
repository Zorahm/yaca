//! The core without an interface.
//!
//! Not here for decoration: if something in the core cannot be exercised from
//! here, the webview has leaked where it does not belong. It is also the
//! cheapest way to watch the raw event stream — the very one the interface
//! renders the feed from.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::process::ProcessState;
use yaca_core::provider;
use yaca_core::provider::registry::{self, Providers};
use yaca_core::secrets;
use yaca_core::{Command, Effort, Mode, PermissionAnswer, SessionConfig};

mod sessions;

#[derive(Parser, Debug)]
#[command(
    name = "yaca",
    about = "Прогнать один турн ядра Yaca и напечатать события",
    args_conflicts_with_subcommands = true
)]
struct Args {
    /// Sessions, search, export, key (§9, §10).
    #[command(subcommand)]
    command: Option<sessions::Cmd>,

    /// What to ask or have done.
    prompt: Option<String>,

    /// Project root. Global: the §9 subcommands work on the same project.
    #[arg(short, long, global = true, default_value = ".")]
    project: PathBuf,

    /// Model id. Without it, the project's `.yaca/settings.json` decides, then
    /// the built-in default.
    ///
    /// No clap default on purpose: a default here would always be "given" and
    /// would override the project's model and provider on every run.
    #[arg(short, long)]
    model: Option<String>,

    /// plan | manual | auto | edits — session mode (§8).
    ///
    /// The default is the same as the window's: the CLI cannot have its own,
    /// or it would test a different core than the one running under the
    /// interface.
    #[arg(long, default_value = "manual")]
    mode: String,

    /// What to do with permission requests: deny | allow.
    ///
    /// Nobody can be asked in a single run, and the request has no timeout
    /// (§5.7) — without this the CLI would hang on the very first question.
    #[arg(long, default_value = "deny")]
    permissions: String,

    /// low | medium | high | extra | max
    #[arg(short, long, default_value = "extra")]
    effort: String,

    /// Print events verbatim, not in the human-readable form.
    #[arg(long)]
    jsonl: bool,

    /// Write a journal to `.yaca/sessions`.
    #[arg(long)]
    journal: bool,

    /// Provider from `providers.json`. Without it, whoever serves the model.
    #[arg(long)]
    provider: Option<String>,

    /// Resume a recorded session (§9). Implies `--journal`: a session that
    /// is not recorded cannot be resumed.
    #[arg(long)]
    resume: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    if let Some(cmd) = args.command {
        return sessions::run(&args.project, cmd);
    }
    let prompt = args
        .prompt
        .context("нечего спрашивать: задайте текст или подкоманду")?;

    // `yaca -p dir sessions` parses the subcommand name as the prompt: clap
    // takes the positional once a flag has been seen. Silently sending the
    // word "sessions" to a model is a worse answer than saying so.
    if sessions::is_command_name(&prompt) {
        anyhow::bail!(
            "`{prompt}` — это подкоманда, а флаги идут после неё: yaca {prompt} -p <проект>"
        );
    }

    // The same `providers.json` the window reads: a gateway configured there
    // must answer from the CLI too, or the core cannot be run without the
    // interface.
    let providers = Providers::load(&registry::config_dir()?)?;
    let mut config = SessionConfig::open(&args.project)?;
    if let Some(id) = args.resume {
        config.resume_session(SessionId::from(id));
    }
    // Flags over the project's settings and the resumed session, the pair
    // replaced together — the same rule the window applies, from the same place.
    config.choose_model(
        args.provider.unwrap_or_default(),
        args.model.unwrap_or_default(),
    );
    let (spec, model) = providers.resolve(&config.provider, &config.model)?;
    let spec = spec.clone();

    // Keychain first, environment second (§10). No key is not an error: the
    // request goes without it, and a server that wants one refuses with a
    // message naming the key. The source is reported on stderr, where it
    // cannot be mistaken for part of the `--jsonl` stream.
    let key = spec
        .needs_key()
        .then(|| secrets::api_key(&spec.id))
        .and_then(|(key, source)| {
            tracing::debug!(?source, provider = %spec.id, "ключ провайдера");
            key
        });

    config.provider = spec.id.clone();
    config.model = model;
    config.models = spec.models.clone();
    config.mode = parse_mode(&args.mode)?;
    config.effort = parse_effort(&args.effort)?;
    config.journal = args.journal || config.resume.is_some();

    let handle = yaca_core::spawn(config, provider::build_with_key(spec, key))?;
    let mut events = handle.subscribe();

    handle
        .send(Command::SendMessage {
            text: prompt,
            attachments: vec![],
        })
        .await?;

    let allow_everything = match args.permissions.as_str() {
        "allow" => true,
        "deny" => false,
        other => anyhow::bail!("неизвестный режим разрешений: {other}"),
    };

    let mut out = std::io::stdout().lock();
    while let Ok(event) = events.recv().await {
        if args.jsonl {
            serde_json::to_writer(&mut out, &event)?;
            out.write_all(b"\n")?;
            out.flush()?;
        } else if let Some(line) = human(&event) {
            writeln!(out, "{line}")?;
            out.flush()?;
        }

        if let AgentEvent::PermissionRequested { request } = &event {
            let answer = if allow_everything {
                PermissionAnswer::Allow
            } else {
                PermissionAnswer::Deny { reason: None }
            };
            handle
                .send(Command::AnswerPermission {
                    request_id: request.id.clone(),
                    answer,
                })
                .await?;
        }

        if matches!(
            event,
            AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }
        ) {
            break;
        }
    }

    Ok(())
}

fn parse_mode(s: &str) -> Result<Mode> {
    Ok(match s {
        "plan" => Mode::Plan,
        "manual" => Mode::Manual,
        "auto" => Mode::Auto,
        "edits" => Mode::Edits,
        other => anyhow::bail!("неизвестный режим: {other}"),
    })
}

fn parse_effort(s: &str) -> Result<Effort> {
    Ok(match s {
        "low" => Effort::Low,
        "medium" => Effort::Medium,
        "high" => Effort::High,
        "extra" => Effort::Extra,
        "max" => Effort::Max,
        other => anyhow::bail!("неизвестный уровень усилия: {other}"),
    })
}

/// The structured reason spelled out. Rough on purpose, like everything here:
/// §5.8's wording belongs to the feed, and this is a window on the stream.
fn checkpoint_reason(reason: &yaca_core::CheckpointReason) -> String {
    use yaca_core::CheckpointReason as R;
    match reason {
        R::Message => "before the message".into(),
        R::Command { command } => format!("before {command}"),
        R::Rollback => "before the rollback".into(),
    }
}

/// The human-readable form. Deliberately rough: this is not the feed, just a
/// window on the stream.
fn human(event: &AgentEvent) -> Option<String> {
    Some(match event {
        AgentEvent::TextDelta { text, .. } => text.clone(),
        AgentEvent::ThinkingDelta { text, .. } => format!("\x1b[2m{text}\x1b[0m"),
        AgentEvent::ToolCallStarted {
            tool,
            description,
            target,
            ..
        } => {
            let подпись = description.clone().unwrap_or_else(|| tool.clone());
            let цель = target.as_deref().unwrap_or("");
            format!("\n\x1b[33m● {подпись}\x1b[0m \x1b[2m{цель}\x1b[0m")
        }
        // Tool output as it arrives — the very point of command streaming:
        // without this branch the CLI would show the agent's work after the
        // tool finished, turning a progress stream into a dump.
        AgentEvent::ToolCallProgress { chunk, .. } => format!("\x1b[2m{chunk}\x1b[0m"),
        AgentEvent::ToolCallFinished { meta, .. } => {
            format!("\x1b[2m  {} строк · {} мс\x1b[0m", meta.lines, meta.duration_ms)
        }
        AgentEvent::ToolCallFailed { message, .. } => format!("\x1b[31m  ✕ {message}\x1b[0m"),
        AgentEvent::ToolCallRejected { reason, .. } => format!("\x1b[33m  ⊘ {reason}\x1b[0m"),
        AgentEvent::ToolCallDenied { reason, .. } => {
            let why = reason.as_deref().unwrap_or("без причины");
            format!("\x1b[31m  ⊘ Отклонено вами: {why}\x1b[0m")
        }
        AgentEvent::PermissionRequested { request } => format!(
            "\n\x1b[33m? {:?} · {} ({} из {})\x1b[0m\n  \x1b[2m{}\x1b[0m",
            request.kind, request.target, request.position, request.total, request.detail
        ),
        AgentEvent::RetryScheduled {
            attempt,
            of,
            delay_ms,
            ..
        } => format!("\x1b[2mПовтор через {} с · попытка {attempt} из {of}\x1b[0m", delay_ms / 1000),
        AgentEvent::Error { code, message, .. } => format!("\x1b[31mОшибка {code}: {message}\x1b[0m"),
        AgentEvent::PlanProposed { plan } => format!("\n--- план ---\n{plan}\n"),
        AgentEvent::FileChanged { fact, .. } => format!(
            "\x1b[2m  {} +{} −{}\x1b[0m",
            fact.path, fact.added, fact.removed
        ),
        AgentEvent::ProcessesUpdated { processes } => {
            let строки: Vec<String> = processes
                .iter()
                .map(|p| format!("  {} {} · {}", p.id, p.command, состояние(&p.state)))
                .collect();
            format!("\n\x1b[2mПроцессы:\n{}\x1b[0m", строки.join("\n"))
        }
        // §5.8. The id is printed because the CLI has no panel to click: it is
        // the only way to name a point when driving the core by hand.
        AgentEvent::CheckpointCreated { checkpoint } => format!(
            "\n\x1b[2m── ● {} · {}\x1b[0m",
            checkpoint.id,
            checkpoint_reason(&checkpoint.reason)
        ),
        AgentEvent::CheckpointRestored { files, .. } => {
            format!("\n\x1b[32mRolled back · {files} file(s)\x1b[0m")
        }
        // §5.11. The summary is printed whole: this is the one place it can be
        // read before the next turn, and reading it is the point.
        AgentEvent::ContextCompacted { summary, freed } => format!(
            "\n\x1b[2m── история сжата · освобождено ~{}k токенов ──\x1b[0m\n{summary}\n",
            freed / 1000
        ),
        AgentEvent::SummaryEdited { summary } => {
            format!("\n\x1b[2m── сводка переписана ──\x1b[0m\n{summary}\n")
        }
        AgentEvent::ProcessOutputReset { text, .. } => format!("\x1b[2m{text}\x1b[0m"),
        AgentEvent::ProcessOutput { chunk, .. } => format!("\x1b[2m{chunk}\x1b[0m"),
        AgentEvent::UsageUpdated { usage } => format!(
            "\n\x1b[2m{}k/{}k · ${:.4}\x1b[0m",
            usage.context_used / 1000,
            usage.context_max / 1000,
            usage.cost_usd
        ),
        AgentEvent::TurnCancelled { .. } => "\n\x1b[2mОстановлено\x1b[0m".into(),
        _ => return None,
    })
}

fn состояние(state: &ProcessState) -> String {
    match state {
        ProcessState::Running => "работает".into(),
        ProcessState::Exited { code: Some(code) } => format!("код {code}"),
        ProcessState::Exited { code: None } => "убит сигналом".into(),
        ProcessState::Stopped => "остановлен".into(),
    }
}