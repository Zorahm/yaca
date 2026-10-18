//! The router: picks a model per user message from typed questions about it.
//!
//! LiteLLM's routing stays in Python and its Rust core only translates
//! requests, so routing lives here, behind the same [`Provider`] boundary the
//! session already uses: the session sees one provider with one virtual model
//! and nothing else changes.
//!
//! Pipeline, in the order a turn takes it:
//! 1. [`decision_model`] asks the configured decision model every signal's
//!    question in one call and returns [`config::Answers`].
//! 2. [`rules`] evaluates the decisions over the answers and names the winner.
//! 3. [`provider::RouterProvider`] sends the request to the winner's provider
//!    (or the default) and reports the choice.
//!
//! The decision is made once per user message and held for the whole tool
//! loop: a thinking signature is bound to its model, and a prompt cache is
//! too, so switching mid-turn would break both.
//!
//! Not done, and said here instead of declared and ignored: the `span`, `set`
//! and `score` question types, the `decision` selection algorithm (a model
//! picking among candidates), and real probabilities from token logprobs —
//! until then the model reports its own confidence in the answer.
//!
//! [`Provider`]: crate::provider::Provider

pub mod attach;
pub mod config;
pub mod decision_model;
pub mod file;
pub mod provider;
pub mod question;
pub mod rules;

/// The provider id the router is offered under. Not an entry of
/// `providers.json`: it is made of them.
pub const ROUTER_ID: &str = "router";
