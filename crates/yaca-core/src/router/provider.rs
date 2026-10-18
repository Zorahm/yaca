//! The router as a [`Provider`]. See [`super`].
//!
//! The session sees one provider with one virtual model. Behind it each user
//! message is routed once: the decision model answers the signals, the rules
//! pick a target, and the request goes to that target's backend under the
//! target's own model id. Whatever the target cannot take (effort, thinking,
//! an output longer than it allows) is dropped here, so the session never has
//! to know which real model answered.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::stream::{self, StreamExt};

use crate::error::{CoreError, ProviderError};
use crate::message::{strip_thinking, ContentBlock, Message, Role};
use crate::provider::registry::{ProviderSpec, Providers};
use crate::provider::{Provider, ProviderEvent, ProviderRequest, ProviderStream};
use crate::router::config::{ModelRef, RoutingConfig};
use crate::router::decision_model::ask;
use crate::router::file;
use crate::router::rules;
use crate::router::ROUTER_ID;
use crate::usage::ModelInfo;

/// How many conversations keep their decision. The map is cleared when a new
/// conversation would exceed it. A cleared conversation decides again on its
/// next request, which mid-turn would switch model under a tool loop; a session
/// process holds far fewer conversations than this, so the clear is a bound
/// against growth and not a path that normally runs.
const MAX_CONVERSATIONS: usize = 64;

/// What identifies the turn a decision was made for: how many user messages
/// carry text, and a hash of the last one. Tool-loop requests do not add to
/// either, so they keep the decision; a new message changes it.
type TurnKey = (usize, u64);

/// A provider that picks a real model per user message.
pub struct RouterProvider {
    cfg: RoutingConfig,
    /// Every model the config can send to, each once. The default is first.
    targets: Vec<Target>,
    /// Index of the default in `targets`.
    default: usize,
    decision: Arc<dyn Provider>,
    /// Each decision's name and the index of its model in `targets`.
    decisions: Vec<DecisionRoute>,
    /// The decision each conversation holds, keyed by the hash of its first
    /// user message. Keyed by conversation rather than by session so that a
    /// subagent's requests, which interleave with the agent's, do not replace
    /// the agent's decision.
    sticky: Mutex<HashMap<u64, Sticky>>,
}

struct Target {
    model: ModelRef,
    /// With `provider_id` set. This is the copy `targets()` hands out.
    info: ModelInfo,
    backend: Arc<dyn Provider>,
}

struct DecisionRoute {
    name: String,
    target: usize,
}

/// The outcome of a decision: which route, why, and where the request goes.
#[derive(Clone)]
struct Pick {
    /// The matched decision's name; `None` for the default.
    route: Option<String>,
    /// The signals the winning rule read; empty for the default.
    matched: Vec<String>,
    /// Index into `RouterProvider::targets`.
    target: usize,
}

#[derive(Clone)]
struct Sticky {
    turn: TurnKey,
    pick: Pick,
}

impl RouterProvider {
    /// Validates `cfg` against `providers`, builds one backend per provider the
    /// config refers to, and returns the router with the virtual model to offer.
    pub fn new(
        cfg: RoutingConfig,
        providers: &Providers,
        build: impl Fn(&ProviderSpec) -> Arc<dyn Provider>,
    ) -> Result<(Self, ModelInfo), CoreError> {
        file::validate(&cfg, providers)?;
        let mut pool = Pool {
            providers,
            build,
            backends: BTreeMap::new(),
            targets: Vec::new(),
        };
        let decision = pool.backend(&cfg.decision_model.model.provider)?;
        let default = pool.target(&cfg.default)?;
        let mut decisions = Vec::with_capacity(cfg.decisions.len());
        for decision in &cfg.decisions {
            decisions.push(DecisionRoute {
                name: decision.name.clone(),
                target: pool.target(&decision.model)?,
            });
        }
        let Pool { targets, .. } = pool;

        let offered = virtual_model(&cfg.name, &targets, default)?;
        let router = RouterProvider {
            cfg,
            targets,
            default,
            decision,
            decisions,
            sticky: Mutex::new(HashMap::new()),
        };
        Ok((router, offered))
    }

    /// [`RouterProvider::new`] with the real backends, whose keys come from the
    /// keychain.
    pub fn from_providers(
        cfg: RoutingConfig,
        providers: &Providers,
    ) -> Result<(Self, ModelInfo), CoreError> {
        Self::new(cfg, providers, |spec| crate::provider::build(spec.clone()))
    }

