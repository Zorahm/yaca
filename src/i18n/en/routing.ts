/**
 * English strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "Router",
  "routing.lede":
    "A small model answers questions about each message, and rules pick which model replies. The routing.json file sits next to providers.json.",
  "routing.state_absent": "Not set up.",
  "routing.state_ready": "Connected: it is listed among the models as “{name}”.",
  "routing.state_broken": "Not connected: {error}",
  "routing.editor": "Contents of routing.json",
  "routing.save": "Save",
  "routing.remove": "Remove the router",
  "routing.saved": "Saved.",
  "routing.next_chat":
    "Chats that are already open keep working as they were; a new chat takes the router from this file.",
  "routing.default_route": "default",
  "routing.chip": "Route: {route} → {model}",
  "routing.menu_broken": "The router is not connected: {error}",
  "routing.wizard_title": "Quick setup",
  "routing.wizard_lede":
    "Pick three models and the editor fills with a ready setup: the strong model takes hard requests, everything else goes to the default one. Nothing is written until you save.",
  "routing.wizard_decider": "Model that answers the questions",
  "routing.wizard_default": "Default model",
  "routing.wizard_strong": "Strong model for hard requests",
  "routing.wizard_build": "Fill the editor",
  "routing.wizard_none": "No models yet: connect a provider first.",
  "routing.wizard_pick": "Choose a model",
  "routing.decision_hint":
    "The decision model is best taken from the Decision provider above: set its address, local or cloud, and ask it for models.",
} satisfies Record<keyof typeof source, string>;
