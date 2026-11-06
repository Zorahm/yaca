//! The `Decision` tool: the agent asks the small model of the Decision
//! provider a question and gets its answer back: typed probabilities from a
//! decision server, or a short text answer from a chat server.
//!
//! The agent uses it for judgments that do not need its own full reasoning —
//! which skill fits, which of three files to open, whether a request is a
//! question or an edit. The caller supplies the question, context and options;
//! the tool never guesses options from prose or asks a scoring model to write.
//!
//! The tool exists only in a session that has a model to ask
//! (`SessionConfig::decision`). A tool that could only answer "not configured"
//! is a schema in every request and a failed call the model learns to repeat,
//! so it is not offered at all — the same rule as `SearchMessage` without a
//! journal.
//!
//! What it says is advice. The description tells the model so, and the answer
//! is not trusted further than the model that wrote it deserves: it is small on
//! purpose, and it sees only the text it is given.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::{ProviderError, ToolError};
use crate::intent::Intent;
use crate::message::Message;
use crate::provider::systemone::SystemOne;
use crate::provider::{Provider, ProviderEvent, ProviderRequest};

mod question;
use question::Question;

/// Who answers: a provider that was built (its key from the keychain) and the
/// model on it.
#[derive(Clone)]
pub struct DecisionSetup {
    pub provider: Arc<dyn Provider>,
    pub model: String,
    /// The same server's typed-decision route, when the session is built from a
    /// real provider. Decision models score rather than chat, so Auto's
    /// reviewer and the Decision tool ask this first and fall back to
    /// `provider` only when the server has no such route.
    pub systemone: Option<Arc<SystemOne>>,
}

impl fmt::Debug for DecisionSetup {
    // The provider holds a client and a key; neither belongs in a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionSetup")
            .field("provider", &self.provider.name())
            .field("model", &self.model)
            .finish()
    }
}

/// What the small model is told about its job. It is a different model from
/// the agent's, with none of the agent's prompt, so this is all it knows.
const SYSTEM: &str = "You are a small, fast model that another AI agent consults for quick \
    judgments: picking one option out of several, classifying a request, a yes/no check. \
    You only see the question you are given; you cannot read files or run anything. \
    Answer briefly and directly. When options are listed, name exactly one of them first, \
    then give a reason in one sentence at most. If the question does not give you enough to \
    decide, say what is missing instead of guessing.";

/// A choice and a sentence of reason. A longer answer is the small model
/// wandering, and every token of it is spent again in each later request.
const MAX_TOKENS: u32 = 512;

/// A free-form answer takes longer than the router's classification, which has
/// a second or three; this is still a quick aside and not a task.
const TIMEOUT: Duration = Duration::from_secs(30);

/// The small model has a small window, and the question is a question, not a
/// file. A paste past this is cut and says so.
const MAX_QUESTION_CHARS: usize = 12_000;

pub struct Decision {
    setup: DecisionSetup,
    /// A field and not the constant alone, so a test can shorten it: waiting
    /// out the real deadline would make the test the slowest in the crate.
    timeout: Duration,
}

