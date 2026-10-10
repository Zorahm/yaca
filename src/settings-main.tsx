/**
 * Точка входа окна настроек §10.
 *
 * Имя со строчной и с дефисом, а не `Settings.tsx`: на Windows пути
 * регистронезависимы, и точка входа затёрла бы сам компонент. Та же беда, из-за
 * которой бинарник оболочки зовётся `yaca-desktop`, — см. `src-tauri/Cargo.toml`.
 */

import React from "react";
import ReactDOM from "react-dom/client";

import Settings from "./Settings";
import { bootPrefs } from "./ui/prefs";

// Как и в главном окне: язык выбирает словарь, тема и акцент — токены, и оба
// обязаны стоять до первого кадра.
void bootPrefs().then((prefs) => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <Settings prefs={prefs} />
    </React.StrictMode>,
  );
});
