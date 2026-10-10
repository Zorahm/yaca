import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { adoptStorage } from "./core/legacy";
import { bootPrefs } from "./ui/prefs";

// До первого чтения: композер берёт черновик при монтировании (§7.4).
adoptStorage();

// Настройки приложения §10 прикладываются до первого кадра: язык выбирает
// словарь, тема и акцент — токены. Окно, нарисованное в Ночи и тут же
// перекрашенное в Бумагу, моргало бы на каждом запуске.
void bootPrefs().then((prefs) => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App prefs={prefs} />
    </React.StrictMode>,
  );
});
