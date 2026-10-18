import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { adoptStorage } from "./core/legacy";
import { bootPrefs } from "./ui/prefs";

// Before the first read: the composer takes its draft on mount (§7.4).
adoptStorage();

// Application settings §10 are applied before the first frame: the language picks
// the dictionary, the theme and accent pick the tokens. A window drawn in night and
// at once repainted into paper would flicker on every launch.
void bootPrefs().then((prefs) => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App prefs={prefs} />
    </React.StrictMode>,
  );
});
