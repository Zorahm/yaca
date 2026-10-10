//! The worker's side of compaction (§5.11): when to compact, and doing it.
//!
//! Where to cut and what the summarizer reads is `compact`'s business. This is
//! the part that owns the history and talks to the provider.
//!
//! On its own, compaction happens before every request, not only before the
//! human's next message. An agent working through a task spends one turn on
//! dozens of calls, and the system prompt tells it that the length of a
//! conversation is never a reason to stop — so the window has to be looked
//! after in the middle of that turn too, and a request the provider refuses as
//! too long is compacted and sent again (`turn`).

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::event::AgentEvent;
use crate::id::TurnId;
use crate::message::Message;
use crate::provider::{ProviderEvent, ProviderRequest};

use super::compact;
use super::turn::Worker;

/// A summary is a page, not a chapter. §5.11 shows it inside a divider, and a
/// summary that fills the context it freed has done nothing.
const SUMMARY_MAX_TOKENS: u32 = 8_000;

/// Characters per token for what the provider has not measured yet. Low on
/// purpose: code and Russian text come out denser than English prose, and an
/// estimate that errs towards full compacts a step early rather than a request
/// too late.
const CHARS_PER_TOKEN: usize = 3;

impl Worker {
    /// §6.1.1 says the context will be compacted on its own. This is where.
    ///
    /// The window has to hold the answer as well as the request: providers
    /// refuse a request whose input and `max_tokens` do not fit together. So
    /// the line is drawn at a share of what the window leaves for input, not of
    /// the whole window — with a 64k answer in a 200k window, 90% of the whole
    /// is already past the point of refusal.
    pub(super) async fn compact_if_full(&mut self, turn: &TurnId, cancel: &CancellationToken) {
        let room = self.context_max().saturating_sub(self.answer_room());
        // A window no bigger than the answer it has to hold: nothing a summary
        // could fix, and compacting at every step would not change that.
        if room == 0 {
            return;
        }
        let limit = (room as f32 * compact::AUTO_RATIO) as u32;

        // Checked before compacting rather than letting `compact` complain:
        // nobody pressed anything, so there is nothing to answer.
        if self.context_needed() >= limit && compact::plan(&self.history).is_some() {
            self.compact(cancel, Some(turn)).await;
        }
    }

    /// How many tokens the history takes in the next request.
    ///
    /// What the provider measured with its last answer, plus an estimate for
    /// what came after it: the results of the calls in that answer, a message
    /// from the human. Mid-turn that is exactly what fills the window — a large
    /// file read, a long build log — and waiting for the provider to measure it
    /// means sending the request that does not fit.
    fn context_needed(&self) -> u32 {
        let measured = self.usage.snapshot(0).context_used;
        let since = self.history.get(self.measured..).unwrap_or_default();
        let estimated = compact::weight(since) / CHARS_PER_TOKEN;
        measured.saturating_add(u32::try_from(estimated).unwrap_or(u32::MAX))
    }

