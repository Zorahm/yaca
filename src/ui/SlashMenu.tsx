/**
 * Меню слэш-команд §7.3 — дополнение над полем композера.
 *
 * Строится теми же классами, что и меню режима и модели: это тот же список
 * над тем же полем, и вторая внешность у него была бы без причины.
 *
 * Выбор берётся мышью на `mousedown`, а не на `click`: клик сначала уводит
 * фокус из поля, и человек, дополнивший команду мышью, оказывался бы с
 * дополненной строкой и без курсора в ней.
 */

import type { SlashItem } from "./slash";

export function SlashMenu({
  items,
  at,
  onPick,
}: {
  items: SlashItem[];
  /** Подсвеченная строка: её берут `⇥` и `⏎`. */
  at: number;
  onPick: (item: SlashItem) => void;
}) {
  return (
    <div className="menu menu-slash" role="listbox">
      {items.map((item, index) => (
        <button
          className="menu-item"
          key={item.name}
          role="option"
          aria-selected={index === at}
          onMouseDown={(event) => {
            event.preventDefault();
            onPick(item);
          }}
        >
          <span className="mono-target">/{item.name}</span>
          <span className="menu-note">{item.description}</span>
        </button>
      ))}
    </div>
  );
}