    /// Every real model the config can route to, with `provider_id` set. The
    /// session prices a request by the one that answered, so it needs them.
    pub fn targets(&self) -> Vec<ModelInfo> {
        self.targets.iter().map(|t| t.info.clone()).collect()
    }

    fn fallback(&self) -> Pick {
        Pick {
            route: None,
            matched: Vec::new(),
            target: self.default,
        }
    }

    /// Asks the decision model and applies the rules. Every failure of the
    /// model reaches here as unknown answers (see `ask`), so a broken model
    /// yields the default and never fails the turn.
    async fn decide(&self, text: &str) -> Pick {
        let timeout = Duration::from_millis(u64::from(self.cfg.decision_model.timeout_ms));
        let answers = ask(
            self.decision.as_ref(),
            &self.cfg.decision_model.model.model,
            &self.cfg.signals,
            text,
            timeout,
        )
        .await;
        let Some(matched) = rules::pick(&self.cfg, &answers) else {
            return self.fallback();
        };
        // Decision names are unique (`file::validate` refuses a repeat), so the
        // name identifies the route. The miss below cannot happen: the matched
        // decision is one of `cfg.decisions`, which `new` walked.
        match self
            .decisions
            .iter()
            .find(|d| d.name == matched.decision.name)
        {
            Some(route) => Pick {
                route: Some(route.name.clone()),
                matched: matched.signals,
                target: route.target,
            },
            None => self.fallback(),
        }
    }

    fn remember(&self, conversation: u64, sticky: Sticky) {
        let mut map = self.sticky.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= MAX_CONVERSATIONS && !map.contains_key(&conversation) {
            map.clear();
        }
        map.insert(conversation, sticky);
    }

    /// Forwards the request to the picked target and reports the pick first.
    async fn send(
        &self,
        mut req: ProviderRequest,
        pick: Pick,
        strip: bool,
    ) -> Result<ProviderStream, ProviderError> {
        let target = self.targets.get(pick.target).ok_or_else(|| {
            ProviderError::Config(
                "маршрут ведёт на модель, которой нет в списке маршрутизатора".into(),
            )
        })?;
        if strip {
            // Reasoning made by another model carries a signature the target
            // refuses, so it goes before the request leaves.
            strip_thinking(&mut req.messages);
        }
        let forwarded = ProviderRequest {
            model: target.model.model.clone(),
            system: req.system,
            messages: req.messages,
            tools: req.tools,
            // The session sized the output for the largest model it knows; a
            // smaller target would refuse a request above its own limit.
            max_tokens: req.max_tokens.min(target.info.max_output),
            // Sent to a model that does not know them, these are a 400.
            effort: if target.info.supports_effort {
                req.effort
            } else {
                None
            },
            thinking: req.thinking && target.info.supports_thinking,
        };
        let inner = target.backend.stream(forwarded).await?;
        // Before the target's own events: the session shows the model ahead of
        // what it wrote, and prices the request by it.
        let routed = ProviderEvent::Routed {
            route: pick.route,
            provider: target.model.provider.clone(),
            model: target.model.model.clone(),
            matched: pick.matched,
        };
        Ok(Box::pin(
            stream::iter([Ok::<_, ProviderError>(routed)]).chain(inner),
        ))
    }
}

#[async_trait]
impl Provider for RouterProvider {
    fn name(&self) -> &str {
        ROUTER_ID
    }

    async fn stream(&self, req: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        // A router must not guess where a foreign model id belongs: the session
        // only ever asks for the virtual one.
        if req.model != self.cfg.name {
            return Err(ProviderError::Config(format!(
                "модель «{}» не принадлежит маршрутизатору «{}»: он выбирает настоящую модель сам",
                req.model, self.cfg.name
            )));
        }

        let texts: Vec<String> = req.messages.iter().filter_map(user_text).collect();
        let (Some(first), Some(last)) = (texts.first(), texts.last()) else {
            // No user text anywhere: there is nothing to decide from, so the
            // default answers and nothing is remembered.
            return self.send(req, self.fallback(), false).await;
        };
        let conversation = hash_text(first);
        let turn: TurnKey = (texts.len(), hash_text(last));

        // The guard is a temporary: it is released before any await below.
        let previous = self
            .sticky
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&conversation)
            .cloned();

