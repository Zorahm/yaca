//! Looking for a newer release on GitHub.
//!
//! The release list is the source: the tag says the version, the body of the
//! release is the changelog. Nothing is downloaded or installed here; the
//! answer is "this release is newer, here is what it says and where it is".
//!
//! # Which version is newer
//!
//! The release **channel** outranks the numbers: `alpha < beta < rc < release`,
//! and only inside one channel do the numbers decide. So `1.0.0-beta` is newer
//! than `1.2.0-alpha` whatever the digits, and a plain `1.0.0` is newer than any
//! beta. This is the order a person means by "newer" for a project that moves
//! alpha, beta, release and restarts the numbers on the way.
//!
//! A build is only offered releases of its own channel or a more stable one: a
//! person on a release is not nagged about betas, a person on a beta is told
//! about the next beta and about the release.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the releases live.
pub const REPOSITORY: &str = "Zorahm/yaca";

/// How long to wait for GitHub. The check runs in the background and from a
/// button; neither should hang on a dead connection.
const TIMEOUT: Duration = Duration::from_secs(10);

/// How far back the list reaches. Newer than the running build is all that
/// matters, and a build older than thirty releases is better off reading the
/// page.
const PAGE: u32 = 30;

/// Maturity of a release, least stable first. The derive order is the
/// comparison order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Channel {
    /// A label nobody here uses (`dev`, `nightly`): older than everything named.
    Other,
    Alpha,
    Beta,
    Rc,
    Release,
}

/// A parsed version. Field order is comparison order: channel first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    channel: Channel,
    major: u64,
    minor: u64,
    patch: u64,
    /// The number after the label: `beta.2`, `beta2` and `beta-2` are all 2.
    pre: u64,
}

impl Version {
    /// `1.0.0`, `v1.2`, `1.0.0-beta`, `1.0.0-beta.2`. Build metadata after
    /// `+` is ignored. `None` for anything that does not start with a number.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim().trim_start_matches(['v', 'V']);
        let text = text.split('+').next().unwrap_or(text);
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };

        let mut numbers = core.split('.');
        let major = numbers.next()?.parse().ok()?;
        let minor = numbers.next().map_or(Some(0), |n| n.parse().ok())?;
        let patch = numbers.next().map_or(Some(0), |n| n.parse().ok())?;
        if numbers.next().is_some() {
            return None;
        }

        let (channel, pre) = match pre {
            None => (Channel::Release, 0),
            Some(pre) => {
                let label: String = pre
                    .chars()
                    .take_while(char::is_ascii_alphabetic)
                    .collect::<String>()
                    .to_ascii_lowercase();
                let digits: String = pre
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(char::is_ascii_digit)
                    .collect();
                let channel = match label.as_str() {
                    "alpha" | "a" => Channel::Alpha,
                    "beta" | "b" => Channel::Beta,
                    "rc" => Channel::Rc,
                    _ => Channel::Other,
                };
                (channel, digits.parse().unwrap_or(0))
            }
        };

        Some(Version { channel, major, minor, patch, pre })
    }
}

/// A release that is newer than the running build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    /// The tag without a leading `v`: `1.0.1-beta`.
    pub version: String,
    /// The release title; the tag when GitHub has none.
    pub name: String,
    /// The text of the release, Markdown. Empty when the author wrote none.
    pub changelog: String,
    /// The release page.
    pub url: String,
    /// The installer for this system, when the release carries one. The page
    /// is the fallback: a missing asset is not a reason to say nothing.
    pub download_url: Option<String>,
    pub published_at: Option<String>,
    pub prerelease: bool,
}

/// The answer to "is there anything newer".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// The version that asked.
    pub current: String,
    /// The newest release it may take, or `None` when it already has it.
    pub latest: Option<ReleaseInfo>,
}

/// Why the check could not be answered. The codes are for the window to put
/// in its own words; none of them is shown as it stands.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("offline")]
    Offline,
    #[error("rate_limited")]
    RateLimited,
    #[error("status:{0}")]
    Status(u16),
    #[error("bad_response")]
    BadResponse,
    #[error("bad_version")]
    BadVersion,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// File name endings of the installer, per system, in order of preference.
fn installer_suffixes() -> &'static [&'static str] {
    if cfg!(windows) {
        &["-setup.exe", ".msi"]
    } else if cfg!(target_os = "macos") {
        &[".dmg"]
    } else {
        &[".AppImage", ".deb"]
    }
}

/// Words in an installer's file name that say which processor it is for.
fn arch_words() -> &'static [&'static str] {
    if cfg!(target_arch = "aarch64") {
        &["aarch64", "arm64"]
    } else {
        &["x64", "x86_64", "amd64"]
    }
}

/// The installer of this system and processor. A release that ships a build per
/// processor (macOS: Apple silicon and Intel) names them differently; when no
/// name says which is ours the first of the system's kind is taken, since a
/// download of the wrong one is better than none.
fn installer_of(assets: &[GhAsset]) -> Option<String> {
    pick_installer(assets, installer_suffixes(), arch_words())
}

