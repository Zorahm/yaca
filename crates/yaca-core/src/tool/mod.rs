//! Тулы и реестр.
//!
//! Здесь живёт механика §5.4.1: кто пишет подпись действия. Тулы делятся на два
//! рода, и деление проведено в схеме вызова, а не в промпте.
//!
//! * [`Narration::Interface`] — подпись выводится из факта: создан файл,
//!   изменён, удалён, не удалось. Поля `description` в схеме нет вовсе, чтобы
//!   модель не тратила на него токены и не пыталась переписать факт.
//! * [`Narration::Model`] — подпись знает только модель: из `rg -n $'\x01' src/`
//!   интерфейс не поймёт, зачем это запускали. Поле `description` обязательно.

pub mod bash;
pub mod bash_output;
pub mod delete_dir;
pub mod delete_file;
pub mod edit_file;
pub mod exit_plan_mode;
pub(crate) mod files;
pub(crate) mod net;
pub mod glob;
pub mod grep;
pub(crate) mod page;
pub mod read_file;
pub mod read_image;
pub mod task;
pub mod todo_write;
pub mod web_fetch;
pub mod web_search;
pub mod write_file;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::change::{FileDiff, FileFact};
use crate::checkpoint::CheckpointReason;
use crate::error::ToolError;
use crate::event::{AgentEvent, EventSink, ToolMeta};
use crate::id::CallId;
use crate::intent::Intent;
use crate::permission::PermissionNeed;
use crate::picture::Picture;
use crate::process::Processes;
use crate::provider::ToolSchema;
use crate::session::subagent::Subagents;
use crate::workspace::Workspace;

/// Максимальная длина подписи из §5.4.1.
pub const DESCRIPTION_MAX_CHARS: usize = 60;

/// Кто подписывает действие в ленте.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Narration {
    Interface,
    Model,
}

/// Куда тул отдаёт вывод, пока ещё работает (§15, `ToolCallProgress`).
///
/// `call_id` зашит внутрь: тул физически не может дописать в чужую строку
/// ленты. Флаг «пользовались» решает, кто отдаёт текст: лента получает его
/// ровно один раз — либо по мере поступления, либо целиком, когда тул вернулся.
pub struct Progress {
    call_id: CallId,
    events: EventSink,
    used: AtomicBool,
}

impl Progress {
    pub fn new(call_id: CallId, events: EventSink) -> Self {
        Progress {
            call_id,
            events,
            used: AtomicBool::new(false),
        }
    }

    pub fn push(&self, chunk: impl Into<String>) {
        let chunk: String = chunk.into();
        if chunk.is_empty() {
            return;
        }
        self.used.store(true, Ordering::Relaxed);
        self.events.emit(AgentEvent::ToolCallProgress {
            call_id: self.call_id.clone(),
            chunk,
        });
    }

    /// Уходил ли вывод в ленту по дороге. Сессия смотрит сюда, чтобы не
    /// показать один и тот же текст дважды.
    pub fn used(&self) -> bool {
        self.used.load(Ordering::Relaxed)
    }
}

pub struct ToolCtx {
    pub ws: Arc<Workspace>,
    /// Фоновые команды сессии (§6.5).
    pub processes: Processes,
    pub events: EventSink,
    pub progress: Progress,
    pub cancel: CancellationToken,
    /// What `Task` starts a subagent with. `None` for a subagent's own calls,
    /// and so one does not hand its work to another.
    pub agents: Option<Subagents>,
    /// Whether the model that made the call can be shown an image.
    pub vision: bool,
}

