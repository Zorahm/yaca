/**
 * The commands of a session row, as menu items.
 *
 * One list with two ways in: the `⋯` of the row opens it as a dropdown, a
 * right click opens it at the pointer. Radix has a primitive for each, and
 * their items take the same props — so the list is written once and handed
 * the `Item` of whichever shell is drawing it. Two lists would drift apart.
 */

import {
  Copy,
  DownloadSimple,
  PencilSimple,
  Trash,
  X,
} from "@phosphor-icons/react";
import type { DropdownMenu } from "radix-ui";

import { t } from "../../i18n";
import { useFlash } from "../useFlash";

export type SessionCommands = {
  /** Only a chat that is open in the window can be closed. */
  onClose: (() => void) | null;
  onRename: () => void;
  onDuplicate: () => void;
  /** Asks where to save and saves. `false` — the person changed their mind. */
  onExport: () => Promise<boolean>;
  onError: (error: unknown) => void;
  /** The first press on delete only asks; this says the question is up. */
  confirming: boolean;
  onDeleteStart: () => void;
  onDelete: () => void;
};

export function SessionMenuItems({
  Item,
  commands,
}: {
  /** `DropdownMenu.Item` or `ContextMenu.Item`: the same props, either shell. */
  Item: typeof DropdownMenu.Item;
  commands: SessionCommands;
}) {
  const { shown: saved, flash } = useFlash();
  const { onClose, confirming } = commands;

  return (
    <>
      <Item className="pop-item" onSelect={commands.onRename}>
        <PencilSimple size={14} />
        {t("sess.rename")}
      </Item>
      <Item className="pop-item" onSelect={commands.onDuplicate}>
        <Copy size={14} />
        {t("sess.duplicate")}
      </Item>
      <Item
        className="pop-item"
        onSelect={(e) => {
          // The menu stays open: «saved» has to be shown somewhere, and the
          // save dialog has taken the focus anyway.
          e.preventDefault();
          void commands
            .onExport()
            .then((done) => done && flash())
            .catch(commands.onError);
        }}
      >
        <DownloadSimple size={14} />
        {saved ? t("sess.exported") : t("sess.export")}
      </Item>
      {/* Closing stops the session and leaves the journal: it is offered for
          a chat that is open and is not there at all for one that is not. */}
      {onClose && (
        <Item className="pop-item" onSelect={onClose}>
          <X size={14} />
          {t("side.close_chat")}
        </Item>
      )}
      <div className="pop-sep" />
      <Item
        className="pop-item pop-item-danger"
        onSelect={(e) => {
          if (confirming) {
            commands.onDelete();
            return;
          }
          // The first press does not delete: the menu stays open and the item
          // becomes a question. What cannot be undone takes a second press.
          e.preventDefault();
          commands.onDeleteStart();
        }}
      >
        <Trash size={14} />
        {confirming ? t("sess.delete_confirm") : t("sess.delete")}
      </Item>
    </>
  );
}
