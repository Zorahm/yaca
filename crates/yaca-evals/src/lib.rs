//! Real-task evals for the Yaca core.
//!
//! The golden tests in `yaca-core` check the *shape* of the event stream
//! against a scripted model. They say nothing about whether the agent solves
//! anything, and nothing about how a prompt or tool change moves that. This
//! crate runs the same core against a real model on small tasks and checks the
//! resulting tree, so such a change shows up as a number.
//!
//! # Layout
//!
//! ```text
//! evals/tasks/<id>/
//!     task.json   the prompt, the mode, an optional model and timeout, and the check
//!     repo/       the starting tree; copied into a fresh temp dir for every run
//! ```
//!
//! `task.json`:
//!
//! ```json
//! {
//!   "prompt": "The tests in tests/ fail. Make them pass.",
//!   "mode": "agent",
//!   "model": "claude-sonnet-5",
//!   "timeoutSecs": 600,
//!   "check": { "command": "cargo test --quiet", "cwd": "." },
//!   "expect": [
//!     { "path": "src/lib.rs", "contains": "fn median" },
//!     { "path": "old.rs", "absent": true }
//!   ]
//! }
//! ```
//!
//! `model` and `timeoutSecs` are optional. `mode` is `agent` (the full toolset)
//! or `plan`. At least one of `check` and `expect` is required: a task with
//! nothing to verify it would be scored as solved by whatever the model did.
//! The check decides: exit 0 is solved. `expect` entries are plain file
//! assertions, for when a whole toolchain is more than the task needs. When
//! both are given, both must pass.
//!
//! # Modules
//!
//! * [`task`]: reads and validates `task.json` and `repo/`.
//! * [`workdir`]: unique temp directories and the tree copy.
//! * [`check`]: the `expect` assertions and the check command.
//! * [`observe`]: folds the event stream into counts, tokens and cost.
//! * [`run`]: drives one session through a task and produces a [`result::TaskResult`].
//! * [`result`]: the per-task and summary JSON, and the table printed to stdout.
//! * [`provider`]: builds the real provider the same way `yaca-cli` does.

pub mod check;
pub mod observe;
pub mod provider;
pub mod result;
pub mod run;
pub mod task;
pub mod workdir;
