//! Rules over signal answers. See [`super`].
//!
//! A rule is evaluated in three-valued (Kleene) logic. A signal the decision
//! model did not answer is `Unknown`, not `False`: `Not(Unknown)` must stay
//! unknown, or a late answer would flip a decision the wrong way. Whether an
//! unknown decision matches at all is the decision's own `on_unknown` choice.
//!
//! The answers do not carry their question's shape, so [`evaluate`] takes the
//! declared signals too and refuses an answer of the wrong kind (a probability
//! for a `choice` question, or a distribution for a `noul` one) as `Unknown`.
//! Without that check a model that answered the wrong question would still be
//! read as if it had answered the right one.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::config::{
    Answers, Condition, Decision, OnUnknown, Question, Rule, RoutingConfig, Signal, SignalValue,
};

/// The three truth values of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    True,
    False,
    Unknown,
}

impl std::ops::Not for Tri {
    type Output = Tri;

    /// Unknown stays unknown: the negation of an unanswered question is still
    /// unanswered.
    fn not(self) -> Tri {
        match self {
            Tri::True => Tri::False,
            Tri::False => Tri::True,
            Tri::Unknown => Tri::Unknown,
        }
    }
}

/// Evaluates `rule` over `answers`. `signals` is the declared set, used to
/// check each answer's kind against its question.
pub fn evaluate(rule: &Rule, signals: &[Signal], answers: &Answers) -> Tri {
    match rule {
        // One False decides a conjunction whatever else is unknown; otherwise an
        // unknown leaves the whole thing unknown.
        Rule::All(rules) => {
            let mut result = Tri::True;
            for rule in rules {
                match evaluate(rule, signals, answers) {
                    Tri::False => return Tri::False,
                    Tri::Unknown => result = Tri::Unknown,
                    Tri::True => {}
                }
            }
            result
        }
        Rule::Any(rules) => {
            let mut result = Tri::False;
            for rule in rules {
                match evaluate(rule, signals, answers) {
                    Tri::True => return Tri::True,
                    Tri::Unknown => result = Tri::Unknown,
                    Tri::False => {}
                }
            }
            result
        }
        Rule::Not(inner) => !evaluate(inner, signals, answers),
        Rule::Signal(condition) => condition_value(condition, signals, answers),
    }
}

fn condition_value(condition: &Condition, signals: &[Signal], answers: &Answers) -> Tri {
    let Some(signal) = signals.iter().find(|s| s.name == condition.name) else {
        return Tri::Unknown;
    };
    let Some(answer) = answers.get(&condition.name) else {
        return Tri::Unknown;
    };
    match (&signal.question, answer) {
        (Question::Noul { .. }, SignalValue::Noul(p)) => noul_value(condition, *p),
        (Question::Choice { .. }, SignalValue::Choice(map)) => choice_value(condition, map),
        // `SignalValue::Unknown`, and an answer of the wrong kind.
        _ => Tri::Unknown,
    }
}

/// The threshold a condition reads a probability against. Without a predicate
/// it is the coin flip: a model that is more often right than wrong says yes.
fn threshold(condition: &Condition) -> f32 {
    condition.predicate.map_or(0.5, |p| p.gte)
}

fn noul_value(condition: &Condition, p: f32) -> Tri {
    // A label names an option, and a yes/no question has none: the condition
    // is malformed, so it cannot be read either way.
    if condition.label.is_some() || !p.is_finite() {
        return Tri::Unknown;
    }
    from_bool(p >= threshold(condition))
}

fn choice_value(condition: &Condition, map: &BTreeMap<String, f32>) -> Tri {
    let Some(label) = condition.label.as_deref() else {
        return Tri::Unknown;
    };
    let Some(&p) = map.get(label) else {
        return Tri::Unknown;
    };
    if !p.is_finite() {
        return Tri::Unknown;
    }
    match condition.predicate {
        Some(predicate) => from_bool(p >= predicate.gte),
        None => {
            // Ties all count as the maximum: two options at 0.5 each are both
            // the most probable, and picking one by iteration order would make
            // the routing depend on key spelling.
            let max = map
                .values()
                .copied()
                .filter(|v| v.is_finite())
                .fold(f32::NEG_INFINITY, f32::max);
            from_bool(p >= max)
        }
    }
}

fn from_bool(value: bool) -> Tri {
    if value {
        Tri::True
    } else {
        Tri::False
    }
}

/// The decision that won a turn, and the signals it read.
pub struct Matched<'a> {
    pub decision: &'a Decision,
    /// Names of the signals the winning rule mentions, deduplicated, in the
    /// order they first appear in the rule. Recorded so the choice can be
    /// explained to the human.
    pub signals: Vec<String>,
}

