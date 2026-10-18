//! Reading, writing and checking `routing.json`. See [`super`].
//!
//! Application-level, next to `providers.json`, for the same reason: a project
//! does not get to decide where a conversation is sent. A file that does not
//! parse is an error, not a fallback to "no routing". A broken file read as
//! "not configured" would quietly send every conversation to whatever the
//! switcher happened to hold, and the human would not learn why.
//!
//! [`load`] only parses. Checking the config needs the providers it points at,
//! which the caller holds, so [`validate`] is a separate step the caller runs
//! after loading and before the router is offered.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::error::CoreError;
use crate::provider::registry::Providers;
use crate::router::config::{Condition, ModelRef, Question, Rule, RoutingConfig, Signal};
use crate::router::ROUTER_ID;

pub const FILE: &str = "routing.json";

/// `Ok(None)` when the file does not exist: routing is simply not configured.
pub fn load(dir: &Path) -> Result<Option<RoutingConfig>, CoreError> {
    let path = dir.join(FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(CoreError::Io(e)),
    };
    parse(&text, &path.display().to_string()).map(Some)
}

/// Parses the text of a `routing.json`, `source` naming where it came from in
/// the error. The window checks what is typed into its editor through here, so
/// a file and an edit are refused by the same rule and with the same words.
pub fn parse(text: &str, source: &str) -> Result<RoutingConfig, CoreError> {
    serde_json::from_str(text)
        .map_err(|e| CoreError::Config(format!("{source} failed to parse: {e}")))
}

/// Written through a temporary file, as `providers.json` is, so a crash
/// mid-write cannot leave a file that stops the next launch.
pub fn save(dir: &Path, cfg: &RoutingConfig) -> Result<(), CoreError> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(cfg)?;
    let temp = dir.join(format!("{FILE}.tmp"));
    fs::write(&temp, text)?;
    fs::rename(temp, dir.join(FILE))?;
    Ok(())
}

/// Checks the config against the providers it routes to.
///
/// Every error names the offending thing, so the human can find it in the file
/// without reading the code.
pub fn validate(cfg: &RoutingConfig, providers: &Providers) -> Result<(), CoreError> {
    check(cfg, providers).map_err(|message| CoreError::Config(format!("routing.json: {message}")))
}

/// The checks themselves, with plain messages: [`validate`] adds the file's name
/// once, so a nested problem does not carry two prefixes.
fn check(cfg: &RoutingConfig, providers: &Providers) -> Result<(), String> {
    if cfg.name.trim().is_empty() {
        return Err("the router has no name".into());
    }
    if cfg.decision_model.timeout_ms == 0 {
        return Err("decision model response time is 0 ms: questions would not get an answer in time".into());
    }

    let signals = signals_by_name(&cfg.signals)?;
    check_model_ref(&cfg.decision_model.model, providers, "model that answers questions")?;
    check_model_ref(&cfg.default, providers, "default route")?;

    let mut names = BTreeSet::new();
    for decision in &cfg.decisions {
        let place = format!("decision «{}»", decision.name);
        if !names.insert(decision.name.as_str()) {
            return Err(format!("{place} is described twice"));
        }
        check_rule(&decision.rules, &signals).map_err(|m| format!("{place}: {m}"))?;
        check_model_ref(&decision.model, providers, &place)?;
    }
    Ok(())
}

fn signals_by_name(signals: &[Signal]) -> Result<BTreeMap<&str, &Signal>, String> {
    let mut by_name = BTreeMap::new();
    for signal in signals {
        check_signal(signal)?;
        if by_name.insert(signal.name.as_str(), signal).is_some() {
            return Err(format!("signal «{}» is described twice", signal.name));
        }
    }
    Ok(by_name)
}

fn check_signal(signal: &Signal) -> Result<(), String> {
    let Question::Choice { choices, .. } = &signal.question else {
        return Ok(());
    };
    if choices.is_empty() {
        return Err(format!(
            "signal «{}» with choices lists no choices",
            signal.name
        ));
    }
    let mut keys = BTreeSet::new();
    for choice in choices {
        if !keys.insert(choice.key.as_str()) {
            return Err(format!(
                "signal «{}» has choice «{}» listed twice",
                signal.name, choice.key
            ));
        }
    }
    Ok(())
}

fn check_rule(rule: &Rule, signals: &BTreeMap<&str, &Signal>) -> Result<(), String> {
    match rule {
        Rule::All(rules) | Rule::Any(rules) => rules
            .iter()
            .try_for_each(|rule| check_rule(rule, signals)),
        Rule::Not(inner) => check_rule(inner, signals),
        Rule::Signal(condition) => check_condition(condition, signals),
    }
}