    /// «Сжать» (§5.11). Says whether the history was replaced.
    ///
    /// `turn` is the turn it happens inside, if any. The replaced history is
    /// written down under it, so that a copy made up to that turn (§9) keeps
    /// the rest of the turn too; between turns it gets a name of its own.
    pub(super) async fn compact(&mut self, cancel: &CancellationToken, turn: Option<&TurnId>) -> bool {
        let Some(plan) = compact::plan(&self.history) else {
            // A button that silently does nothing is an interface lying about
            // state (§0), so this is said out loud even though nothing broke.
            self.emit(AgentEvent::Error {
                code: "nothing_to_compact".into(),
                message: "the conversation is still too short to compact".into(),
                retryable: false,
            });
            return false;
        };

        // Measured before the summarizing request, which is a conversation of
        // its own and would otherwise be the thing the ring reports.
        let context_max = self.context_max();
        let before = self.usage.snapshot(context_max).context_used;

        let summary = match self.summarize(&plan, cancel).await {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => {
                self.emit(AgentEvent::Error {
                    code: "compact_failed".into(),
                    message: "the summarizing model returned nothing".into(),
                    retryable: true,
                });
                return false;
            }
            Err(err) => {
                self.emit(AgentEvent::Error {
                    code: err.code(),
                    message: err.to_string(),
                    retryable: err.is_retryable(),
                });
                return false;
            }
        };

        let freed = compact::freed(before, &plan, summary.chars().count());

        self.history = compact::rebuilt(&summary, plan.tail);
        self.compacted = true;
        // The provider's figure less what was freed now stands for the whole
        // history; the next answer measures it properly.
        self.measured = self.history.len();
        self.persist_history(turn);

        self.usage.shrank_by(freed);
        self.emit(AgentEvent::ContextCompacted { summary, freed });
        // The ring and §6.1.1's bar both read the context, and both would go
        // on insisting it is full until the next answer without this.
        self.emit(AgentEvent::UsageUpdated {
            usage: self.usage.snapshot(context_max),
        });
        true
    }

    /// §5.11: the summary can be rewritten before the conversation continues.
    pub(super) fn edit_summary(&mut self, text: String) {
        if !self.compacted || self.history.is_empty() {
            return self.emit(AgentEvent::Error {
                code: "no_summary".into(),
                message: "there is no summary to edit".into(),
                retryable: false,
            });
        }
        let tail = self.history.split_off(1);
        self.history = compact::rebuilt(&text, tail);
        self.measured = self.history.len();
        self.persist_history(None);
        self.emit(AgentEvent::SummaryEdited { summary: text });
    }

    fn persist_history(&self, turn: Option<&TurnId>) {
        if let Some(recorder) = &self.shared.recorder {
            // Between turns nobody spoke, but the journal is keyed by turn, and
            // the replacement needs a name of its own to be cut at later.
            let turn = turn.cloned().unwrap_or_else(TurnId::new);
            recorder.replaced_history(&turn, &self.history);
        }
    }

    pub(super) fn context_max(&self) -> u32 {
        let model = self.shared.state.lock().unwrap().model.clone();
        self.shared.model_info(&model).context_max
    }

    /// Asks a model to write the record the agent will wake up with.
    ///
    /// A conversation of exactly one message: the transcript. No tools, no
    /// thinking, no effort — §8 promises this works with a weak local model
    /// too, and the fewer features a request leans on, the more models answer
    /// it.
    async fn summarize(
        &mut self,
        plan: &compact::Plan,
        cancel: &CancellationToken,
    ) -> Result<String, ProviderError> {
        let model = {
            let state = self.shared.state.lock().unwrap();
            self.shared
                .settings
                .compact_model_or(&state.model)
                .to_string()
        };
        let info = self.shared.model_info(&model);

        let request = ProviderRequest {
            model,
            system: compact::SYSTEM.to_string(),
            messages: vec![Message::user_text(plan.transcript.clone())],
            tools: Vec::new(),
            max_tokens: info.max_output.min(SUMMARY_MAX_TOKENS),
            effort: None,
            thinking: false,
        };

        let mut stream = self.open_stream(request, cancel).await?;
        let mut summary = String::new();
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(ProviderError::Network("сжатие отменено".into()))
                }
                item = stream.next() => match item {
                    None => break,
                    Some(Ok(ProviderEvent::TextDelta { text, .. })) => summary.push_str(&text),
                    Some(Ok(ProviderEvent::MessageStart { usage, .. })) => {
                        self.usage.record_aside(usage, info.pricing);
                    }
                    // The summary costs money like anything else, and §7.1
                    // shows the money.
                    Some(Ok(ProviderEvent::MessageDelta { usage, .. })) => {
                        self.usage.record_aside(usage, info.pricing);
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(err),
                },
            }
        }
        Ok(summary)
    }
}
