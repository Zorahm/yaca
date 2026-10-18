//! Which providers Yaca can talk to, and how (§10).
//!
//! One file, `providers.json`, in the application's config directory. It is
//! application-level and not project-level on purpose: a project does not get
//! to decide where the conversation and the key are sent.
//!
//! Keys are never in this file. A header value may contain `{key}`, and the
//! placeholder is filled from the system keychain under the provider's id at
//! request time — see [`crate::secrets`]. Writing `Authorization: Bearer …`
//! with a live token would put that token in plain JSON, which is the one
//! thing the keychain exists to prevent.
//!
//! A key is never required up front. With none stored, the header that waits
//! for it is left out and the request goes anyway: a local server never asks,
//! and one that does answers 401, which reaches the human as an error naming
//! the missing key. Demanding keys first meant configuring providers nobody
//! was about to use.
//!
//! A provider is configured when its address answered with its models (§10).
//! That is what the model switcher offers and what a new chat starts on; a
//! list written by hand into the code is not an answer.
//!
//! A file that does not parse stops the caller instead of falling back to the
//! built-in Anthropic entry. This is the same rule the core applies to project
//! settings and for a sharper reason: a typo in `baseUrl` would quietly send
//! the conversation somewhere the human did not choose.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;
use crate::legacy;
use crate::usage::ModelInfo;

const FILE: &str = "providers.json";

/// The application's own directory inside the platform's config directory.
const CONFIG_DIR: &str = "Yaca";

mod presets;

pub use presets::{is_preset, ANTHROPIC_ID, DECISION_ID, OPENAI_ID};

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// What a header value writes to ask for the provider's key.
pub const KEY_PLACEHOLDER: &str = "{key}";

/// The wire shape a provider speaks.
///
/// Not a guess from the URL: OpenRouter and LiteLLM serve both shapes from
/// neighbouring paths on one host, and guessing wrong fails at the first tool
/// call rather than at the first request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// Messages API: `POST {base_url}/v1/messages` — see [`ProviderSpec::api`]
    /// for when the `/v1` is left out.
    #[default]
    Anthropic,
    /// Chat Completions: `POST {base_url}/v1/chat/completions`.
    #[serde(rename = "openai")]
    OpenAi,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSpec {
    /// Also the name the key is kept under in the keychain.
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub protocol: Protocol,
    pub base_url: String,
    /// Added to every request, over the protocol's own defaults — so the
    /// authorisation header can be replaced rather than only extended.
    /// A value containing `{key}` gets the key substituted in.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// What the model switcher (§7.1) offers, and what the cost is counted by.
    /// Filled by asking the provider (`crate::provider::models`); a provider
    /// with none has not answered yet and cannot be chosen: there would be
    /// nothing to send the request as.
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

impl ProviderSpec {
    /// The preset Anthropic entry, without models.
    ///
    /// It used to ship the four current models written into the code, so the
    /// switcher had something in it before any key existed. That made a
    /// provider nobody had set up look configured, and a new chat started on
    /// it: talking to a local server meant getting an Anthropic key first.
    /// The list arrives when the address answers — see
    /// [`crate::provider::models`].
    pub fn anthropic() -> Self {
        // Invariant: `ANTHROPIC_ID` is an entry of the preset table, which
        // `presets_are_all_there_by_id` checks.
        presets::get(ANTHROPIC_ID).expect("the Anthropic preset")
    }

    /// The preset OpenAI entry.
    ///
    /// Without a model list, and that is not an omission: OpenAI's
    /// `/v1/models` answers ids and nothing else, so a catalog written here by
    /// hand would be exactly the guesswork the panel stopped doing. The list
    /// arrives with the key — see [`crate::provider::models`] — and until then
    /// the entry says plainly that it has no models rather than offering ones
    /// nobody checked.
    pub fn openai() -> Self {
        // Invariant: as for `anthropic` above.
        presets::get(OPENAI_ID).expect("the OpenAI preset")
    }

    pub fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The model a session should actually ask for.
    ///
    /// If empty, the provider's first model is picked. An unknown model is an
    /// error: the model carries its provider, so there is no mismatch fallback.
    pub fn resolve_model(&self, wanted: &str) -> Result<String, CoreError> {
        if wanted.is_empty() {
            return self.models.first().map(|m| m.id.clone()).ok_or_else(|| {
                CoreError::Config(format!(
                    "у провайдера «{}» не описано ни одной модели",
                    self.id
                ))
            });
        }
        if self.model(wanted).is_some() {
            return Ok(wanted.to_string());
        }
        Err(CoreError::Config(format!(
            "у провайдера «{}» нет модели «{wanted}»",
            self.id
        )))
    }

    /// `base_url` without its trailing slash, so joining a path is one rule.
    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }

    /// Where a path of the protocol lives under this provider's address:
    /// `api("/models")` is `{base_url}/v1/models`.
    ///
    /// The `/v1` is left out when the address already names a version. Google
    /// serves this shape under `/v1beta/openai` and z.ai under `/v4`, and
    /// neither has a `/v1` below that; the same rule forgives the commonest
    /// slip in the form, a base URL pasted with its `/v1` on.
    pub fn api(&self, path: &str) -> String {
        if names_a_version(&self.base_url) {
            self.endpoint(path)
        } else {
            self.endpoint(&format!("/v1{path}"))
        }
    }

    /// Does any header ask for a key?
    ///
    /// A local gateway needs none, and demanding one would keep Yaca from
    /// talking to an Ollama on the same machine for no reason.
    pub fn needs_key(&self) -> bool {
        self.all_headers()
            .values()
            .any(|v| v.contains(KEY_PLACEHOLDER))
    }

    /// Headers as they go out, with `{key}` filled in.
    ///
    /// Without a key the header waiting for one is left out rather than sent
    /// half-filled: `Bearer ` with nothing after it is a malformed credential,
    /// while no credential is a question the server can answer (module note).
    pub fn headers_for(&self, key: Option<&str>) -> Vec<(String, String)> {
        let key = key.filter(|k| !k.trim().is_empty());
        self.all_headers()
            .into_iter()
            .filter_map(|(name, value)| {
                if !value.contains(KEY_PLACEHOLDER) {
                    return Some((name, value));
                }
                let filled = value.replace(KEY_PLACEHOLDER, key?);
                Some((name, filled))
            })
            .collect()
    }

    /// Protocol defaults first, the spec's own over them.
    fn all_headers(&self) -> BTreeMap<String, String> {
        let mut all: BTreeMap<String, String> = protocol_headers(self.protocol)
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        all.extend(self.headers.iter().map(|(k, v)| (k.clone(), v.clone())));
        all
    }
}