        let (pick, strip) = match &previous {
            Some(sticky) if sticky.turn == turn => (sticky.pick.clone(), false),
            _ => {
                let pick = self.decide(last).await;
                // Thinking in the history was made by the previous target. With
                // no record of which that was, it is treated as another model's:
                // a signature from the wrong model is refused, a lost reasoning
                // only costs depth.
                let strip = match &previous {
                    Some(sticky) => sticky.pick.target != pick.target,
                    None => true,
                };
                self.remember(
                    conversation,
                    Sticky {
                        turn,
                        pick: pick.clone(),
                    },
                );
                (pick, strip)
            }
        };
        self.send(req, pick, strip).await
    }
}

/// Builds each backend once and each real model once, from the references in
/// the config.
struct Pool<'a, F> {
    providers: &'a Providers,
    build: F,
    backends: BTreeMap<String, Arc<dyn Provider>>,
    targets: Vec<Target>,
}

impl<'a, F: Fn(&ProviderSpec) -> Arc<dyn Provider>> Pool<'a, F> {
    fn spec(&self, provider: &str) -> Result<&'a ProviderSpec, CoreError> {
        self.providers
            .get(provider)
            .ok_or_else(|| CoreError::Config(format!("провайдера «{provider}» нет в списке")))
    }

    fn backend(&mut self, provider: &str) -> Result<Arc<dyn Provider>, CoreError> {
        if let Some(backend) = self.backends.get(provider) {
            return Ok(Arc::clone(backend));
        }
        let backend = (self.build)(self.spec(provider)?);
        self.backends
            .insert(provider.to_string(), Arc::clone(&backend));
        Ok(backend)
    }

    /// The index of `model` among the targets, adding it on first mention.
    fn target(&mut self, model: &ModelRef) -> Result<usize, CoreError> {
        if let Some(index) = self.targets.iter().position(|t| t.model == *model) {
            return Ok(index);
        }
        let spec = self.spec(&model.provider)?;
        let mut info = spec.model(&model.model).cloned().ok_or_else(|| {
            CoreError::Config(format!(
                "у провайдера «{}» нет модели «{}»",
                spec.id, model.model
            ))
        })?;
        info.provider_id = spec.id.clone();
        let backend = self.backend(&model.provider)?;
        self.targets.push(Target {
            model: model.clone(),
            info,
            backend,
        });
        Ok(self.targets.len() - 1)
    }
}

/// The model the session is offered. Its window and output are the smallest
/// of the targets, because the target is unknown until a request arrives; a
/// capability is offered when any target has it, and the router strips it per
/// target. Images are the exception: `ModelInfo::supports_images` is a yes
/// unless proven otherwise, so one capable target is enough to offer them, and
/// a blind target refuses one request which the session then resends without
/// images.
///
/// Pricing is the default's. The session does not price by this entry: it
/// prices each request by the real model that answered, from `targets()`.
/// The decision model is not a target (the conversation never reaches it), so
/// it does not constrain the window.
fn virtual_model(name: &str, targets: &[Target], default: usize) -> Result<ModelInfo, CoreError> {
    let default = targets
        .get(default)
        .ok_or_else(|| CoreError::Config("маршрутизатор без маршрута по умолчанию".into()))?;
    let smallest = targets
        .iter()
        .min_by_key(|t| t.info.context_max)
        .unwrap_or(default);
    Ok(ModelInfo {
        id: name.to_string(),
        provider_id: ROUTER_ID.to_string(),
        display_name: "Авто".into(),
        context_max: smallest.info.context_max,
        // The window's source travels with the number it gave: a number that is
        // a guess stays a guess, even when it is the one the session uses.
        context_source: smallest.info.context_source,
        max_output: targets
            .iter()
            .map(|t| t.info.max_output)
            .min()
            .unwrap_or(default.info.max_output),
        pricing: default.info.pricing,
        supports_effort: targets.iter().any(|t| t.info.supports_effort),
        supports_thinking: targets.iter().any(|t| t.info.supports_thinking),
        supports_images: targets.iter().any(|t| t.info.supports_images),
    })
}

/// The text of a user message: its text blocks, joined. `None` for a message
/// without any, such as a batch of tool results. That is the tool loop talking,
/// not a new message from the human.
fn user_text(message: &Message) -> Option<String> {
    if message.role != Role::User {
        return None;
    }
    let parts: Vec<&str> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;
