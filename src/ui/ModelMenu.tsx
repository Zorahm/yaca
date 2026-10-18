/**
 * The model chip of the bottom row and its list (§7.1).
 *
 * The list holds every provider's models, each bringing its own provider —
 * past a hundred once the presets have answered. So it is not a list to
 * scroll: there is a search field with the focus in it, a group per provider,
 * the recently picked models first, and the keyboard takes it from there
 * (`./modelSearch` has the rules).
 *
 * A model a local server has not loaded is marked: picking it costs a long
 * load or an error, and a list that looks the same for all of them hides that
 * (`./residency`).
 */

import { Check } from "@phosphor-icons/react";
import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";

import {
  modelResidency,
  providers as registry,
  ROUTER_PROVIDER_ID,
  routingStatus,
  type ModelInfo,
} from "../core/client";
import { t } from "../i18n";
import { ProviderMark } from "./brand";
import {
  groupsOf,
  modelKey,
  parseRecent,
  recentOf,
  withRecent,
  RECENT_KEY,
  type ModelGroup,
} from "./modelSearch";
import { createResidency, residencyKey, type Residency } from "./residency";

/** One per window: which providers stay quiet is worth remembering all run. */
const residencyOf = createResidency(modelResidency);

export function ModelMenu({
  models,
  active,
  model,
  provider,
  open,
  onToggle,
  onPick,
}: {
  models: ModelInfo[];
  /** What is known of the chosen model; `undefined` when no provider lists it. */
  active: ModelInfo | undefined;
  model: string;
  provider?: string;
  open: boolean;
  onToggle: () => void;
  onPick: (model: ModelInfo) => void;
}) {
  const [residency, setResidency] = useState<Residency>(new Map());
  /** Provider names by id: a model knows only the id of who serves it. */
  const [names, setNames] = useState<Map<string, string>>(new Map());
  const [recent, setRecent] = useState<string[]>([]);
  /** Why the router is not in the list, when `routing.json` is there and unusable. */
  const [routerError, setRouterError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  /** The row `⏎` takes, as an index into `rows`. */
  const [at, setAt] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const listId = useId();

  // A string, not the array: the list arrives as a fresh array on every render
  // of the window, and each one would ask the servers again.
  const providers = [...new Set(models.map((m) => m.providerId))].join("\n");

  useEffect(() => {
    if (!open) return;
    let live = true;
    // Last open's marks may be stale already; none is better than wrong ones.
    setResidency(new Map());
    void residencyOf(providers.split("\n")).then((marks) => {
      if (live) setResidency(marks);
    });
    // Asked on every open, like the marks: the panel may have renamed one
    // since. Until it answers — and if it refuses — a group is headed by its
    // id, which is what every row used to say anyway.
    registry.list().then(
      (all) => {
        if (live) setNames(new Map(all.list.map((p) => [p.id, p.displayName])));
      },
      () => undefined,
    );
    // A file that is there and does not work is said here: a router that just
    // is not in the list reads as a feature that was never set up.
    routingStatus().then(
      (status) => {
        if (live) setRouterError(status.state === "broken" ? status.error : null);
      },
      () => undefined,
    );
    setRecent(parseRecent(localStorage.getItem(RECENT_KEY)));
    setQuery("");
    setAt(0);
    return () => {
      live = false;
    };
  }, [open, providers]);

  const isActive = (m: ModelInfo) =>
    m.id === model && (!provider || !m.providerId || m.providerId === provider);

  const sections = useMemo(() => {
    const nameOf = (id: string) =>
      id === ROUTER_PROVIDER_ID ? t("routing.title") : (names.get(id) ?? id);
    const found: ModelGroup[] = groupsOf(models, nameOf, query.trim());
    // The recent models are a shortcut past the search, not a result of it:
    // under a query they would only show the same model twice.
    const picked = query.trim() === "" ? recentOf(models, recent) : [];
    const all = picked.length > 0 ? [{ provider: "", models: picked }, ...found] : found;
    return all.map((group) => ({
      ...group,
      title: group.provider === "" ? t("composer.model_recent") : nameOf(group.provider),
    }));
  }, [models, names, query, recent]);

  const rows = sections.flatMap((section) => section.models);

  // The highlighted row stays in sight as the arrows move it past the edge.
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [at, query, open]);

  function pick(m: ModelInfo) {
    localStorage.setItem(RECENT_KEY, JSON.stringify(withRecent(recent, modelKey(m))));
    onPick(m);
  }

  function onKey(event: KeyboardEvent) {
    switch (event.key) {
      case "ArrowDown":
        setAt((now) => Math.min(now + 1, rows.length - 1));
        break;
      case "ArrowUp":
        setAt((now) => Math.max(now - 1, 0));
        break;
      case "Enter":
        if (rows[at]) pick(rows[at]);
        break;
      case "Escape":
        onToggle();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  let index = -1;

  return (
    <div className="menu-anchor">
      <button className="chip chip-strong" onClick={onToggle}>
        {active?.displayName ?? (model || t("composer.no_model"))}
      </button>
      {open && (
        // The composer closes its menus on a click anywhere in its band; a
        // click on the search field is not one of those.
        <div className="menu menu-models" onClick={(event) => event.stopPropagation()}>
          <input
            className="menu-search"
            role="combobox"
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={rows[at] ? `${listId}-${at}` : undefined}
            aria-label={t("composer.model_search")}
            placeholder={t("composer.model_search")}
            autoFocus
            spellCheck={false}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setAt(0);
            }}
            onKeyDown={onKey}
          />
          <div className="menu-scroll" id={listId} role="listbox" ref={list}>
            {sections.map((section) => (
              <div role="group" aria-label={section.title} key={section.provider}>
                <div className="menu-group">
                  <ProviderMark id={section.provider} size={12} />
                  {section.title}
                </div>
                {section.models.map((m) => {
                  index += 1;
                  const row = index;
                  const unloaded =
                    residency.get(residencyKey(m.providerId, m.id)) === false;
                  return (
                    <button
                      className="menu-item"
                      role="option"
                      id={`${listId}-${row}`}
                      aria-selected={row === at}
                      // The focus stays in the search field; the rows are
                      // reached with the arrows.
                      tabIndex={-1}
                      key={modelKey(m)}
                      title={m.id}
                      onClick={() => pick(m)}
                    >
                      <span className="menu-model">{m.displayName}</span>
                      <span className="menu-note">
                        {/* Among the recent ones the heading does not say
                            who serves the model, so the row does. */}
                        {section.provider === "" &&
                          (m.providerId === ROUTER_PROVIDER_ID
                            ? t("routing.title")
                            : (names.get(m.providerId) ?? m.providerId))}
                        {unloaded && section.provider === "" && " · "}
                        {unloaded && t("composer.model_unloaded")}
                      </span>
                      {isActive(m) && (
                        <span className="menu-mark">
                          <Check size={13} weight="bold" />
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            ))}
            {rows.length === 0 && (
              <div className="menu-empty">{t("composer.model_none")}</div>
            )}
            {routerError !== null && (
              <div className="menu-empty">{t("routing.menu_broken", { error: routerError })}</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
