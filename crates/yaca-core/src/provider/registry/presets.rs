//! The providers Yaca ships with (§10): an address and a wire shape each,
//! waiting only for a key.
//!
//! There used to be two, one per wire shape, on the grounds that a third would
//! be a name and not a shape. That was true of the core and wrong for the
//! person: typing `https://generativelanguage.googleapis.com/v1beta/openai`
//! from memory is exactly the work a preset exists to remove.
//!
//! None of them carries a model list. A preset is not configured until its
//! address answered (see the module note in [`super`]), and the answer arrives
//! the moment a key is stored — [`crate::provider::models::refresh`].

use std::collections::BTreeMap;

use super::{Protocol, ProviderSpec};

pub const OPENAI_ID: &str = "openai";
pub const ANTHROPIC_ID: &str = "anthropic";
/// The place of the small model that answers the router's questions. Not a
/// service of its own: an address to be filled in with whatever serves that
/// model, a local server or a cloud one.
pub const DECISION_ID: &str = "decision";

struct Preset {
    id: &'static str,
    display_name: &'static str,
    protocol: Protocol,
    /// With the version segment where the provider's own path has one that is
    /// not `/v1` — see [`ProviderSpec::api`].
    base_url: &'static str,
    /// A server that never asks for a key. Written down the way the rest of
    /// the core writes it — `authorization` overridden to nothing — so the
    /// panel says "no key needed" and not "key missing". A preset that is
    /// pointed at a cloud service puts the header back in the form.
    keyless: bool,
}

/// In the order the panel lists them on a first launch.
const PRESETS: &[Preset] = &[
    Preset {
        id: OPENAI_ID,
        display_name: "OpenAI",
        protocol: Protocol::OpenAi,
        base_url: "https://api.openai.com",
        keyless: false,
    },
    Preset {
        id: ANTHROPIC_ID,
        display_name: "Anthropic",
        protocol: Protocol::Anthropic,
        base_url: "https://api.anthropic.com",
        keyless: false,
    },
    Preset {
        id: "google",
        display_name: "Google AI Studio",
        protocol: Protocol::OpenAi,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        keyless: false,
    },
    Preset {
        id: "openrouter",
        display_name: "OpenRouter",
        protocol: Protocol::OpenAi,
        base_url: "https://openrouter.ai/api",
        keyless: false,
    },
    Preset {
        id: "deepseek",
        display_name: "DeepSeek",
        protocol: Protocol::OpenAi,
        base_url: "https://api.deepseek.com",
        keyless: false,
    },
    Preset {
        // The coding plan has its own address; the pay-as-you-go one
        // (`/api/paas/v4`) refuses a coding-plan key.
        id: "zai",
        display_name: "z.ai (coding plan)",
        protocol: Protocol::OpenAi,
        base_url: "https://api.z.ai/api/coding/paas/v4",
        keyless: false,
    },
    Preset {
        // Ollama's address, where a small local model most often is. Changed
        // in the provider form to a llama.cpp or vLLM server, or to a cloud
        // one; the router asks this entry's models its questions.
        id: DECISION_ID,
        display_name: "Decision",
        protocol: Protocol::OpenAi,
        base_url: "http://localhost:11434",
        keyless: true,
    },
];

/// Is this one of the presets? They stay in the list: a delete that the next
/// load undoes is a button lying about what it did (§0).
pub fn is_preset(id: &str) -> bool {
    PRESETS.iter().any(|p| p.id == id)
}

/// Every preset, without models.
pub(super) fn all() -> Vec<ProviderSpec> {
    PRESETS.iter().map(spec).collect()
}

/// One preset by id, without models.
pub(super) fn get(id: &str) -> Option<ProviderSpec> {
    PRESETS.iter().find(|p| p.id == id).map(spec)
}

fn spec(preset: &Preset) -> ProviderSpec {
    ProviderSpec {
        id: preset.id.to_string(),
        display_name: preset.display_name.to_string(),
        protocol: preset.protocol,
        base_url: preset.base_url.to_string(),
        headers: if preset.keyless {
            BTreeMap::from([("authorization".to_string(), String::new())])
        } else {
            BTreeMap::new()
        },
        models: Vec::new(),
    }
}
