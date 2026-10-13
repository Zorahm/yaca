/**
 * English strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "Providers",
  "prov.home_button": "Configure providers",
  "prov.add": "Add provider",
  "prov.edit_row": "Configure",
  "prov.delete": "Delete",
  "prov.delete_confirm": "Delete forever?",
  "prov.models_count.one": "{n} model",
  "prov.models_count.few": "{n} models",
  "prov.models_count.many": "{n} models",

  "prov.form_new": "New provider",
  "prov.form_name": "Name",
  "prov.form_more": "Advanced",
  "prov.form_protocol": "Protocol",
  "prov.form_base_url": "Base URL",
  "prov.form_base_url_hint":
    "The protocol path is added for you: /v1/chat/completions for OpenAI, /v1/messages for Anthropic. For OpenRouter that is https://openrouter.ai/api. An address that already names a version (/v1, /v1beta, /v4) gets no /v1.",
  "prov.form_headers": "Custom headers",
  "prov.form_headers_hint":
    "Added over the protocol's own headers and may replace them — including authorization. A {key} in a value gets the key from the keychain; the key itself never reaches the file. An empty value drops a protocol header.",
  "prov.form_header_name": "Name",
  "prov.form_header_value": "Value",
  "prov.form_add_header": "Add header",
  "prov.form_remove_header": "Remove header",

  "prov.form_key_title": "Key",
  "prov.form_key_hint":
    "Goes to the system keychain under the provider's name — providers.json never holds it",
  "prov.form_key_new_hint":
    "Once saved, the provider is asked for its models: whatever it answers is what the model switcher offers",
  "prov.key_save": "Save key",
  "prov.key_forget": "Forget key",
  "prov.key_source_keychain": "key is in the system keychain",
  "prov.key_source_environment": "key is in an environment variable",
  "prov.key_source_none": "no key",
  "prov.key_source_unknown": "the source becomes visible once the provider is saved",
  "prov.key_not_needed": "No key needed: {key} appears in no header",

  "prov.form_models": "Models",
  "prov.form_models_hint":
    "The list comes from the provider itself: it is asked as soon as a key is saved, and Ask asks again. Whatever it answers is what Yaca talks to; the price is dollars per million tokens, and zero means free or unknown.",
  "prov.form_model_context": "Context, tokens",
  "prov.form_model_output": "Max output, tokens",
  "prov.form_model_effort": "supports effort",
  "prov.models_ask": "Ask",
  "prov.models_asking": "Asking…",
  "prov.models_none": "No models",
  "prov.models_price": "$ / 1M in and out",
  "prov.models_effort": "effort",

  "prov.form_save": "Save provider",
  "prov.form_cancel": "Cancel",

  "prov.saved_unasked":
    "“{name}” is saved, but did not answer with its models: {error}",
  "prov.error_name": "Give the provider a name",
  "prov.error_url": "Give the provider a base URL",
  "prov.error_header_name": "Header {n} has no name",
  "prov.error_header_dup": "Header {name} is listed twice",

  "prov.lede": "Where the conversation and the key are sent. Application level: a project does not decide this.",
  "prov.answered": "Answered with its models",
  "prov.unasked": "not asked yet",
  "prov.unasked_note":
    "A hollow dot means the provider never answered with a model list. It will not appear in the model switcher: a session would have nothing to send a request with.",
} satisfies Record<keyof typeof source, string>;
