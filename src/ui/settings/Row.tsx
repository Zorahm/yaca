/**
 * Формы окна настроек §10: строка, выключатель, переключатель из нескольких,
 * выпадающий список.
 *
 * Строка применяется сразу — кнопки «Сохранить» у раздела нет. Она разрешила
 * бы окну показывать одно, а диску держать другое, а §0 запрещает интерфейсу
 * врать про состояние. Документ — описание провайдера — исключение и живёт со
 * своими «Сохранить» и «Отмена»: недописанный адрес на диск попадать не
 * должен.
 */

import type { ReactNode } from "react";
import { CaretUpDown, Check } from "@phosphor-icons/react";
import { Select as RadixSelect } from "radix-ui";

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

/**
 * One of many, folded into a single control. For a list that grows — the
 * languages, the models — where `Segments` would outgrow the row: the control
 * column is 320px wide and every option there costs a button.
 *
 * Radix, unlike `Switch`: a listbox needs typeahead, arrow keys and focus
 * return, and that is more contract than a `<button>` gives away for free.
 * Not the native `<select>` either: its list is drawn by the OS and ignores
 * the theme.
 *
 * No option may have the value `""`: Radix keeps the empty string for "nothing
 * chosen". A caller with a "not set" choice names it with a sentinel.
 */
export function Select<T extends string>({
  label,
  value,
  options,
  mono = false,
  align = "end",
  className,
  onChange,
}: {
  /** Accessible name: the trigger shows the value, not what it is a value of. */
  label: string;
  value: T;
  options: { value: T; label: string }[];
  /** Identifiers — model names — are set in mono, words are not. */
  mono?: boolean;
  /**
   * Which edge of the trigger the list hangs from. `end` suits a row, where
   * the control sits at the right; a select at the left edge wants `start`.
   */
  align?: "start" | "end";
  /** For a select outside a `Row`, which has its own shape to keep. */
  className?: string;
  onChange: (value: T) => void;
}) {
  const face = mono ? " select-mono" : "";
  const shape = className ? ` ${className}` : "";
  return (
    // Radix hands back a plain string; it can only be one of `options`.
    <RadixSelect.Root value={value} onValueChange={(next) => onChange(next as T)}>
      <RadixSelect.Trigger className={`select${face}${shape}`} aria-label={label}>
        <span className="select-value">
          <RadixSelect.Value />
        </span>
        <RadixSelect.Icon className="select-caret">
          <CaretUpDown size={12} />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal>
        <RadixSelect.Content
          className={`pop select-list${face}`}
          position="popper"
          align={align}
          sideOffset={4}
        >
          <RadixSelect.Viewport>
            {options.map((option, index) => (
              // The index is in the key because two providers may serve a
              // model under one id.
              <RadixSelect.Item
                className="pop-item"
                key={`${option.value}/${index}`}
                value={option.value}
              >
                <RadixSelect.ItemText>{option.label}</RadixSelect.ItemText>
                <RadixSelect.ItemIndicator className="select-mark">
                  <Check size={13} weight="bold" />
                </RadixSelect.ItemIndicator>
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
