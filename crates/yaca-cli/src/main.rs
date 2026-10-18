//! The core without an interface.
//!
//! Not here for decoration: if something in the core cannot be exercised from
//! here, the webview has leaked where it does not belong. It is also the
//! cheapest way to watch the raw event stream — the very one the interface
//! renders the feed from.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use yaca_core::event::AgentEvent;
use yaca_core::mcp::McpStatus;
use yaca_core::id::SessionId;
use yaca_core::process::ProcessState;
use yaca_core::provider;
use yaca_core::provider::registry::{self, Providers};
use yaca_core::provider::Provider;
use yaca_core::router::{self, ROUTER_ID};
use yaca_core::secrets;
use yaca_core::{Command, Effort, Mode, PermissionAnswer, SessionConfig};

mod mcp;
mod plugins;
mod sessions;
mod skills;

#[derive(Subcommand, Debug)]
enum Cmd {
    #[command(flatten)]
    Session(sessions::Cmd),

    /// Connect to the project's MCP servers and list them with their tools.
    Mcp {
        /// Approve a server declared in the project's `.mcp.json`. Such a
        /// server is not started until it is approved, and is asked about
        /// again when its declaration changes.
        #[arg(long, value_name = "NAME")]
        approve: Option<String>,
        /// Log in to a remote server that asks for it (OAuth): the browser
        /// opens the server's login page, and the login is kept in the
        /// system keychain.
        #[arg(long, value_name = "NAME")]
        login: Option<String>,
        /// Forget the login to a remote server.
        #[arg(long, value_name = "NAME")]
        logout: Option<String>,
    },

    /// List the skills the project and you have, and what is wrong with the
    /// ones that did not load.
    Skills,

    /// List your plugins — Yaca's own and those Claude Code installed — and
    /// turn one on or off for the chats opened after this.
    Plugins {
        #[command(subcommand)]
        action: Option<plugins::Action>,
        #[arg(long, value_name = "ID")]
        enable: Option<String>,
        #[arg(long, value_name = "ID")]
        disable: Option<String>,
    },
}

#[derive(Parser, Debug)]
#[command(
    name = "yaca",
    about = "Прогнать один турн ядра Yaca и напечатать события",
    args_conflicts_with_subcommands = true
)]
struct Args {
    /// Sessions, search, export, key (§9, §10), and the MCP servers.
    #[command(subcommand)]
    command: Option<Cmd>,

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
    #[arg(long, default_value = "auto")]
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

    /// Work in a git worktree of this run's own, on branch `yaca/<session>`.
    ///
    /// Needs a repository with a commit; what is uncommitted is not in it.
    #[arg(long)]
    isolate: bool,

    /// Print events verbatim, not in the human-readable form.
    #[arg(long)]
    jsonl: bool,

    /// Write a journal to the chats folder, beside the window's chats.
    #[arg(long)]
    journal: bool,

    /// Provider from `providers.json`. Without it, whoever serves the model.
    #[arg(long)]
    provider: Option<String>,

    /// Resume a recorded session (§9). Implies `--journal`: a session that
    /// is not recorded cannot be resumed.
    #[arg(long)]
    resume: Option<String>,

    /// Another folder the session may work in, besides the project. Repeatable.
    /// Its files are addressed by absolute path. Fixed for the session: a
    /// resume must repeat the same `--add-dir` flags.
    #[arg(long = "add-dir", value_name = "PATH")]
    add_dir: Vec<PathBuf>,
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

