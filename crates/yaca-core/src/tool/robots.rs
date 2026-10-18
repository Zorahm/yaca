//! `robots.txt`: whose restriction Yaca obeys and how it turns into a tool refusal.
//!
//! Custom parsing instead of a dependency — for the same reason the window has
//! its own i18n: groups, `Allow`/`Disallow`, and two wildcards are needed, not a
//! crate tree. The rules below follow RFC 9309, with deviations noted where made.
//!
//! **The product token is matched, not the full `User-Agent` string.** That is
//! `YacaTraveler` from [`super::net`], case-insensitively; if its own group is
//! absent, `*` is used. Hence the requirement for the name: it must be specific
//! enough that a rule for it touches nothing else.
//!
//! **Longest match wins.** A path may match both `Allow` and `Disallow`; the
//! longer pattern wins, and on equal length — `Allow`. This is RFC 9309 §2.2.2,
//! and the only reason rules cannot be simply iterated in order.
//!
//! **Percent-encoding is matched as-is.** The RFC asks to normalize path and
//! pattern first; here the incoming bytes are compared directly — `url.path()`
//! is already encoded, and rules are usually written the same way.
//!
//! **Unavailability is not a restriction.** The RFC says an unanswered `robots.txt`
//! is a complete ban. For a crawler that is right, for Yaca it is not: the user
//! just manually allowed this domain (§5.7), and a server down for a second
//! should not block the page for a reason no one can see. Therefore network
//! errors and 5xx allow, as does 404. Only a parsed file with an explicit rule
//! restricts.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Url;

use super::net;
use super::ToolCtx;
use crate::error::ToolError;

/// How long the parsed file lives in process memory.
///
/// Without cache each page would take two requests, fetching `robots.txt`
/// repeatedly. One hour is the upper bound the RFC considers reasonable;
/// sessions are shorter, so in practice the file is read once.
const TTL: Duration = Duration::from_secs(60 * 60);

/// Ceiling on the file itself. RFC asks to parse at least 500 KiB; anything
/// larger is someone's mistake, with no need to pull it into memory.
const MAX_BYTES: usize = 512 * 1024;

/// How many redirects `robots.txt` follows. The client does not follow them
/// automatically ([`net::client`]), but RFC asks to follow at least five.
const MAX_REDIRECTS: usize = 5;

/// A single rule: path pattern and whether it allows access.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    pattern: String,
    allow: bool,
}

/// Parsed file — only the group applicable to us.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rules {
    rules: Vec<Rule>,
}

impl Rules {
    /// Empty rules: file is absent, did not respond, or has no group for us.
    pub fn allow_all() -> Self {
        Rules::default()
    }

    /// Parses the file for the given product token `agent`.
    ///
    /// There are many groups in the file, and they cannot be merged: rules of
    /// another group do not apply to us. Our group is preferred; if absent, `*`.
    pub fn parse(text: &str, agent: &str) -> Self {
        let agent = agent.to_ascii_lowercase();
        let mut mine: Option<Vec<Rule>> = None;
        let mut fallback: Option<Vec<Rule>> = None;

        // Consecutive `User-agent` lines form one group. Flags indicate
        // who receives the following rules.
        let mut for_me = false;
        let mut for_all = false;
        // A rule after a group closes it: next `User-agent` starts a new group.
        let mut collecting = false;

        for line in text.lines() {
            let Some((field, value)) = split(line) else {
                continue;
            };

            match field.as_str() {
                "user-agent" => {
                    if collecting {
                        for_me = false;
                        for_all = false;
                        collecting = false;
                    }
                    let value = value.to_ascii_lowercase();
                    if value == agent {
                        for_me = true;
                    } else if value == "*" {
                        for_all = true;
                    }
                }
                "allow" | "disallow" => {
                    collecting = true;
                    let allow = field == "allow";
                    // An empty `Disallow` means "nothing forbidden", i.e. absence
                    // of restriction, not forbidding an empty path.
                    if !allow && value.is_empty() {
                        continue;
                    }
                    let rule = Rule {
                        pattern: value.to_string(),
                        allow,
                    };
                    if for_me {
                        mine.get_or_insert_with(Vec::new).push(rule.clone());
                    }
                    if for_all {
                        fallback.get_or_insert_with(Vec::new).push(rule);
                    }
                }
                // `Crawl-delay`, `Sitemap`, etc. are ignored: Yaca fetches single
                // pages on user request, not crawling the site.
                _ => {}
            }
        }

        Rules {
            rules: mine.or(fallback).unwrap_or_default(),
        }
    }

