/**
 * Формы окна настроек §10: строка, выключатель, переключатель из нескольких.
 *
 * Строка применяется сразу — кнопки «Сохранить» у раздела нет. Она разрешила
 * бы окну показывать одно, а диску держать другое, а §0 запрещает интерфейсу
 * врать про состояние. Документ — описание провайдера — исключение и живёт со
 * своими «Сохранить» и «Отмена»: недописанный адрес на диск попадать не
 * должен.
 */

import type { ReactNode } from "react";

/** Строка настройки: имя и подпись слева, контрол справа. */
export function Row({
  name,
  hint,
  children,
}: {
  name: ReactNode;
  /** Зачем это нужно и что случится. Не пересказ имени. */
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="set-row">
      <div className="set-row-text">
        <span className="set-row-name">{name}</span>
        {hint !== undefined && <span className="set-row-hint">{hint}</span>}
      </div>
      <div className="set-row-control">{children}</div>
    </div>
  );
}

/**
 * Выключатель. Radix здесь не нужен: `role="switch"` с `aria-checked` — это
 * весь контракт, а клавиатура достаётся от настоящей `<button>`.
 */
export function Switch({
  on,
  label,
  onChange,
}: {
  on: boolean;
  /** Доступное имя: у выключателя нет своего текста (§14). */
  label: string;
  onChange: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      className="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
    />
  );
}

/**
 * Переключатель из нескольких. Тот же `.segment`, что в нижнем ряду §7.1:
 * одна форма на окно, а не своя в каждом разделе.
 */
export function Segments<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string; title?: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="segments" role="group" aria-label={label}>
      {options.map((option) => (
        <button
          type="button"
          key={option.value}
          className="segment"
          aria-pressed={value === option.value}
          title={option.title}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
