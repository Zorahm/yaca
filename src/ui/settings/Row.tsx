/**
 * Forms of the §10 settings window: a row, a switch, a multi-way toggle, a dropdown.
 *
 * A row applies at once — the section has no "Save" button. It would let the window
 * show one thing while the disk held another, and §0 forbids the interface to lie
 * about its state. The document — the provider description — is the exception and
 * lives with its own "Save" and "Cancel": a half-written address must not reach the
 * disk.
 */

import type { ReactNode } from "react";
import { CaretUpDown, Check } from "@phosphor-icons/react";
import { Select as RadixSelect } from "radix-ui";

/** A settings row: name and hint on the left, control on the right. */
export function Row({
  name,
  hint,
  children,
}: {
  name: ReactNode;
  /** Why this is needed and what will happen. Not a restatement of the name. */
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
 * The switch. Radix is not needed here: `role="switch"` with `aria-checked` is the
 * whole contract, and the keyboard comes free with a real `<button>`.
 */
export function Switch({
  on,
  label,
  onChange,
}: {
  on: boolean;
  /** Accessible name: the switch has no text of its own (§14). */
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
 * Multi-way toggle. The same `.segment` as the bottom row of §7.1: one form per
 * window, not its own in every section.
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