/// The first decision that matches, by descending priority.
///
/// A decision with `Unknown` matches only if it asked for that with
/// `on_unknown: match`. The default is no match: an unanswered question must
/// not send a conversation to an expensive model.
pub fn pick<'a>(cfg: &'a RoutingConfig, answers: &Answers) -> Option<Matched<'a>> {
    let mut order: Vec<&Decision> = cfg.decisions.iter().collect();
    // `sort_by_key` is stable: among equal priorities the file's order stays,
    // which is the tie-break the config documents.
    order.sort_by_key(|d| Reverse(d.priority));

    order.into_iter().find_map(|decision| {
        let matches = match evaluate(&decision.rules, &cfg.signals, answers) {
            Tri::True => true,
            Tri::Unknown => decision.on_unknown == OnUnknown::Match,
            Tri::False => false,
        };
        matches.then(|| Matched {
            decision,
            signals: mentioned_signals(&decision.rules),
        })
    })
}

fn mentioned_signals(rule: &Rule) -> Vec<String> {
    let mut names = Vec::new();
    collect_names(rule, &mut names);
    names
}

fn collect_names(rule: &Rule, names: &mut Vec<String>) {
    match rule {
        Rule::All(rules) | Rule::Any(rules) => {
            for rule in rules {
                collect_names(rule, names);
            }
        }
        Rule::Not(inner) => collect_names(inner, names),
        Rule::Signal(condition) => {
            if !names.contains(&condition.name) {
                names.push(condition.name.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::config::{Choice, DecisionModel, ModelRef, Predicate};

    fn noul_signal(name: &str) -> Signal {
        Signal {
            name: name.into(),
            question: Question::Noul {
                instructions: format!("is it {name}?"),
            },
        }
    }

    fn choice_signal(name: &str) -> Signal {
        let choices = ["math", "prose", "other"]
            .into_iter()
            .map(|key| Choice {
                key: key.into(),
                description: key.into(),
            })
            .collect();
        Signal {
            name: name.into(),
            question: Question::Choice {
                instructions: "which one?".into(),
                choices,
            },
        }
    }

    fn signals() -> Vec<Signal> {
        vec![noul_signal("a"), noul_signal("b"), noul_signal("c"), choice_signal("topic")]
    }

    fn answers(entries: Vec<(&str, SignalValue)>) -> Answers {
        entries
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect()
    }

    fn yes() -> SignalValue {
        SignalValue::Noul(1.0)
    }

    fn no() -> SignalValue {
        SignalValue::Noul(0.0)
    }

    fn unknown() -> SignalValue {
        SignalValue::Unknown
    }

    fn probs(entries: &[(&str, f32)]) -> SignalValue {
        SignalValue::Choice(
            entries
                .iter()
                .map(|(k, p)| (k.to_string(), *p))
                .collect(),
        )
    }

    fn cond(name: &str, label: Option<&str>, gte: Option<f32>) -> Condition {
        Condition {
            name: name.into(),
            label: label.map(String::from),
            predicate: gte.map(|gte| Predicate { gte }),
        }
    }

    fn sig(name: &str) -> Rule {
        Rule::Signal(cond(name, None, None))
    }

    fn eval(rule: &Rule, answers: &Answers) -> Tri {
        evaluate(rule, &signals(), answers)
    }

    /// Answers `a` yes, `b` no, `c` unknown, and `topic` leaning to math.
    fn fixed() -> Answers {
        answers(vec![
            ("a", yes()),
            ("b", no()),
            ("c", unknown()),
            ("topic", probs(&[("math", 0.6), ("prose", 0.3), ("other", 0.1)])),
        ])
    }

    #[test]
    fn all_follows_kleene_logic() {
        let t = || sig("a");
        let f = || sig("b");
        let u = || sig("c");
        assert_eq!(eval(&Rule::All(vec![t(), t()]), &fixed()), Tri::True);
        assert_eq!(eval(&Rule::All(vec![t(), f()]), &fixed()), Tri::False);
        assert_eq!(eval(&Rule::All(vec![t(), u()]), &fixed()), Tri::Unknown);
        // A False wins over an Unknown, wherever it sits in the list.
        assert_eq!(eval(&Rule::All(vec![u(), f()]), &fixed()), Tri::False);
        assert_eq!(eval(&Rule::All(vec![u(), u()]), &fixed()), Tri::Unknown);
        assert_eq!(eval(&Rule::All(vec![]), &fixed()), Tri::True);
    }

    #[test]
    fn any_follows_kleene_logic() {
        let t = || sig("a");
        let f = || sig("b");
        let u = || sig("c");
        assert_eq!(eval(&Rule::Any(vec![f(), f()]), &fixed()), Tri::False);
        assert_eq!(eval(&Rule::Any(vec![f(), t()]), &fixed()), Tri::True);
        assert_eq!(eval(&Rule::Any(vec![f(), u()]), &fixed()), Tri::Unknown);
        // A True wins over an Unknown, wherever it sits in the list.
        assert_eq!(eval(&Rule::Any(vec![u(), t()]), &fixed()), Tri::True);
        assert_eq!(eval(&Rule::Any(vec![u(), u()]), &fixed()), Tri::Unknown);
        assert_eq!(eval(&Rule::Any(vec![]), &fixed()), Tri::False);
    }

    #[test]
    fn not_swaps_truth_and_keeps_unknown() {
        assert_eq!(eval(&Rule::Not(Box::new(sig("a"))), &fixed()), Tri::False);
        assert_eq!(eval(&Rule::Not(Box::new(sig("b"))), &fixed()), Tri::True);
        assert_eq!(eval(&Rule::Not(Box::new(sig("c"))), &fixed()), Tri::Unknown);
        // Nested: Not(All[True, Unknown]) is Not(Unknown).
        let nested = Rule::Not(Box::new(Rule::All(vec![sig("a"), sig("c")])));
        assert_eq!(eval(&nested, &fixed()), Tri::Unknown);
    }

    #[test]
    fn noul_defaults_to_a_half_threshold() {
        let rule = sig("a");
        let at = answers(vec![("a", SignalValue::Noul(0.5))]);
        let below = answers(vec![("a", SignalValue::Noul(0.49))]);
        assert_eq!(eval(&rule, &at), Tri::True);
        assert_eq!(eval(&rule, &below), Tri::False);
    }

    #[test]
    fn noul_uses_the_explicit_predicate() {
        let rule = Rule::Signal(cond("a", None, Some(0.8)));
        let at = answers(vec![("a", SignalValue::Noul(0.8))]);
        let below = answers(vec![("a", SignalValue::Noul(0.79))]);
        assert_eq!(eval(&rule, &at), Tri::True);
        assert_eq!(eval(&rule, &below), Tri::False);
    }

    #[test]
    fn noul_with_a_label_is_malformed_and_unknown() {
        let rule = Rule::Signal(cond("a", Some("math"), None));
        assert_eq!(eval(&rule, &fixed()), Tri::Unknown);
    }

    #[test]
    fn missing_or_unknown_answers_are_unknown() {
        let missing = answers(vec![]);
        assert_eq!(eval(&sig("a"), &missing), Tri::Unknown);
        assert_eq!(eval(&sig("c"), &fixed()), Tri::Unknown);
    }

    #[test]
    fn a_signal_not_declared_is_unknown() {
        let rule = sig("nowhere");
        let answered = answers(vec![("nowhere", yes())]);
        assert_eq!(eval(&rule, &answered), Tri::Unknown);
    }

    #[test]
    fn choice_without_predicate_is_argmax() {
        let math = Rule::Signal(cond("topic", Some("math"), None));
        let prose = Rule::Signal(cond("topic", Some("prose"), None));
        assert_eq!(eval(&math, &fixed()), Tri::True);
        assert_eq!(eval(&prose, &fixed()), Tri::False);
    }

    #[test]
    fn choice_ties_all_count_as_maximum() {
        let tied = answers(vec![("topic", probs(&[("math", 0.5), ("prose", 0.5), ("other", 0.0)]))]);
        let math = Rule::Signal(cond("topic", Some("math"), None));
        let prose = Rule::Signal(cond("topic", Some("prose"), None));
        let other = Rule::Signal(cond("topic", Some("other"), None));
        assert_eq!(eval(&math, &tied), Tri::True);
        assert_eq!(eval(&prose, &tied), Tri::True);
        assert_eq!(eval(&other, &tied), Tri::False);
    }

    #[test]
    fn choice_with_predicate_ignores_the_argmax() {
        // `prose` is not the most probable option, but it clears 0.25.
        let low = Rule::Signal(cond("topic", Some("prose"), Some(0.25)));
        let high = Rule::Signal(cond("topic", Some("prose"), Some(0.4)));
        assert_eq!(eval(&low, &fixed()), Tri::True);
        assert_eq!(eval(&high, &fixed()), Tri::False);
    }

    #[test]
    fn choice_without_label_or_unknown_label_is_unknown() {
        let no_label = Rule::Signal(cond("topic", None, None));
        let absent = Rule::Signal(cond("topic", Some("poetry"), None));
        assert_eq!(eval(&no_label, &fixed()), Tri::Unknown);
        assert_eq!(eval(&absent, &fixed()), Tri::Unknown);
    }

    #[test]
    fn answer_of_the_wrong_kind_is_unknown() {
        // A probability for a choice question, and a distribution for a yes/no one.
        let wrong_for_choice = answers(vec![("topic", SignalValue::Noul(0.9))]);
        let wrong_for_noul = answers(vec![("a", probs(&[("math", 1.0)]))]);
        let on_choice = Rule::Signal(cond("topic", Some("math"), None));
        assert_eq!(eval(&on_choice, &wrong_for_choice), Tri::Unknown);
        assert_eq!(eval(&sig("a"), &wrong_for_noul), Tri::Unknown);
    }

    fn model() -> ModelRef {
        ModelRef {
            provider: "gateway".into(),
            model: "big".into(),
        }
    }

    fn decision(name: &str, priority: i32, rules: Rule, on_unknown: OnUnknown) -> Decision {
        Decision {
            name: name.into(),
            priority,
            rules,
            on_unknown,
            model: model(),
        }
    }

    fn config(decisions: Vec<Decision>) -> RoutingConfig {
        RoutingConfig {
            name: "auto".into(),
            decision_model: DecisionModel {
                model: model(),
                timeout_ms: 1000,
            },
            default: model(),
            signals: signals(),
            decisions,
        }
    }

    fn picked_name(cfg: &RoutingConfig, answers: &Answers) -> Option<String> {
        pick(cfg, answers).map(|m| m.decision.name.clone())
    }

    #[test]
    fn higher_priority_wins_and_equal_priority_keeps_file_order() {
        let cfg = config(vec![
            decision("first", 5, sig("a"), OnUnknown::NoMatch),
            decision("second", 5, sig("a"), OnUnknown::NoMatch),
            decision("top", 10, sig("a"), OnUnknown::NoMatch),
        ]);
        assert_eq!(picked_name(&cfg, &fixed()).as_deref(), Some("top"));

        // Without the top one, the tie goes to whichever comes first in the file.
        let cfg = config(vec![
            decision("first", 5, sig("a"), OnUnknown::NoMatch),
            decision("second", 5, sig("a"), OnUnknown::NoMatch),
            decision("top", 10, sig("b"), OnUnknown::NoMatch),
        ]);
        assert_eq!(picked_name(&cfg, &fixed()).as_deref(), Some("first"));
    }

    #[test]
    fn unknown_matches_only_when_the_decision_says_so() {
        let unknown_rule = || sig("c");

        let matching = config(vec![decision("m", 0, unknown_rule(), OnUnknown::Match)]);
        assert_eq!(picked_name(&matching, &fixed()).as_deref(), Some("m"));

        let refusing = config(vec![decision("n", 0, unknown_rule(), OnUnknown::NoMatch)]);
        assert_eq!(picked_name(&refusing, &fixed()), None);
    }

    #[test]
    fn an_unknown_decision_yields_to_a_later_known_one() {
        let cfg = config(vec![
            decision("unsure", 10, sig("c"), OnUnknown::NoMatch),
            decision("sure", 0, sig("a"), OnUnknown::NoMatch),
        ]);
        assert_eq!(picked_name(&cfg, &fixed()).as_deref(), Some("sure"));
    }

    #[test]
    fn nothing_matched_is_none() {
        let cfg = config(vec![decision("never", 0, sig("b"), OnUnknown::Match)]);
        assert!(pick(&cfg, &fixed()).is_none());
        assert!(pick(&config(vec![]), &fixed()).is_none());
    }

    #[test]
    fn matched_signals_are_deduplicated_in_first_seen_order() {
        // `b` is answered no, so `Not(b)` holds, and `Any[a, b]` holds through `a`:
        // the whole rule is True and names each signal once.
        let rule = Rule::All(vec![
            Rule::Not(Box::new(sig("b"))),
            sig("a"),
            Rule::Signal(cond("topic", Some("math"), None)),
            Rule::Any(vec![sig("a"), sig("b")]),
        ]);
        let cfg = config(vec![decision("d", 0, rule, OnUnknown::NoMatch)]);
        let matched = pick(&cfg, &fixed()).unwrap();
        assert_eq!(matched.signals, ["b", "a", "topic"]);
    }
}