impl ToolCtx {
    pub(crate) fn check_cancelled(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() {
            Err(ToolError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Изменение, легшее в рабочее дерево. Отсюда интерфейс берёт подпись
/// строки (§5.5) и дифф для её раскрытия (§5.5.1).
#[derive(Debug, Clone)]
pub struct FileChange {
    pub fact: FileFact,
    pub diff: FileDiff,
}

#[derive(Debug)]
pub struct ToolOutput {
    /// Что уедет модели как `tool_result`.
    pub text: String,
    /// Что покажет интерфейс в раскрытой строке и в счётчиках.
    pub meta: ToolMeta,
    /// Заполнено файловыми тулами. Событие по нему шлёт сессия — она одна
    /// знает `call_id`, и все события уходят из одного места.
    pub changed: Option<FileChange>,
    /// Images for the model, each under the name it is told. They reach it
    /// after the results of the batch: see [`ContentBlock::Image`].
    ///
    /// [`ContentBlock::Image`]: crate::message::ContentBlock::Image
    pub pictures: Vec<(String, Picture)>,
}

impl ToolOutput {
    pub fn new(text: String) -> Self {
        let lines = text.lines().count() as u32;
        ToolOutput {
            text,
            meta: ToolMeta {
                lines,
                ..Default::default()
            },
            changed: None,
            pictures: Vec::new(),
        }
    }

    pub fn picture(mut self, name: String, picture: Picture) -> Self {
        self.pictures.push((name, picture));
        self
    }

    pub fn changed(mut self, change: FileChange) -> Self {
        self.changed = Some(change);
        self
    }

    pub fn items(mut self, n: u32) -> Self {
        self.meta.items = Some(n);
        self
    }

    pub fn truncated(mut self, yes: bool) -> Self {
        self.meta.truncated = yes;
        self
    }

    /// Сколько строк ушло в ленту. Считается из `text` по умолчанию, но у
    /// стримящих тулов лента и модель видят разное: команда отдаёт человеку
    /// весь вывод, а модели — обрезанный.
    pub fn lines(mut self, n: u32) -> Self {
        self.meta.lines = n;
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;

    /// Описание тула для модели — не подпись действия.
    fn description(&self) -> &'static str;

    /// Свойства JSON Schema без служебного `description`: его добавляет реестр.
    fn properties(&self) -> Value;

    fn required(&self) -> &'static [&'static str];

    fn narration(&self) -> Narration;

    fn intents(&self) -> &'static [Intent];

    /// Может ли вызов идти одновременно с соседями по одной пачке (§5.4).
    ///
    /// Модель зовёт тулы пачкой ровно затем, чтобы они шли разом, и пять
    /// чтений подряд — это пять ожиданий вместо одного. Но одновременно
    /// можно не всем.
    ///
    /// `true` ставится тулу, который ничего не меняет и ни о чём не
    /// спрашивает. Пишущим нельзя: две правки одного файла — гонка вокруг
    /// инварианта свежести §8, где обе проверки проходят, а переживает одна,
    /// и чекпоинт «перед командой» (§5.8) перестаёт что-либо значить, если
    /// рядом бежит вторая команда. Спрашивающим нельзя тоже: §5.7 рисует один
    /// блок со счётчиком `2 из 3`, и двум вопросам разом там негде встать.
    fn concurrent(&self) -> bool {
        false
    }

    /// Первая строка раскрытой строки действия (§5.5.1): полный путь,
    /// полная команда, полный URL.
    fn target(&self, _input: &Value) -> Option<String> {
        None
    }

    /// Разрешение, которое нужно этому вызову (§8).
    ///
    /// `None` — действие спрашивать не о чем ни в одном режиме доверия.
    /// Считается до запуска тула: вопрос должен быть решён раньше, чем
    /// что-либо произойдёт. Кривые аргументы дают `None` — про них
    /// понятнее скажет сам тул.
    ///
    /// Берёт воркспейс, а не весь [`ToolCtx`]: вопрос задаётся до вызова, и
    /// собирать ради него ленту, процессы и токен отмены значило бы строить
    /// контекст, которым никто не воспользуется.
    fn permission(&self, _ws: &Workspace, _input: &Value) -> Option<PermissionNeed> {
        None
    }

    /// The snapshot this call needs taken before it runs (§5.8).
    ///
    /// `None` for file edits: the point taken before the message (§5.8) already
    /// covers everything the turn writes, and that is the granularity `Esc Esc`
    /// rolls back to. A divider per edited file would make the feed a list of
    /// snapshots instead of a conversation.
    ///
    /// Asked after the permission gate and answered before the tool starts, for
    /// the same reason §5.7 is: what cannot be undone must be dealt with while
    /// there is still nothing to undo.
    fn checkpoint(&self, _input: &Value) -> Option<CheckpointReason> {
        None
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError>;
}

/// Текст, который системный промпт вставляет в описание поля `description`.
const DESCRIPTION_FIELD_DOC: &str = "Single line, up to 60 characters, no trailing period: \
what was done or discovered, in past tense first person, in the conversation's language. \
Focus on the substance, not the tool: 'Found existing clipboard usage' \
rather than 'Ran rg clipboard src/'. No tool names and no command flags.";

pub struct ToolRegistry {
    tools: BTreeMap<&'static str, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        ToolRegistry {
            tools: tools.into_iter().map(|t| (t.name(), t)).collect(),
        }
    }

    /// Полный набор ядра.
    pub fn standard() -> Self {
        Self::new(vec![
            Arc::new(read_file::ReadFile),
            Arc::new(read_image::ReadImage),
            Arc::new(glob::Glob),
            Arc::new(grep::Grep::new()),
            Arc::new(web_fetch::WebFetch),
            Arc::new(web_search::WebSearch::default()),
            Arc::new(todo_write::TodoWrite),
            Arc::new(exit_plan_mode::ExitPlanMode),
            Arc::new(write_file::WriteFile),
            Arc::new(edit_file::EditFile),
            Arc::new(delete_file::DeleteFile),
            Arc::new(delete_dir::DeleteDir),
            Arc::new(delete_file::MoveFile),
            Arc::new(bash::Bash),
            Arc::new(bash_output::BashOutput),
            Arc::new(task::Task),
        ])
    }

    /// What a subagent works with: everything the agent has, less what
    /// belongs to the agent alone. `Task`, because the agent is the one who
    /// splits the work; the task list and the way out of plan mode, because
    /// §5.6 and §7.1 draw one of each and it is the agent's.
    pub fn delegated() -> Self {
        let mut all = Self::standard();
        for own in ["Task", "TodoWrite", "ExitPlanMode"] {
            all.tools.remove(own);
        }
        all
    }

    /// Тул, если он вообще есть в этом режиме.
    ///
    /// Режим тут не украшение: §7.1 не отдаёт модели записывающие тулы в
    /// «Плане», и придуманный ею `write_file` обязан получить «неизвестный
    /// инструмент», а не запрос разрешения. Спрашивать про действие, которого
    /// в этом режиме не существует, — интерфейс, врущий про состояние (§0).
    pub fn get(&self, name: &str, intent: Intent) -> Option<Arc<dyn Tool>> {
        self.tools
            .get(name)
            .filter(|t| intent.allows(t.intents()))
            .cloned()
    }

    /// Схемы для запроса к модели.
    ///
    /// Порядок детерминирован (`BTreeMap`): схемы идут первым куском запроса,
    /// и перетасовка списка обнуляла бы кеш промпта на каждом ходу.
    pub fn schemas(&self, intent: Intent) -> Vec<ToolSchema> {
        self.tools
            .values()
            .filter(|t| intent.allows(t.intents()))
            .map(|t| {
                let mut props = t.properties();
                let mut required: Vec<Value> = t.required().iter().map(|s| json!(s)).collect();

                if t.narration() == Narration::Model {
                    if let Some(obj) = props.as_object_mut() {
                        obj.insert(
                            "description".into(),
                            json!({
                                "type": "string",
                                "maxLength": DESCRIPTION_MAX_CHARS,
                                "description": DESCRIPTION_FIELD_DOC,
                            }),
                        );
                    }
                    required.push(json!("description"));
                }

                ToolSchema {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": props,
                        "required": Value::Array(required),
                        "additionalProperties": false,
                    }),
                }
            })
            .collect()
    }
}

/// Вынимает подпись модели из аргументов и приводит её к форме §5.4.1.
///
/// Нарушение длины режется, а не роняет турн: подпись — украшение ленты,
/// и терять из-за неё работу тула нельзя.
pub fn take_description(input: &mut Value) -> Option<String> {
    let raw = input
        .as_object_mut()
        .and_then(|o| o.remove("description"))?
        .as_str()?
        .to_string();

    let one_line = raw.split('\n').next().unwrap_or("").trim();
    let trimmed = one_line.trim_end_matches(['.', '。']).trim();
    if trimmed.is_empty() {
        return None;
    }

    let cut: String = if trimmed.chars().count() > DESCRIPTION_MAX_CHARS {
        trimmed
            .chars()
            .take(DESCRIPTION_MAX_CHARS - 1)
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…"
    } else {
        trimmed.to_string()
    };
    Some(cut)
}

/// Общая обвязка тестов тулов: настоящий каталог, настоящий воркспейс,
/// живая шина событий. Моков здесь нет намеренно — тул, который врёт про
/// файловую систему, ловится только на файловой системе.
/// Общая обвязка тестов тулов: настоящий каталог, настоящий воркспейс,
/// живая шина событий. Моков здесь нет намеренно — тул, который врёт про
/// файловую систему, ловится только на файловой системе.
#[cfg(test)]
pub(crate) mod tests_support {
    use std::sync::Arc;

