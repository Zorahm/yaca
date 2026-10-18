//! The `Decision` tool: the agent asks the small model of the Decision
//! provider a question and gets its short answer back.
//!
//! The agent uses it for judgments that do not need its own full reasoning —
//! which skill fits, which of three files to open, whether a request is a
//! question or an edit. The question goes to the model as it is written; there
//! is no template, no fixed set of questions and no typed answer. The caller
//! knows what it is choosing between, and anything built in here would be a
//! guess about that.
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

/// Who answers: a provider that was built (its key from the keychain) and the
/// model on it.
#[derive(Clone)]
pub struct DecisionSetup {
    pub provider: Arc<dyn Provider>,
    pub model: String,
    /// The same server's typed-decision route, when the session is built from a
    /// real provider. Decision models score rather than chat, so Auto's
    /// reviewer asks this first and falls back to `provider` only when the
    /// server has no such route.
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
        "Ask a small, fast side model a question and get its short answer. Use it for quick \
         judgments that do not need your full reasoning: choosing one option from a list (which \
         skill, which file, which approach), classifying a request, or a yes/no check. Put \
         everything it needs into `question` — it cannot see this conversation, the files or \
         your tools, so list the options with a line about each. Its answer is advice, not a \
         result: weigh it, and verify before relying on it for anything that matters."
    }

    fn properties(&self) -> Value {
        json!({
            "question": {
                "type": "string",
                "description": "The question, with everything needed to answer it. When \
                                choosing, list the options and what each is for.",
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
        let question = input["question"]
            .as_str()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .ok_or_else(|| ToolError::failed("missing question"))?;
        ctx.check_cancelled()?;

        let request = ProviderRequest {
            model: self.setup.model.clone(),
            system: SYSTEM.to_string(),
            messages: vec![Message::user_text(capped(question))],
            // No tools: it answers from the text alone.
            tools: Vec::new(),
            max_tokens: MAX_TOKENS,
            effort: None,
            // Reasoning would make it slower and dearer than the agent's own
            // thought, which is the thing this tool exists to avoid.
            thinking: false,
        };

        let answer = tokio::select! {
            _ = ctx.cancel.cancelled() => return Err(ToolError::Cancelled),
            answer = tokio::time::timeout(
                self.timeout,
                collect_text(self.setup.provider.as_ref(), request),
            ) => answer,
        };

        match answer {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                Ok(ToolOutput::new(text.trim().to_string()))
            }
            Ok(Ok(_)) => Err(unanswered("the model replied with nothing")),
            Ok(Err(e)) => Err(unanswered(&e.to_string())),
            Err(_) => Err(unanswered("it did not answer in time")),
        }
    }
}

/// The text of the reply; thinking and other blocks are not part of the answer.
async fn collect_text(
    provider: &dyn Provider,
    request: ProviderRequest,
) -> Result<String, ProviderError> {
    let mut stream = provider.stream(request).await?;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        if let ProviderEvent::TextDelta { text: delta, .. } = event? {
            text.push_str(&delta);
        }
    }
    Ok(text)
}

/// A failure the agent can act on. The feed gets it in Russian, the model in
/// English with what to do next: a side question that failed must not stall
/// the work it was asked for.
fn unanswered(why: &str) -> ToolError {
    ToolError::failed_with(
        format!("Decision-модель не ответила: {why}"),
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
