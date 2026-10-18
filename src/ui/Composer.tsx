/**
 * Composer §7 and its bottom row §7.1.
 *
 * The field is not locked while the agent works (§7.4): what is sent goes into
 * the core's queue and is shown as a chip above the field. The chips are the
 * core's whole list: each shows a waiting message, and "cancel" sends the command
 * with its id. The core answers with a new list, so a chip says only what is true.
 *
 * The bottom row's settings come in as a prop, not taken from `SessionState`:
 * §9.1 puts the same composer on the home screen, where there is no session yet,
 * and what is picked there rides into the session with the first message.
 *
 * What is not here from §7.1: the dictation button. There is no speech
 * recognition, and a button that silently does nothing is an interface lying
 * about the state. `＋` is here: the shell's file dialog was found. `@` (§7.2) is
 * here: the project's file menu (`./MentionMenu`) opens when `@` starts a word;
 * the chosen file becomes a chip above the field, as one attached via `＋` does.
 * On the home screen there is no search, and the menu does not open there at all.
 *
 * `/` at the start of a line opens the command menu §7.3 (`./slash`). The menu is
 * not a state but a function of the field's text: `⇥` and `⏎` take the
 * highlighted item, a space closes it — what follows are arguments.
 */

import { ArrowUp, Plus, Stop } from "@phosphor-icons/react";
import { useEffect, useRef, useState, type ClipboardEvent } from "react";
import { createPortal } from "react-dom";

import type { Effort, Mode, ModelInfo, UsageSnapshot } from "../core/client";
import type { Feed } from "../core/feed";
import type { QueuedMessage } from "../core/bindings/QueuedMessage";
import { t } from "../i18n";
import { EffortMenu } from "./EffortMenu";
import { ModeMenu, MODE_KEY } from "./ModeMenu";
import { ModelMenu } from "./ModelMenu";
import { MentionMenu, useMentions } from "./MentionMenu";
import { withoutMention, type Mention } from "./mention";
import { QueuedChip } from "./QueuedChip";
import { SlashMenu } from "./SlashMenu";
import { Attachment } from "./Attachment";
import { builtinFor, menuFor, type Builtin, type MenuCommand, type SlashItem } from "./slash";
import { ContextWindow } from "./ContextWindow";
import { ContextRing } from "./ContextRing";


export type ComposerActions = {
  send: (text: string, attachments: string[]) => void;
  stop: () => void;
  setMode: (mode: Mode) => void;
  /**
   * Bypassing permissions (§8) is a decision about a running session. Where there
   * is no session, there is no action either, and the mode menu shows no row that
   * has nowhere to attach.
   */
  setBypass?: (on: boolean) => void;
  setModel: (model: string, providerId?: string) => void;
  setEffort: (effort: Effort) => void;
  /**
   * `/compact` (§5.11) and `/clear` are about the conversation in progress, and on
   * the home screen (§9.1) they are absent: the menu shows no rows that have
   * nowhere to go.
   */
  compact?: () => void;
  newChat?: () => void;
  /** `/checkpoints` (§7.3, §1.1) — the points of this conversation, not of the project. */
  openCheckpoints?: () => void;
  /** `/mcp` (§7.3) — the servers of this conversation and the state of each. */
  openMcp?: () => void;
  openLsp?: () => void;
  /**
   * `＋` (§7.2): ask for files and return the paths from the project root.
   *
   * The dialog belongs to the shell, and the root to the screen: the composer is
   * the same in a session and on the home screen, and it has no need to know about
   * the root. No action — no button either.
   */
  pickFiles?: () => Promise<string[]>;
  /**
   * Save the picture pasted from the clipboard (base64) and return its path.
   * A pasted picture has no path, and an attachment is a path (§7.2). No action —
   * pasting a picture stays an ordinary paste, that is, nothing.
   */
  stashImage?: (data: string) => Promise<string>;
  /**
   * Subscribe to files thrown into the window; the paths already come from the
   * project root. Returns the unsubscribe.
   */
  onDropped?: (listener: (paths: string[]) => void) => () => void;
  /**
   * Subscribe to "files now over the window". While they are there, the window is
   * covered by a veil with a hint: a drop may go anywhere, and the person needs to
   * see that the drop will be accepted.
   */
  onDragging?: (listener: (over: boolean) => void) => () => void;
  /**
   * Re-read the project's command list. Called when a line becomes a command: a
   * file written a minute ago must be in the menu, not only in the core, which
   * reads the catalog on every message.
   */
  refreshCommands?: () => void;
  /**
   * `@` (§7.2): ask the core about the project's files by a query. The answer comes
   * into the feed, not here. No action — there is no session on the screen, and the
   * menu does not open.
   */
  searchFiles?: (query: string) => void;
  /**
   * "cancel" on the queue chip (§7.4): remove the message that has not yet been
   * merged into the turn. No action — no button; the chip only shows that it waits.
   */
  cancelQueued?: (id: string) => void;
};