    match args.command {
        Some(Cmd::Session(cmd)) => return sessions::run(&args.project, cmd),
        Some(Cmd::Mcp { approve, login, logout }) => {
            return mcp::run(&args.project, approve, login, logout).await
        }
        Some(Cmd::Skills) => return skills::run(&args.project),
        Some(Cmd::Plugins { action, enable, disable }) => return plugins::run(action, enable, disable),
        None => {}
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
    let config_dir = registry::config_dir()?;
    let providers = Providers::load(&config_dir)?;
    let mut config = SessionConfig::open(&args.project)?;
    // The human's own MCP servers, from the same directory and for the same
    // reason as the providers: a server set up in the window must be there
    // from the CLI too.
    config.mcp_servers = yaca_core::mcp::config::load_global(&config_dir)?;
    config.lsp_servers = yaca_core::lsp::config::load_global(&config_dir)?;
    config.lsp_autodetect = true;
    config.user_skills = yaca_core::skills::user_dirs(&config_dir);
    config.user_agents = yaca_core::agents::user_dirs(&config_dir);
    config.plugins = yaca_core::plugin::PluginSources::standard(&config_dir).load().enabled();
    // The window's chats folder, so that a session recorded here is listed
    // there, and one held there can be resumed here.
    if args.journal || args.resume.is_some() {
        config.journal = Some(yaca_core::store::chats_dir()?);
    }
    config.mcp_approvals = Some(config_dir.clone());
    if let Some(id) = args.resume {
        config.resume_session(SessionId::from(id));
    }
    config.isolate = args.isolate;
    config.additional_roots = args.add_dir;
    // Flags over the project's settings and the resumed session, the pair
    // replaced together — the same rule the window applies, from the same place.
    config.choose_model(
        args.provider.unwrap_or_default(),
        args.model.unwrap_or_default(),
    );
    // The small model the `Decision` tool asks, when the Decision provider or
    // the router has one: the same rule the window applies, from the same place.
    config.decision = router::attach::decision_setup(&config_dir, &providers);
    let provider = if config.provider == ROUTER_ID {
        router::attach::attach(&config_dir, &providers, &mut config)?
    } else {
        resolve_provider(&providers, &mut config)?
    };
    config.mode = parse_mode(&args.mode)?;
    config.effort = parse_effort(&args.effort)?;

    let handle = yaca_core::spawn(config, provider)?;
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

        // Nobody sits at the other end of a headless run to pick an option.
        // The question is dismissed rather than left open, which would hang
        // the turn: the model is told there is no answer and carries on.
        if let AgentEvent::QuestionAsked { request } = &event {
            handle
                .send(Command::AnswerQuestion {
                    request_id: request.id.clone(),
                    answer: yaca_core::question::QuestionReply::Dismissed,
                })
                .await?;
        }

        // A server's form has no one to fill it in here: the server is told
        // the person declined, rather than left waiting.
        if let AgentEvent::ElicitationRequested { request } = &event {
            handle
                .send(Command::AnswerElicitation {
                    request_id: request.id.clone(),
                    reply: yaca_core::mcp::ElicitationReply::Decline,
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

/// Chooses the provider from `providers.json` and sets the provider, model and
/// model list on `config` to match. The key is read here, because only the
/// provider knows whether it needs one.
fn resolve_provider(
    providers: &Providers,
    config: &mut SessionConfig,
) -> Result<Arc<dyn Provider>> {
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
    Ok(provider::build_with_key(spec, key))
}

/// The human-readable form. Deliberately rough: this is not the feed, just a
/// window on the stream.
fn human(event: &AgentEvent) -> Option<String> {
    Some(match event {
        AgentEvent::TextDelta { text, .. } => text.clone(),
        AgentEvent::RouteDecided {
            route,
            provider,
            model,
            ..
        } => format!(
            "\x1b[2mМаршрут: {} → {provider}/{model}\x1b[0m",
            route.as_deref().unwrap_or("по умолчанию")
        ),
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
        AgentEvent::QuestionAsked { request } => {
            let options: Vec<String> = request
                .options
                .iter()
                .enumerate()
                .map(|(i, o)| format!("  {}. {}", i + 1, o.label))
                .collect();
            format!(
                "\n\x1b[33m? {}\x1b[0m\n\x1b[2m{}\x1b[0m",
                request.question,
                options.join("\n")
            )
        }
        AgentEvent::RetryScheduled {
            attempt,
            of,
            delay_ms,
            ..
        } => format!("\x1b[2mПовтор через {} с · попытка {attempt} из {of}\x1b[0m", delay_ms / 1000),
        AgentEvent::Error { code, message, .. } => format!("\x1b[31mОшибка {code}: {message}\x1b[0m"),
        AgentEvent::PlanProposed { plan } => format!("\n--- план ---\n{plan}\n"),
        AgentEvent::TextShown { path, text, .. } => format!("\n--- {path} ---\n{text}\n"),
        AgentEvent::FileChanged { fact, .. } => format!(
            "\x1b[2m  {} +{} −{}\x1b[0m",
            fact.path, fact.added, fact.removed
        ),
        AgentEvent::McpUpdated { servers } => {
            let строки: Vec<String> = servers
                .iter()
                .map(|s| {
                    let что = match s.status {
                        McpStatus::Connecting => "подключается".to_string(),
                        McpStatus::Ready => format!("готов · {} инстр.", s.tools.len()),
                        McpStatus::Failed => {
                            format!("сбой: {}", s.reason.as_deref().unwrap_or("причина неизвестна"))
                        }
                        McpStatus::Disabled => "выключен".to_string(),
                        McpStatus::NeedsApproval => {
                            format!("ждёт одобрения: yaca mcp --approve {}", s.name)
                        }
                        McpStatus::NeedsLogin => format!("нужен вход: yaca mcp --login {}", s.name),
                        McpStatus::LoggingIn => "идёт вход".to_string(),
                    };
                    format!("  {} · {что}", s.name)
                })
                .collect();
            format!("\n\x1b[2mMCP:\n{}\x1b[0m", строки.join("\n"))
        }
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