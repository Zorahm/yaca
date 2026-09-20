import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { adoptStorage } from "./core/legacy";
import { locale } from "./i18n";

// §14: скринридер должен знать, на каком языке читает. Язык выбирается в
// разработке один раз, но разметка о нём знать обязана.
document.documentElement.lang = locale;

// До первого чтения: композер берёт черновик при монтировании (§7.4).
adoptStorage();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
