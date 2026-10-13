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

struct Preset {
    id: &'static str,
    display_name: &'static str,
    protocol: Protocol,
    /// With the version segment where the provider's own path has one that is
    /// not `/v1` — see [`ProviderSpec::api`].
    base_url: &'static str,
}

/// In the order the panel lists them on a first launch.
const PRESETS: &[Preset] = &[
    Preset {
        id: OPENAI_ID,
        display_name: "OpenAI",
        protocol: Protocol::OpenAi,
        base_url: "https://api.openai.com",
    },
    Preset {
        id: ANTHROPIC_ID,
        display_name: "Anthropic",
        protocol: Protocol::Anthropic,
        base_url: "https://api.anthropic.com",
    },
    Preset {
        id: "google",
        display_name: "Google AI Studio",
        protocol: Protocol::OpenAi,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    },
    Preset {
        id: "openrouter",
        display_name: "OpenRouter",
        protocol: Protocol::OpenAi,
        base_url: "https://openrouter.ai/api",
    },
    Preset {
        id: "deepseek",
        display_name: "DeepSeek",
        protocol: Protocol::OpenAi,
        base_url: "https://api.deepseek.com",
    },
    Preset {
        // The coding plan has its own address; the pay-as-you-go one
        // (`/api/paas/v4`) refuses a coding-plan key.
        id: "zai",
        display_name: "z.ai (coding plan)",
        protocol: Protocol::OpenAi,
        base_url: "https://api.z.ai/api/coding/paas/v4",
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
        headers: BTreeMap::new(),
        models: Vec::new(),
    }
}