fn pick_installer(assets: &[GhAsset], suffixes: &[&str], arch: &[&str]) -> Option<String> {
    suffixes.iter().find_map(|suffix| {
        let mut ours = assets.iter().filter(|a| a.name.ends_with(suffix)).peekable();
        let first = ours.peek().map(|a| a.browser_download_url.clone());
        ours.find(|a| {
            let name = a.name.to_ascii_lowercase();
            arch.iter().any(|word| name.contains(word))
        })
        .map(|a| a.browser_download_url.clone())
        .or(first)
    })
}

/// The newest release the running version may take, if any.
fn pick(releases: Vec<GhRelease>, current: Version) -> Option<ReleaseInfo> {
    releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| Version::parse(&r.tag_name).map(|v| (v, r)))
        // Own channel or a more stable one, and actually newer.
        .filter(|(v, _)| v.channel >= current.channel && *v > current)
        .max_by_key(|(v, _)| *v)
        .map(|(_, r)| ReleaseInfo {
            version: r.tag_name.trim_start_matches(['v', 'V']).to_string(),
            name: r.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| r.tag_name.clone()),
            changelog: r.body.unwrap_or_default().replace("\r\n", "\n").trim().to_string(),
            download_url: installer_of(&r.assets),
            url: r.html_url,
            published_at: r.published_at,
            prerelease: r.prerelease,
        })
}

/// Reads the release list out of GitHub's answer and picks the newer one.
/// Split from [`check`] so the choice can be tested without a network.
pub fn decide(current: &str, releases_json: &str) -> Result<UpdateCheck, UpdateError> {
    let running = Version::parse(current).ok_or(UpdateError::BadVersion)?;
    let releases: Vec<GhRelease> =
        serde_json::from_str(releases_json).map_err(|_| UpdateError::BadResponse)?;
    Ok(UpdateCheck { current: current.to_string(), latest: pick(releases, running) })
}

/// Asks GitHub whether a release newer than `current` exists.
pub async fn check(current: &str) -> Result<UpdateCheck, UpdateError> {
    check_at("https://api.github.com", current).await
}