    /// Finds the rule deciding the fate of the path: longest matching pattern,
    /// on tie — allow (RFC 9309 §2.2.2).
    fn best(&self, path: &str) -> Option<&Rule> {
        self.rules
            .iter()
            .filter(|rule| matches(&rule.pattern, path))
            .fold(None, |best: Option<&Rule>, rule| match best {
                Some(other) if other.pattern.len() > rule.pattern.len() => Some(other),
                Some(other) if other.pattern.len() == rule.pattern.len() && other.allow => {
                    Some(other)
                }
                _ => Some(rule),
            })
    }

    /// The blocking pattern, or `None` if the path is allowed.
    fn blocking(&self, path: &str) -> Option<&str> {
        self.best(path)
            .filter(|rule| !rule.allow)
            .map(|rule| rule.pattern.as_str())
    }
}

/// `field: value` with comment stripped.
fn split(line: &str) -> Option<(String, &str)> {
    let line = line.split('#').next().unwrap_or("").trim();
    let (field, value) = line.split_once(':')?;
    let field = field.trim().to_ascii_lowercase();
    if field.is_empty() {
        return None;
    }
    Some((field, value.trim()))
}

/// Matches pattern against path: `*` matches any sequence, `$` at the end pins
/// the end of string, remainder is literal.
fn matches(pattern: &str, path: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = if anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut at = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        // First part must match at the beginning if pattern lacks leading `*`.
        if index == 0 {
            if !path[at..].starts_with(part) {
                return false;
            }
            at += part.len();
            continue;
        }
        let Some(found) = path[at..].find(part) else {
            return false;
        };
        at += found + part.len();
    }

    if !anchored {
        return true;
    }
    // With `$`, last part must reach the end if it is not `*`.
    match parts.last() {
        Some(last) if !last.is_empty() => at == path.len(),
        _ => true,
    }
}

/// Cache by origin. Key is `scheme://host:port`.
type Cache = Mutex<HashMap<String, (Rules, Instant)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn origin_of(url: &Url) -> String {
    url.origin().ascii_serialization()
}

/// Checks the URL and, if disallowed, returns a rejection error.
///
/// [`ToolError::Rejected`], not `Failed`: a safeguard tripped.
pub async fn check(ctx: &ToolCtx, url: &Url) -> Result<(), ToolError> {
    let rules = rules_for(ctx, url).await;
    let path = path_of(url);

    if let Some(pattern) = rules.blocking(&path) {
        return Err(ToolError::Rejected(format!(
            "robots.txt on {} disallows this path with rule `Disallow: {pattern}`. \
             Yaca respects such rules; this can be turned off in settings under \"Tools\".",
            url.host_str().unwrap_or_default()
        )));
    }
    Ok(())
}

/// Path along with query string: robots.txt rules can match queries too.
fn path_of(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    }
}

/// Host rules: from cache or from server.
async fn rules_for(ctx: &ToolCtx, url: &Url) -> Rules {
    let origin = origin_of(url);

    if let Some((rules, at)) = cache().lock().unwrap().get(&origin) {
        if at.elapsed() < TTL {
            return rules.clone();
        }
    }

    let rules = fetch(ctx, url).await;
    cache()
        .lock()
        .unwrap()
        .insert(origin, (rules.clone(), Instant::now()));
    rules
}