    use tokio::sync::broadcast;

    use super::{Progress, ToolCtx};
    use crate::event::{AgentEvent, EventSink};
    use crate::id::CallId;
    use crate::process::Processes;
    use crate::testutil::TempDir;
    use crate::workspace::Workspace;

    fn build(dir: &TempDir, tx: broadcast::Sender<AgentEvent>) -> (ToolCtx, Arc<Workspace>) {
        let ws = Arc::new(Workspace::open(dir.path()).unwrap());
        let events = EventSink::new(tx);
        let ctx = ToolCtx {
            processes: Processes::new(events.clone()),
            progress: Progress::new(CallId::new(), events.clone()),
            ws: ws.clone(),
            events,
            cancel: tokio_util::sync::CancellationToken::new(),
            agents: None,
            vision: true,
        };
        (ctx, ws)
    }

    pub fn ctx_for(dir: &TempDir) -> (ToolCtx, Arc<Workspace>) {
        let (tx, _rx) = broadcast::channel(64);
        build(dir, tx)
    }

    pub fn ctx_with_events(dir: &TempDir) -> (ToolCtx, broadcast::Receiver<AgentEvent>) {
        let (tx, rx) = broadcast::channel(64);
        let (ctx, _ws) = build(dir, tx);
        (ctx, rx)
    }
}

#[cfg(test)]
mod tests;