async fn check_at(api: &str, current: &str) -> Result<UpdateCheck, UpdateError> {
    // Refuse a version we cannot read before spending a request on it.
    Version::parse(current).ok_or(UpdateError::BadVersion)?;

    let client = reqwest::Client::builder()
        .user_agent(format!("Yaca/{current}"))
        .timeout(TIMEOUT)
        .build()
        .map_err(|_| UpdateError::Offline)?;

    let response = client
        .get(format!("{api}/repos/{REPOSITORY}/releases?per_page={PAGE}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| UpdateError::Offline)?;

    let status = response.status();
    // GitHub answers an exhausted anonymous quota with 403, or 429 when asked
    // too fast; both mean "not now" rather than "broken".
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(UpdateError::RateLimited);
    }
    if !status.is_success() {
        return Err(UpdateError::Status(status.as_u16()));
    }

    let body = response.text().await.map_err(|_| UpdateError::Offline)?;
    decide(current, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    #[test]
    fn a_beta_is_newer_than_an_alpha_whatever_the_digits() {
        assert!(v("1.0.0-beta") > v("1.2.0-alpha"));
        assert!(v("1.0.0-beta") > v("9.9.9-alpha"));
        assert!(v("0.0.1-beta") > v("1.0.4-alpha"));
    }

    #[test]
    fn a_release_is_newer_than_any_prerelease() {
        assert!(v("1.0.0") > v("1.0.0-beta"));
        assert!(v("1.0.0") > v("1.2.0-beta"));
        assert!(v("1.0.0") > v("1.0.0-rc.3"));
        assert!(v("1.0.0-rc") > v("1.0.0-beta.9"));
    }

    #[test]
    fn inside_a_channel_the_numbers_decide() {
        assert!(v("1.0.1-beta") > v("1.0.0-beta"));
        assert!(v("1.1.0-alpha") > v("1.0.9-alpha"));
        assert!(v("1.0.0-beta.2") > v("1.0.0-beta.1"));
        assert!(v("1.0.0-beta.10") > v("1.0.0-beta.9"));
        assert!(v("1.10.0") > v("1.9.0"));
        assert_eq!(v("1.0.0-beta"), v("v1.0.0-beta.0"));
    }

    #[test]
    fn spellings_of_the_same_prerelease_agree() {
        assert_eq!(v("1.0.0-beta2"), v("1.0.0-beta.2"));
        assert_eq!(v("1.0.0-beta-2"), v("1.0.0-beta.2"));
        assert_eq!(v("1.0.0-BETA.2"), v("1.0.0-beta.2"));
        assert_eq!(v("1.0.0+build5"), v("1.0.0"));
        assert_eq!(v("v1.2"), v("1.2.0"));
    }

    #[test]
    fn nonsense_does_not_parse() {
        for text in ["", "latest", "v", "1.x.0", "1.2.3.4", "-beta"] {
            assert!(Version::parse(text).is_none(), "{text:?}");
        }
    }

    fn release(tag: &str, body: &str) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "name": format!("Yaca {tag}"),
            "body": body,
            "html_url": format!("https://github.com/Zorahm/yaca/releases/tag/{tag}"),
            "draft": false,
            "prerelease": tag.contains('-'),
            "published_at": "2026-10-20T10:00:00Z",
            "assets": [
                {
                    "name": format!("Yaca_{}_x64-setup.exe", tag.trim_start_matches('v')),
                    "browser_download_url": format!("https://example.test/{tag}/setup.exe"),
                },
                { "name": "notes.txt", "browser_download_url": "https://example.test/notes.txt" },
            ],
        })
    }

    fn answer(current: &str, releases: Vec<serde_json::Value>) -> UpdateCheck {
        decide(current, &serde_json::Value::Array(releases).to_string()).unwrap()
    }

    #[test]
    fn the_changelog_is_the_text_of_the_release() {
        let got = answer(
            "1.0.0-beta",
            vec![release("v1.0.1-beta", "## New\r\n- one\r\n- two\r\n\r\n")],
        );
        let latest = got.latest.expect("a newer beta exists");
        assert_eq!(latest.version, "1.0.1-beta");
        assert_eq!(latest.changelog, "## New\n- one\n- two");
        assert_eq!(latest.name, "Yaca v1.0.1-beta");
        assert!(latest.prerelease);
        if cfg!(windows) {
            assert_eq!(
                latest.download_url.as_deref(),
                Some("https://example.test/v1.0.1-beta/setup.exe")
            );
        }
    }

    #[test]
    fn a_beta_is_told_about_the_release_and_picks_the_newest() {
        let got = answer(
            "1.0.0-beta",
            vec![
                release("v1.0.1-beta", ""),
                release("v1.0.0", ""),
                release("v1.2.0-alpha", ""),
                release("v0.9.0", ""),
            ],
        );
        assert_eq!(got.latest.unwrap().version, "1.0.0");
    }

    #[test]
    fn a_release_is_not_offered_prereleases() {
        let got = answer("1.0.0", vec![release("v1.1.0-beta", ""), release("v1.0.0", "")]);
        assert_eq!(got.latest, None);
    }

    #[test]
    fn an_alpha_is_offered_the_beta_that_followed() {
        let got = answer("1.2.0-alpha", vec![release("v1.0.0-beta", "")]);
        assert_eq!(got.latest.unwrap().version, "1.0.0-beta");
    }

    #[test]
    fn the_running_build_is_not_newer_than_itself() {
        let got = answer("1.0.0-beta", vec![release("v1.0.0-beta", "")]);
        assert_eq!(got.latest, None);
        assert_eq!(got.current, "1.0.0-beta");
    }

    #[test]
    fn drafts_and_unreadable_tags_are_skipped() {
        let mut draft = release("v2.0.0", "");
        draft["draft"] = serde_json::json!(true);
        let got = answer("1.0.0", vec![draft, release("nightly", ""), release("v1.0.1", "")]);
        assert_eq!(got.latest.unwrap().version, "1.0.1");
    }

    #[test]
    fn a_release_without_text_or_title_still_answers() {
        let mut bare = release("v1.0.1", "");
        bare["body"] = serde_json::Value::Null;
        bare["name"] = serde_json::json!("");
        bare["assets"] = serde_json::json!([]);
        let latest = answer("1.0.0", vec![bare]).latest.unwrap();
        assert_eq!(latest.changelog, "");
        assert_eq!(latest.name, "v1.0.1");
        assert_eq!(latest.download_url, None);
    }

    #[test]
    fn a_broken_answer_and_a_broken_version_are_told_apart() {
        assert_eq!(decide("1.0.0", "{").unwrap_err(), UpdateError::BadResponse);
        assert_eq!(decide("1.0.0", "{\"message\":\"x\"}").unwrap_err(), UpdateError::BadResponse);
        assert_eq!(decide("soon", "[]").unwrap_err(), UpdateError::BadVersion);
    }

    fn asset(name: &str) -> GhAsset {
        GhAsset { name: name.into(), browser_download_url: format!("https://example.test/{name}") }
    }

    #[test]
    fn the_installer_for_our_processor_wins_over_the_first_one() {
        let assets = [asset("Yaca_1_aarch64.dmg"), asset("Yaca_1_x64.dmg")];
        assert_eq!(
            pick_installer(&assets, &[".dmg"], &["x64", "x86_64"]).as_deref(),
            Some("https://example.test/Yaca_1_x64.dmg")
        );
        assert_eq!(
            pick_installer(&assets, &[".dmg"], &["aarch64", "arm64"]).as_deref(),
            Some("https://example.test/Yaca_1_aarch64.dmg")
        );
    }

    #[test]
    fn without_a_processor_in_the_name_the_first_installer_is_taken() {
        let assets = [asset("notes.txt"), asset("Yaca.AppImage")];
        assert_eq!(
            pick_installer(&assets, &[".AppImage", ".deb"], &["x64"]).as_deref(),
            Some("https://example.test/Yaca.AppImage")
        );
        assert_eq!(pick_installer(&assets, &[".dmg"], &["x64"]), None);
    }
}