/// Fetches and parses `robots.txt`. Any failure allows access.
async fn fetch(ctx: &ToolCtx, url: &Url) -> Rules {
    let Ok(mut at) = url.join("/robots.txt") else {
        return Rules::allow_all();
    };

    for _ in 0..=MAX_REDIRECTS {
        if ctx.cancel.is_cancelled() {
            return Rules::allow_all();
        }

        let Ok(response) = net::client().get(at.clone()).send().await else {
            return Rules::allow_all();
        };

        if response.status().is_redirection() {
            let next = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| at.join(v).ok());
            match next {
                Some(next) => {
                    at = next;
                    continue;
                }
                None => return Rules::allow_all(),
            }
        }

        // 404 and similar mean no rules. 5xx also allows rather than forbids.
        if !response.status().is_success() {
            return Rules::allow_all();
        }

        let Ok(body) = response.bytes().await else {
            return Rules::allow_all();
        };
        let text = String::from_utf8_lossy(&body[..body.len().min(MAX_BYTES)]);
        return Rules::parse(&text, net::PRODUCT_TOKEN);
    }

    Rules::allow_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(text: &str) -> Rules {
        Rules::parse(text, "yacatraveler")
    }

    /// Whether access is allowed. Wrapper over [`Rules::blocking`].
    fn allows(rules: &Rules, path: &str) -> bool {
        rules.blocking(path).is_none()
    }

    #[test]
    fn an_empty_file_allows_everything() {
        assert!(allows(&rules(""), "/anything"));
    }

    #[test]
    fn a_star_group_applies_when_we_are_not_named() {
        let r = rules("User-agent: *\nDisallow: /private\n");
        assert!(!allows(&r, "/private/page"));
        assert!(allows(&r, "/public"));
    }

    /// Our own group replaces `*` entirely rather than augmenting it.
    #[test]
    fn our_own_group_replaces_the_star_group() {
        let r = rules("User-agent: *\nDisallow: /\n\nUser-agent: YacaTraveler\nDisallow: /secret\n");
        assert!(allows(&r, "/anything"), "rules from `*` do not apply to us");
        assert!(!allows(&r, "/secret"));
    }

    #[test]
    fn the_name_is_matched_without_case() {
        let r = Rules::parse("User-agent: YACATRAVELER\nDisallow: /x\n", "yacatraveler");
        assert!(!allows(&r, "/x"));
    }

    /// Consecutive `User-agent` share one group of rules.
    #[test]
    fn agents_listed_together_share_their_rules() {
        let r = rules("User-agent: SomeBot\nUser-agent: YacaTraveler\nDisallow: /both\n");
        assert!(!allows(&r, "/both"));
    }

    /// A rule after a group closes it: next `User-agent` starts a new group.
    #[test]
    fn a_new_group_starts_after_the_rules_of_the_previous_one() {
        let r = rules("User-agent: YacaTraveler\nDisallow: /mine\nUser-agent: Other\nDisallow: /theirs\n");
        assert!(!allows(&r, "/mine"));
        assert!(allows(&r, "/theirs"));
    }

    /// Longest pattern wins over order of lines (RFC 9309 §2.2.2).
    #[test]
    fn the_longest_pattern_wins_over_the_order_of_lines() {
        let r = rules("User-agent: *\nDisallow: /docs\nAllow: /docs/public\n");
        assert!(!allows(&r, "/docs/secret"));
        assert!(allows(&r, "/docs/public/page"));

        // Same result with reversed line order.
        let r = rules("User-agent: *\nAllow: /docs/public\nDisallow: /docs\n");
        assert!(allows(&r, "/docs/public/page"));
    }

    #[test]
    fn equal_length_goes_to_allow() {
        let r = rules("User-agent: *\nDisallow: /a\nAllow: /a\n");
        assert!(allows(&r, "/a"));
    }

    #[test]
    fn an_empty_disallow_forbids_nothing() {
        let r = rules("User-agent: *\nDisallow:\n");
        assert!(allows(&r, "/anything"));
    }

    #[test]
    fn comments_and_odd_spacing_are_survived() {
        let r = rules("# note\n  User-agent :  *  # who\n Disallow : /x  \n");
        assert!(!allows(&r, "/x"));
    }

    #[test]
    fn a_star_stands_for_anything() {
        let r = rules("User-agent: *\nDisallow: /*/private\n");
        assert!(!allows(&r, "/a/private"));
        assert!(!allows(&r, "/a/b/private"));
        assert!(allows(&r, "/private"));
    }

    #[test]
    fn a_dollar_pins_the_end() {
        let r = rules("User-agent: *\nDisallow: /*.pdf$\n");
        assert!(!allows(&r, "/paper.pdf"));
        assert!(allows(&r, "/paper.pdf.html"));
    }

    /// Query string is part of the path checked by robots.txt rules.
    #[test]
    fn the_query_string_is_part_of_the_path() {
        let r = rules("User-agent: *\nDisallow: /search?q=\n");
        assert!(!allows(&r, "/search?q=secret"));
        assert!(allows(&r, "/search"));
    }

    #[test]
    fn the_blocking_pattern_is_reported_for_the_explanation() {
        let r = rules("User-agent: *\nDisallow: /private\n");
        assert_eq!(r.blocking("/private/page"), Some("/private"));
        assert_eq!(r.blocking("/public"), None);
    }

    /// Allowed path has no blocking pattern.
    #[test]
    fn an_allowed_path_has_no_blocking_pattern() {
        let r = rules("User-agent: *\nDisallow: /docs\nAllow: /docs/ok\n");
        assert_eq!(r.blocking("/docs/ok"), None);
        assert_eq!(r.blocking("/docs/no"), Some("/docs"));
    }
}