fn check_condition(condition: &Condition, signals: &BTreeMap<&str, &Signal>) -> Result<(), String> {
    let name = condition.name.as_str();
    let signal = signals.get(name).ok_or_else(|| {
        format!("rule refers to signal «{name}», which is not in signals")
    })?;
    match (&signal.question, condition.label.as_deref()) {
        (Question::Noul { .. }, None) => Ok(()),
        (Question::Noul { .. }, Some(_)) => Err(format!(
            "signal «{name}» is yes/no, a condition cannot name a choice"
        )),
        (Question::Choice { .. }, None) => Err(format!(
            "signal «{name}» has choices: the condition must name a choice"
        )),
        (Question::Choice { choices, .. }, Some(label)) => {
            if choices.iter().any(|choice| choice.key == label) {
                Ok(())
            } else {
                Err(format!("signal «{name}» has no choice «{label}»"))
            }
        }
    }
}

fn check_model_ref(model: &ModelRef, providers: &Providers, place: &str) -> Result<(), String> {
    // Checked before the provider lookup: the router is not in `providers.json`,
    // and "no such provider" would hide the real mistake.
    if model.provider == ROUTER_ID {
        return Err(format!(
            "{place} points at provider «{ROUTER_ID}»: a router cannot route to itself"
        ));
    }
    let spec = providers
        .get(&model.provider)
        .ok_or_else(|| format!("{place}: provider «{}» is not in the list", model.provider))?;
    if spec.model(&model.model).is_none() {
        return Err(format!(
            "{place}: provider «{}» has no model «{}»",
            model.provider, model.model
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::provider::registry::{Protocol, ProviderSpec};
    use crate::router::config::{Choice, Decision, DecisionModel, OnUnknown, Predicate};
    use crate::usage::unknown_model;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    /// A directory no other test, run, or process shares: the pid and a counter
    /// tell them apart, and the test name makes a failing one easy to find.
    /// Clock-based names collide on Windows, where the clock is coarse.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(test: &str) -> Self {
            let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "yaca-routing-{}-{n}-{test}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn providers() -> Providers {
        Providers {
            list: vec![ProviderSpec {
                id: "gateway".into(),
                display_name: "Gateway".into(),
                protocol: Protocol::OpenAi,
                base_url: "https://gate.example.com".into(),
                headers: BTreeMap::new(),
                models: vec![unknown_model("fast"), unknown_model("big")],
            }],
        }
    }

    fn model(provider: &str, model: &str) -> ModelRef {
        ModelRef {
            provider: provider.into(),
            model: model.into(),
        }
    }

    fn noul(name: &str) -> Signal {
        Signal {
            name: name.into(),
            question: Question::Noul {
                instructions: "is it code?".into(),
            },
        }
    }

    fn topic() -> Signal {
        Signal {
            name: "topic".into(),
            question: Question::Choice {
                instructions: "which topic?".into(),
                choices: ["math", "prose"]
                    .into_iter()
                    .map(|key| Choice {
                        key: key.into(),
                        description: key.into(),
                    })
                    .collect(),
            },
        }
    }

    fn signal_rule(name: &str, label: Option<&str>) -> Rule {
        Rule::Signal(Condition {
            name: name.into(),
            label: label.map(String::from),
            predicate: None,
        })
    }

    /// A config that passes [`validate`] against [`providers`]. Each rejection
    /// test breaks exactly one thing in it.
    fn valid() -> RoutingConfig {
        RoutingConfig {
            name: "auto".into(),
            decision_model: DecisionModel {
                model: model("gateway", "fast"),
                timeout_ms: 1000,
            },
            default: model("gateway", "big"),
            signals: vec![noul("code"), topic()],
            decisions: vec![Decision {
                name: "code".into(),
                priority: 10,
                rules: Rule::All(vec![
                    signal_rule("code", None),
                    signal_rule("topic", Some("math")),
                ]),
                on_unknown: OnUnknown::NoMatch,
                model: model("gateway", "big"),
            }],
        }
    }

    /// The message of the error [`validate`] gives, which must be a config error.
    fn rejected(cfg: &RoutingConfig) -> String {
        match validate(cfg, &providers()) {
            Err(CoreError::Config(message)) => message,
            other => panic!("expected a config error, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_file_is_not_configured() {
        let dir = TempDir::new("missing");
        assert_eq!(load(&dir.0).unwrap(), None);
    }

    #[test]
    fn save_then_load_gives_the_same_config() {
        let dir = TempDir::new("roundtrip");
        let cfg = valid();
        save(&dir.0, &cfg).unwrap();
        assert_eq!(load(&dir.0).unwrap(), Some(cfg));
        // The temporary file is renamed away, not left beside the real one.
        assert!(!dir.0.join(format!("{FILE}.tmp")).exists());
    }

    #[test]
    fn a_broken_file_is_an_error_not_no_routing() {
        let dir = TempDir::new("broken");
        fs::write(dir.0.join(FILE), "{ not json").unwrap();
        assert!(matches!(load(&dir.0), Err(CoreError::Config(_))));
    }

    #[test]
    fn a_valid_config_passes() {
        assert!(validate(&valid(), &providers()).is_ok());
    }

    #[test]
    fn an_empty_name_is_rejected() {
        let mut cfg = valid();
        cfg.name = "  ".into();
        assert!(rejected(&cfg).contains("has no name"));
    }

    #[test]
    fn duplicate_signal_names_are_rejected() {
        let mut cfg = valid();
        cfg.signals.push(noul("code"));
        assert!(rejected(&cfg).contains("signal «code» is described twice"));
    }

    #[test]
    fn duplicate_decision_names_are_rejected() {
        let mut cfg = valid();
        cfg.decisions.push(cfg.decisions[0].clone());
        assert!(rejected(&cfg).contains("decision «code» is described twice"));
    }

    #[test]
    fn a_choice_question_without_choices_is_rejected() {
        let mut cfg = valid();
        cfg.signals[1] = Signal {
            name: "topic".into(),
            question: Question::Choice {
                instructions: "which topic?".into(),
                choices: vec![],
            },
        };
        assert!(rejected(&cfg).contains("lists no choices"));
    }

    #[test]
    fn a_choice_question_with_duplicate_keys_is_rejected() {
        let mut cfg = valid();
        if let Question::Choice { choices, .. } = &mut cfg.signals[1].question {
            choices.push(Choice {
                key: "math".into(),
                description: "again".into(),
            });
        }
        assert!(rejected(&cfg).contains("choice «math» listed twice"));
    }

    #[test]
    fn a_condition_on_an_unknown_signal_is_rejected() {
        let mut cfg = valid();
        cfg.decisions[0].rules = signal_rule("ghost", None);
        assert!(rejected(&cfg).contains("signal «ghost», which is not in"));
    }

    #[test]
    fn a_label_on_a_yes_no_signal_is_rejected() {
        let mut cfg = valid();
        cfg.decisions[0].rules = signal_rule("code", Some("math"));
        assert!(rejected(&cfg).contains("is yes/no"));
    }

    #[test]
    fn a_choice_condition_without_a_label_is_rejected() {
        let mut cfg = valid();
        cfg.decisions[0].rules = signal_rule("topic", None);
        assert!(rejected(&cfg).contains("must name a choice"));
    }

    #[test]
    fn a_label_that_is_not_a_choice_is_rejected() {
        let mut cfg = valid();
        cfg.decisions[0].rules = signal_rule("topic", Some("poetry"));
        assert!(rejected(&cfg).contains("has no choice «poetry»"));
    }

    #[test]
    fn a_model_of_an_unknown_provider_is_rejected() {
        let mut cfg = valid();
        cfg.default = model("elsewhere", "big");
        assert!(rejected(&cfg).contains("provider «elsewhere» is not in the list"));
    }

    #[test]
    fn a_model_the_provider_does_not_have_is_rejected() {
        let mut cfg = valid();
        cfg.decisions[0].model = model("gateway", "gone");
        assert!(rejected(&cfg).contains("provider «gateway» has no model «gone»"));
    }

    #[test]
    fn a_zero_timeout_is_rejected() {
        let mut cfg = valid();
        cfg.decision_model.timeout_ms = 0;
        assert!(rejected(&cfg).contains("0 ms"));
    }

    #[test]
    fn a_router_routing_to_itself_is_rejected() {
        let mut cfg = valid();
        cfg.default = model(ROUTER_ID, "auto");
        assert!(rejected(&cfg).contains("to itself"));
    }

    #[test]
    fn a_bad_condition_deep_in_the_rule_tree_is_found() {
        // The walk is recursive: a bad condition under `Not` inside `Any` is still found.
        let mut cfg = valid();
        cfg.decisions[0].rules = Rule::Any(vec![Rule::Not(Box::new(signal_rule("ghost", None)))]);
        assert!(rejected(&cfg).contains("signal «ghost», which is not in"));
    }

    #[test]
    fn a_predicate_is_not_checked_structurally() {
        // Only the shape is checked here; the threshold is a value, read at evaluation.
        let mut cfg = valid();
        cfg.decisions[0].rules = Rule::Signal(Condition {
            name: "code".into(),
            label: None,
            predicate: Some(Predicate { gte: 0.9 }),
        });
        assert!(validate(&cfg, &providers()).is_ok());
    }
}
