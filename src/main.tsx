import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { adoptStorage } from "./core/legacy";
import { openLink } from "./ui/link/boot";
import { LinkLayers } from "./ui/link/LinkLayers";
import { bootPrefs } from "./ui/prefs";

// Before the first read: the composer takes its draft on mount (§7.4).
adoptStorage();

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

// Against a server nothing below works until the socket has accepted the token,
// and the preferences are on the server too: connecting comes first.
void openLink(root)
  // Application settings §10 are applied before the first frame: the language picks
  // the dictionary, the theme and accent pick the tokens. A window drawn in night and
  // at once repainted into paper would flicker on every launch.
  .then(() => bootPrefs())
  .then((prefs) => {
    root.render(
      <React.StrictMode>
        <App prefs={prefs} />
        <LinkLayers />
      </React.StrictMode>,
    );
  });