/// Is any segment of the address's path a version: `v1`, `v1beta`, `v4`?
///
/// The host is skipped, so `v2.example.com` is not one.
fn names_a_version(base_url: &str) -> bool {
    let after_scheme = base_url.split_once("://").map_or(base_url, |(_, rest)| rest);
    after_scheme.split('/').skip(1).any(|segment| {
        let mut chars = segment.chars();
        chars.next() == Some('v')
            && chars.next().is_some_and(|c| c.is_ascii_digit())
            && chars.all(|c| c.is_ascii_alphanumeric())
    })
}

/// What each shape sends when the spec says nothing.
///
/// `content-type` and `accept` are here rather than in the client so that a
/// gateway which insists on something else can be told, in one place, without
/// the request carrying the header twice.
fn protocol_headers(protocol: Protocol) -> &'static [(&'static str, &'static str)] {
    match protocol {
        Protocol::Anthropic => &[
            ("accept", "text/event-stream"),
            ("anthropic-version", ANTHROPIC_VERSION),
            ("content-type", "application/json"),
            ("x-api-key", KEY_PLACEHOLDER),
        ],
        Protocol::OpenAi => &[
            ("accept", "text/event-stream"),
            ("authorization", "Bearer {key}"),
            ("content-type", "application/json"),
        ],
    }
}

/// Everything Yaca can talk to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Providers {
    pub list: Vec<ProviderSpec>,
}

impl Default for Providers {
    fn default() -> Self {
        Providers {
            list: presets::all(),
        }
    }
}

impl Providers {
    /// Reads the file, or the built-in defaults when there is none.
    ///
    /// A file that does not parse is an error and not a fallback: see the
    /// module note.
    pub fn load(dir: &Path) -> Result<Self, CoreError> {
        let path = dir.join(FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Providers::default()),
            Err(e) => return Err(CoreError::Io(e)),
        };

