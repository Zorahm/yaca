/**
 * Каркас выдвижной панели §1.1 на Radix Dialog.
 *
 * Radix отвечает только за поведение: ловушка фокуса, `Esc`, dismiss по клику
 * вне, роли и `aria-*` (§14). Внешний вид — токены §2 в `panel.css`; состояния
 * читаются с `data-state`, который Radix ставит сам.
 *
 * `modal={true}` нормативен: лента становится inert, фокус заперт в панели.
 *
 * «Открыта всегда одна» и «закрывается повторным нажатием той же кнопки»
 * решаются снаружи: одна `panel`-строка состояния в `App`, триггеры — обычные
 * кнопки на контролируемом `open`. Отсюда `onInteractOutside` ниже: pointerdown
 * по триггеру игнорируется, иначе клик по открытой панели сперва закрыл бы её
 * как клик вне, а потом кнопка открыла бы снова.
 */

import { X } from "@phosphor-icons/react";
import { Dialog } from "radix-ui";
import type { ReactNode } from "react";

import { t } from "../../i18n";

export function Panel({
  open,
  onOpenChange,
  title,
  children,
  className,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Заголовок и доступное имя панели (§14): путь у файла, слово у списков. */
  title: ReactNode;
  children: ReactNode;
  /** Модификатор каркаса: на домашнем экране (§9.1) верхней панели нет,
      и панель `panel-cover` занимает всю высоту окна. */
  className?: string;
}) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange} modal={true}>
      <Dialog.Portal>
        <Dialog.Content
          className={className === undefined ? "panel" : `panel ${className}`}
          aria-describedby={undefined}
          onInteractOutside={(event) => {
            const target = event.target;
            if (
              target instanceof Element &&
              target.closest("[data-panel-trigger]")
            ) {
              event.preventDefault();
            }
          }}
        >
          <header className="panel-head">
            <Dialog.Title className="panel-title">{title}</Dialog.Title>
            <Dialog.Close
              className="top-icon"
              aria-label={t("common.close")}
              title={t("common.close")}
            >
              <X size={14} />
            </Dialog.Close>
          </header>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