impl Decision {
    pub fn new(setup: DecisionSetup) -> Self {
        Decision {
            setup,
            timeout: TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl Tool for Decision {
    fn name(&self) -> &'static str {
        "Decision"
    }

    fn description(&self) -> &'static str {
        "Ask a small, fast side model for a quick judgment. Put the context in `state` and \
         the question in `question`. For a selection or classification, always supply `choices` \
         mapping option names to descriptions; otherwise ask a yes/no question. It cannot see \
         this conversation, the files or your tools. A decision server returns a probability \
         of yes or a choice with probabilities; a chat server returns a short text answer. \
         Its answer is advice: weigh it and verify before relying on it for anything that matters."
    }

    fn properties(&self) -> Value {
        json!({
            "question": {
                "type": "string",
                "description": "What to judge: a yes/no question, or a selection question \
                                with the options supplied separately in choices.",
            },
            "state": {
                "type": "string",
                "description": "Everything needed to judge: relevant context, facts or text. \
                                If omitted, the question itself is used as the context.",
            },
            "choices": {
                "type": "object",
                "minProperties": 2,
                "maxProperties": 255,
                "additionalProperties": { "type": "string" },
                "description": "For selection questions, map each option name to its description. \
                                Do not supply for yes/no questions. Example: \
                                {\"debugging\": \"Investigate a failing test\", \
                                \"design\": \"Design a visual layout\"}.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["question"]
    }

    fn narration(&self) -> Narration {
        // The row cannot tell from the call what was being decided, so the
        // model says it in its own words, as for Skill.
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        // Read-only and offered in Plan too: asking changes nothing.
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let question = Question::parse(&input)?;
        ctx.check_cancelled()?;

        let answer = tokio::select! {
            _ = ctx.cancel.cancelled() => return Err(ToolError::Cancelled),
            answer = tokio::time::timeout(self.timeout, self.answer(&question, ctx)) => answer,
        };

        match answer {
            Ok(Ok(text)) if !text.trim().is_empty() => Ok(ToolOutput::new(text.trim().to_string())),
            Ok(Ok(_)) => Err(unanswered("the model replied with nothing")),
            Ok(Err(e)) => Err(unanswered(&e.to_string())),
            Err(_) => Err(unanswered("it did not answer in time")),
        }
    }
}

impl Decision {
    async fn answer(&self, question: &Question, ctx: &ToolCtx) -> Result<String, ProviderError> {
        if let Some(client) = &self.setup.systemone {
            if let Some(reply) = client
                .ask(&self.setup.model, question.state(), question.typed())
                .await?
            {
                if let Some(ledger) = &ctx.aside_ledger {
                    ledger.add(question::usage(&reply), ctx.decision_pricing, None);
                }
                return question.read_answer(&reply);
            }
        }

        let request = ProviderRequest {
            model: self.setup.model.clone(),
            system: SYSTEM.to_string(),
            messages: vec![Message::user_text(question.chat())],
            // No tools: it answers from the text alone.
            tools: Vec::new(),
            max_tokens: MAX_TOKENS,
            effort: None,
            // Reasoning would make it slower and dearer than the agent's own
            // thought, which is the thing this tool exists to avoid.
            thinking: false,
        };

        collect_text(
            self.setup.provider.as_ref(),
            request,
            ctx,
            ctx.decision_pricing,
        )
        .await
    }
}

/// The text of the reply; thinking and other blocks are not part of the answer.
pub(crate) async fn collect_text(
    provider: &dyn Provider,
    request: ProviderRequest,
    ctx: &ToolCtx,
    pricing: crate::usage::Pricing,
) -> Result<String, ProviderError> {
    let mut stream = provider.stream(request).await?;
    let mut text = String::new();
    let mut usage = crate::usage::TokenCounts::default();
    let mut cost = None;
    while let Some(event) = stream.next().await {
        match event? {
            ProviderEvent::TextDelta { text: delta, .. } => text.push_str(&delta),
            ProviderEvent::MessageStart { usage: counts, .. }
            | ProviderEvent::MessageDelta { usage: counts, .. } => usage.add(counts),
            ProviderEvent::Cost { usd } => cost = Some(usd),
            ProviderEvent::AsideUsage {
                usage,
                pricing,
                cost,
            } => {
                if let Some(ledger) = &ctx.aside_ledger {
                    ledger.add(usage, pricing, cost);
                }
            }
            _ => {}
        }
    }
    if let Some(ledger) = &ctx.aside_ledger {
        ledger.add(usage, pricing, cost);
    }
    Ok(text)
}

/// A failure the agent can act on, with what to do next: a side question that failed must not stall
/// the work it was asked for.
fn unanswered(why: &str) -> ToolError {
    ToolError::failed_with(
        format!("Decision model did not answer: {why}"),
        format!(
            "The Decision model gave no answer ({why}). Decide it yourself and carry on; \
             ask the human only if the choice really matters."
        ),
    )
}

fn capped(question: &str) -> String {
    match question.char_indices().nth(MAX_QUESTION_CHARS) {
        // The byte offset where the first dropped character starts, so the
        // slice ends on a char boundary.
        Some((cut, _)) => format!("{}\n[question truncated]", &question[..cut]),
        None => question.to_string(),
    }
}

#[cfg(test)]
mod tests;