        let mut providers: Providers = serde_json::from_str(&text)
            .map_err(|e| CoreError::Config(format!("{} не разобрался: {e}", path.display())))?;
        providers.ensure_presets();
        providers.forget_shipped_catalog();
        Ok(providers)
    }

    /// Written through a temporary file, so a crash mid-write cannot leave a
    /// file that stops the next launch.
    pub fn save(&self, dir: &Path) -> Result<(), CoreError> {
        fs::create_dir_all(dir)?;
        let text = serde_json::to_string_pretty(self)?;
        let temp = dir.join(format!("{FILE}.tmp"));
        fs::write(&temp, text)?;
        fs::rename(temp, dir.join(FILE))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ProviderSpec> {
        self.list.iter().find(|p| p.id == id)
    }

    /// Which provider a session talks to, and the model to ask it for.
    ///
    /// The window and the CLI both come through here, so the rule exists once.
    ///
    /// An empty `provider` means the source did not say: a session recorded
    /// before sessions wrote their provider down, a project file that names
    /// only a model. The model decides then, because it carries its provider.
    /// Filling the gap with Anthropic instead is what sent a local model to
    /// Anthropic, which came back with «у провайдера «anthropic» нет модели».
    ///
    /// Neither said — a new chat in a project with no settings — starts on
    /// the first configured provider, the first that answered with its
    /// models. None configured, nothing starts, and the error says what to do.
    ///
    /// A provider that is named and missing is an error, not a search by model:
    /// another provider serving the same id would get a conversation the human
    /// sent somewhere else.
    pub fn resolve(&self, provider: &str, model: &str) -> Result<(&ProviderSpec, String), CoreError> {
        let spec = if !provider.is_empty() {
            self.get(provider)
                .ok_or_else(|| CoreError::Config(format!("провайдера «{provider}» нет в списке")))?
        } else if !model.is_empty() {
            self.find_by_model(model).map(|(spec, _)| spec).ok_or_else(|| {
                CoreError::Config(format!("модели «{model}» нет ни у одного провайдера"))
            })?
        } else {
            self.list.iter().find(|p| !p.models.is_empty()).ok_or_else(|| {
                CoreError::Config(
                    "ни один провайдер ещё не ответил списком моделей: опросите провайдера в панели провайдеров".into(),
                )
            })?
        };
        Ok((spec, spec.resolve_model(model)?))
    }

    /// Finds a provider and model info by model id.
    pub fn find_by_model(&self, model_id: &str) -> Option<(&ProviderSpec, &ModelInfo)> {
        for spec in &self.list {
            if let Some(m) = spec.model(model_id) {
                return Some((spec, m));
            }
        }
        None
    }

    /// All models across all configured providers, with provider_id attached.
    pub fn all_models(&self) -> Vec<ModelInfo> {
        let mut all = Vec::new();
        for spec in &self.list {
            for mut m in spec.models.clone() {
                m.provider_id = spec.id.clone();
                all.push(m);
            }
        }
        all
    }

    /// Adds or replaces by id, keeping the list ordered by insertion.
    pub fn put(&mut self, spec: ProviderSpec) {
        match self.list.iter_mut().find(|p| p.id == spec.id) {
            Some(slot) => *slot = spec,
            None => self.list.push(spec),
        }
    }

    /// Removes a provider. The presets cannot go: [`ensure_presets`] would put
    /// them back on the next load, and a delete the next launch undoes is a
    /// button lying about what it did.
    ///
    /// [`ensure_presets`]: Providers::ensure_presets
    pub fn remove(&mut self, id: &str) -> Result<(), CoreError> {
        if is_preset(id) {
            return Err(CoreError::Config(
                "предустановленный провайдер не удаляется".into(),
            ));
        }
        self.list.retain(|p| p.id != id);
        Ok(())
    }

    /// Drops the models the Anthropic preset used to ship.
    ///
    /// They were written into the code, not answered by the address, and a
    /// file saved back then still carries them: without this the provider
    /// would look configured and a new chat would start on it. Exactly that
    /// list and nothing else — asking Anthropic gives a longer one.
    fn forget_shipped_catalog(&mut self) {
        const SHIPPED: [&str; 4] = [
            "claude-opus-5",
            "claude-opus-4-8",
            "claude-sonnet-5",
            "claude-haiku-4-5",
        ];
        if let Some(spec) = self.list.iter_mut().find(|p| p.id == ANTHROPIC_ID) {
            if spec.models.iter().map(|m| m.id.as_str()).eq(SHIPPED) {
                spec.models.clear();
            }
        }
    }

    /// Puts back the presets the file does not have: ones a hand-written file
    /// left out, and ones that did not exist when it was saved.
    ///
    /// At the end, in their own order. What the file lists stays where its
    /// author put it — the first provider that answered is what a new chat
    /// starts on, and a preset arriving with an update must not move that.
    fn ensure_presets(&mut self) {
        for preset in presets::all() {
            if self.get(&preset.id).is_none() {
                self.list.push(preset);
            }
        }
    }
}

/// Where `providers.json` and the rest of the application's settings live.
///
/// Computed here rather than taken from the shell so that `yaca-cli` and the
/// window look in the same place. A CLI that could not reach the providers the
/// window configured would make the core untestable without a webview, which
/// is the one thing the crate boundary exists to prevent.
///
/// On Windows this is `%LOCALAPPDATA%\Yaca`, the folder the installer puts
/// the program and its uninstaller in: settings and chats (`store::location`)
/// sit beside them, so "delete all data" in the uninstaller has one folder to
/// take. Roaming would carry chat journals between machines, which is not
/// what they are for.
///
/// The directory the edoC build used is moved here on the first call, for the
/// same reason: whichever of the two starts first finds it — see
/// [`crate::legacy`]. On Windows the edoC settings were in Roaming, so that is
/// where they are looked for.
pub fn config_dir() -> Result<PathBuf, CoreError> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    let (old_base, base) = if cfg!(windows) {
        (var("APPDATA"), var("LOCALAPPDATA"))
    } else if cfg!(target_os = "macos") {
        let base = var("HOME").map(|h| h.join("Library/Application Support"));
        (base.clone(), base)
    } else {
        let base = var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")));
        (base.clone(), base)
    };

    let missing = || CoreError::Config("не нашёлся каталог настроек пользователя".into());
    let base = base.ok_or_else(missing)?;
    let old_base = old_base.unwrap_or_else(|| base.clone());
    Ok(legacy::adopt_dir(&old_base.join(legacy::NAME), &base.join(CONFIG_DIR)))
}

#[cfg(test)]
mod tests;