/** What the bottom row of §7.1 shows and switches. */
export type ComposerSettings = {
  mode: Mode;
  provider?: string;
  model: string;
  effort: Effort;
  bypass: boolean;
};

export function Composer({
  settings,
  busy,
  usage,
  models,
  commands,
  queued,
  found,
  actions,
  disabled = false,
  draftKey,
}: {
  settings: ComposerSettings;
  busy: boolean;
  usage: UsageSnapshot | null;
  models: ModelInfo[];
  /** The project's slash commands (§7.3) — the second half of the menu above the field. */
  commands: MenuCommand[];
  /** Messages waiting for the core's queue (§7.4), in order. */
  queued: QueuedMessage[];
  /** The last `@` search reply (§7.2); shown only if the query is the same. */
  found: Feed["found"];
  actions: ComposerActions;
  /**
   * There is nowhere to send: on the home screen (§9.1), until a project is picked.
   * The field stays silent and does not accept text that has nowhere to go.
   */
  disabled?: boolean;
  /** The draft is remembered per session and survives a restart (§7.4). */
  draftKey: string;
}) {
  const [text, setText] = useState(() => localStorage.getItem(draftKey) ?? "");
  const [menu, setMenu] = useState<"mode" | "model" | "effort" | "context" | null>(null);

  // A menu closes on a press anywhere outside it, not only on the composer's
  // own band: the feed, the sidebar and the empty ground count too. The chip
  // that opened it is inside a `.menu-anchor`, so its own toggle still works.
  useEffect(() => {
    if (menu === null) return;
    const close = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest(".menu, .menu-anchor")) return;
      setMenu(null);
    };
    document.addEventListener("pointerdown", close, true);
    return () => document.removeEventListener("pointerdown", close, true);
  }, [menu]);
  const [slashAt, setSlashAt] = useState(0);
  /** Attached files §7.2 — chips above the field; they travel with the message. */
  const [attached, setAttached] = useState<string[]>([]);
  /** An attachment is attached once, however many times it is dropped. */
  const attach = (paths: string[]) =>
    setAttached((now) => [...now, ...paths.filter((p) => !now.includes(p))]);

  const onDropped = actions.onDropped;
  useEffect(() => {
    if (onDropped === undefined || disabled) return;
    return onDropped(attach);
  }, [onDropped, disabled]);

  // The veil is shown only where a drop will really attach: not while the
  // composer is disabled, or the window would promise what it does not do.
  const [dragging, setDragging] = useState(false);
  const onDragging = actions.onDragging;
  useEffect(() => {
    if (onDragging === undefined || disabled) return setDragging(false);
    return onDragging(setDragging);
  }, [onDragging, disabled]);

  /** Pictures from the clipboard become attachments; the rest is pasted as usual. */
  function pasteImages(event: ClipboardEvent<HTMLTextAreaElement>) {
    const stash = actions.stashImage;
    if (stash === undefined) return;
    const images = Array.from(event.clipboardData.files).filter((f) =>
      f.type.startsWith("image/"),
    );
    if (images.length === 0) return;
    // Otherwise a file name or nothing would land in the field: there is no text in such a clipboard.
    event.preventDefault();
    for (const image of images) {
      void encoded(image)
        .then(stash)
        .then(
          (path) => attach([path]),
          // What was not saved is not attached: there is no chip, and that is visible.
          () => {},
        );
    }
  }
  const field = useRef<HTMLTextAreaElement>(null);

  // What is known about the selected model: the name for the chip and whether it
  // takes effort (§7.1). `undefined` — the model is not in the provider's list, and
  // that is not the same as "does not take effort".
  const activeModel =
    models.find(
      (m) =>
        m.id === settings.model &&
        (!settings.provider || !m.providerId || m.providerId === settings.provider),
    ) ?? models.find((m) => m.id === settings.model);

  // The built-in commands §7.3 — only those that have an action on this screen: on
  // the home screen there is no conversation yet, so there is nothing to compact or
  // start over.
  const builtins: Builtin[] = ["model"];
  if (actions.compact) builtins.push("compact");
  if (actions.newChat) builtins.push("clear");
  if (actions.openCheckpoints) builtins.push("checkpoints");
  if (actions.openMcp) builtins.push("mcp");
  if (actions.openLsp) builtins.push("lsp");

  const slash = menuFor(text, commands, builtins);
  const slashIndex = slash === null ? 0 : Math.min(slashAt, slash.length - 1);

  /** Where the caret is: `@` reads the text up to it (§7.2). */
  const [caret, setCaret] = useState(0);
  /** Where to put the caret when the field gets text that shifts it. */
  const pendingCaret = useRef<number | null>(null);
  const mentions = useMentions({
    text,
    caret,
    found,
    search: actions.searchFiles,
    onPick: (path, mention) => pickFile(path, mention),
  });

  useEffect(() => {
    if (text) localStorage.setItem(draftKey, text);
    else localStorage.removeItem(draftKey);
  }, [text, draftKey]);

  // The command list is re-read when the line has become a command, not on every
  // keypress: the project's catalog could have been added to while the window was open.
  const typing = text.startsWith("/");
  const refresh = actions.refreshCommands;
  useEffect(() => {
    if (typing) refresh?.();
  }, [typing, refresh]);

  // Auto-height up to twelve lines, then an internal scroll (§7).
  useEffect(() => {
    const node = field.current;
    if (!node) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
    const pending = pendingCaret.current;
    if (pending !== null) {
      pendingCaret.current = null;
      node.setSelectionRange(pending, pending);
    }
  }, [text]);

  /**
   * Empty the field after sending or a built-in command.
   *
   * The draft is erased here, not by the effect above: both `/clear` and the first
   * message from the home screen take the window to another screen, and the effect
   * of an unmounted composer no longer runs, so the sent text would stay in the
   * draft (§7.4).
   */
  function clear() {
    setText("");
    setAttached([]);
    localStorage.removeItem(draftKey);
  }

  /** The built-in is done by the interface; what goes to the core is a command, not text. */
  function run(builtin: Builtin) {
    if (builtin === "compact") actions.compact?.();
    if (builtin === "clear") actions.newChat?.();
    if (builtin === "checkpoints") actions.openCheckpoints?.();
    if (builtin === "mcp") actions.openMcp?.();
    if (builtin === "lsp") actions.openLsp?.();
    if (builtin === "model") setMenu("model");
    clear();
  }

  /**
   * Take the highlighted line of the menu.
   *
   * A built-in one runs — it takes no arguments. A project one is written into the
   * field: anything the person wants to pass to it may follow the name.
   */
  function pick(item: SlashItem) {
    setSlashAt(0);
    if (item.builtin !== null) {
      run(item.builtin);
      return;
    }
    setText(`/${item.name} `);
  }

  /**
   * Take a file from the `@` menu (§7.2): `@query` leaves the field, and the file
   * becomes a chip above it — the same one as attached via `＋`, and it goes with
   * the message.
   */
  function pickFile(path: string, mention: Mention) {
    const next = withoutMention(text, mention, caret);
    setText(next.text);
    setCaret(next.caret);
    pendingCaret.current = next.caret;
    attach([path]);
  }

  function send() {
    if (disabled || !text.trim()) return;
    // §7.3: a built-in command does not go to the model as text. A project command
    // does: the core expands it, and it is the core that holds the files.
    const builtin = builtinFor(text, builtins);
    if (builtin !== null) {
      run(builtin);
      return;
    }
    actions.send(text, attached);
    clear();
  }

  return (
    <div className="composer-wrap" onClick={() => setMenu(null)}>
      {dragging &&
        createPortal(
          <div className="drop-veil" aria-hidden="true">
            <div className="drop-card">{t("composer.drop")}</div>
          </div>,
          document.body,
        )}
      <div className="composer" onClick={(e) => e.stopPropagation()}>
        {queued.map((message) => (
          <QueuedChip
            key={message.id}
            text={message.text}
            onCancel={
              actions.cancelQueued && (() => actions.cancelQueued?.(message.id))
            }
          />
        ))}

        {attached.length > 0 && (
          <div className="attachments">
            {attached.map((path) => (
              <Attachment
                key={path}
                path={path}
                onRemove={() => setAttached((now) => now.filter((p) => p !== path))}
              />
            ))}
          </div>
        )}

        <div className="composer-input">
        <div className="menu-anchor composer-field">
          {slash !== null && (
            <SlashMenu items={slash} at={slashIndex} onPick={pick} onHover={setSlashAt} />
          )}
          {mentions.open !== null && (
            <MentionMenu paths={mentions.paths} at={mentions.at} onPick={mentions.pick} />
          )}

          <textarea
            ref={field}
            rows={1}
            value={text}
            disabled={disabled}
            placeholder={t("composer.placeholder")}
            onChange={(e) => {
              setText(e.target.value);
              setCaret(e.target.selectionStart);
            }}
            onSelect={(e) => setCaret(e.currentTarget.selectionStart)}
            onPaste={pasteImages}
            onKeyDown={(e) => {
              // While the command menu §7.3 is open, the arrows, `⇥` and `⏎` belong
              // to it. `⇧⇥` is not touched: it is the mode switch §13.
              if (slash !== null) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setSlashAt((now) => (now + 1) % slash.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setSlashAt((now) => (now - 1 + slash.length) % slash.length);
                  return;
                }
                if ((e.key === "Tab" || e.key === "Enter") && !e.shiftKey) {
                  e.preventDefault();
                  pick(slash[slashIndex]);
                  return;
                }
              }

              // `@` (§7.2): while the file menu is open, the arrows, ⏎ and ⇥ belong to
              // it, and Esc closes it without stopping the agent.
              if (mentions.keyDown(e)) return;

              // While a permission request hangs, the digits belong to it (§5.7), and
              // the field stays live all the same.
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
        </div>
            {busy ? (
              <button
                className="send send-stop"
                onClick={actions.stop}
                aria-label={t("composer.stop")}
              >
                <Stop size={14} weight="fill" />
              </button>
            ) : (
              <button
                className="send"
                onClick={send}
                disabled={disabled || !text.trim()}
                aria-label={t("composer.send")}
              >
                <ArrowUp size={16} weight="bold" />
              </button>
            )}
        </div>
      </div>

      {/* Everything but the action lives under the box, not in it: the box is
          the message, the row is how the message will be sent. Its own click
          must not reach the wrap, which closes the menus. */}
      <div className="composer-row" onClick={(e) => e.stopPropagation()}>
          {/* §7.1: the `＋` attach button comes first, the mode chip after it. No
              action — no button either: a screen without a project root has nothing
              to attach. */}
          {actions.pickFiles && (
            <button
              className="chip chip-icon"
              aria-label={t("composer.attach")}
              title={t("composer.attach")}
              onClick={() => {
                void actions.pickFiles?.().then(
                  (picked) =>
                    setAttached((now) => [
                      ...now,
                      ...picked.filter((path) => !now.includes(path)),
                    ]),
                  // A dialog that did not open attached nothing: there is nothing to
                  // show here, and raising a screen error for it would be too much.
                  () => {},
                );
              }}
            >
              <Plus size={14} weight="bold" />
            </button>
          )}

          <div className="menu-anchor">
            <button
              className={settings.bypass ? "chip chip-danger" : "chip"}
              onClick={() => setMenu(menu === "mode" ? null : "mode")}
            >
              {t(MODE_KEY[settings.mode])}
            </button>
            {menu === "mode" && (
              <ModeMenu
                settings={settings}
                actions={actions}
                onClose={() => setMenu(null)}
              />
            )}
          </div>

          <div className="composer-gap" />

          <ModelMenu
            models={models}
            active={activeModel}
            model={settings.model}
            provider={settings.provider}
            open={menu === "model"}
            onToggle={() => {
              setMenu(menu === "model" ? null : "model");
              // The menu's search field held the focus and is gone with it:
              // without this `Esc` would leave the keyboard nowhere.
              if (menu === "model") field.current?.focus();
            }}
            onPick={(model) => {
              actions.setModel(model.id, model.providerId);
              setMenu(null);
              field.current?.focus();
            }}
          />

          {/* §7.1 puts effort next to the model, but a model without reasoning has
              nothing to choose: the core does not send the field at all, because
              `reasoning_effort` where it is not expected is a 400. A chip that
              switches and changes nothing lies about the state (§0).

              It is hidden only by an explicit "no": a model that is not in the list
              may have had effort available, and hiding an existing level silently is
              worse than showing an extra chip. */}
          {activeModel?.supportsEffort !== false && (
            <EffortMenu
              effort={settings.effort}
              open={menu === "effort"}
              onToggle={() => setMenu(menu === "effort" ? null : "effort")}
              onPick={(effort) => {
                actions.setEffort(effort);
                setMenu(null);
              }}
            />
          )}

          {/* The ring opens into the context window (§7.1). Without a
              session there is no window to open, and the ring is only a ring:
              a button that opens nothing would lie about state (§0). */}
          {usage === null ? (
            <ContextRing usage={null} />
          ) : (
            <div className="menu-anchor">
              <button
                className="chip chip-icon"
                aria-label={t("ctx.title")}
                aria-haspopup="dialog"
                aria-expanded={menu === "context"}
                onClick={() => setMenu(menu === "context" ? null : "context")}
              >
                <ContextRing usage={usage} />
              </button>
              {menu === "context" && <ContextWindow usage={usage} />}
            </div>
          )}
      </div>
    </div>
  );
}

/** The file as base64 without the `data:` prefix — in the form the core expects. */
function encoded(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error);
    reader.onload = () => {
      const url = typeof reader.result === "string" ? reader.result : "";
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.readAsDataURL(file);
  });
}
