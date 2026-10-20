/**
 * What the window shows about its connection once it is running: a strip when
 * the line is down, and the folder chooser of the server's disk.
 */

import { useEffect, useState } from "react";

import { listDir, makeDir, serveFolderRequests, type DirListing } from "../../core/pickers";
import { isRemote, savedRemote } from "../../core/link";
import { link, type LinkFault, type LinkState } from "../../core/remote";
import { t } from "../../i18n";
import { plainPath } from "../format";

import "./link.css";

export function LinkLayers() {
  if (!isRemote()) return null;
  return (
    <>
      <LinkStrip />
      <FolderLayer />
    </>
  );
}

function useLinkState(): LinkState {
  const [state, setState] = useState(link.state);
  useEffect(() => link.watch(setState), []);
  return state;
}

/** Shown while the line is down, or closed for a reason the person can act on. */
function LinkStrip() {
  const state = useLinkState();
  if (state.kind === "lost") {
    return (
      <div className="link-strip" role="status">
        {t("link.lost")}
      </div>
    );
  }
  if (state.kind !== "closed") return null;

  const text: Record<LinkFault, string> = {
    replaced: t("link.err_replaced"),
    revoked: t("link.err_revoked"),
    unauthorized: t("link.err_unauthorized"),
    unreachable: t("link.err_unreachable"),
    bad_address: t("link.err_bad_address"),
  };
  const again = () => {
    const saved = savedRemote();
    if (saved === null) return;
    // A fresh page takes the road that catches a window up with the chats.
    void link.connect(saved).then(
      () => window.location.reload(),
      () => undefined,
    );
  };
  return (
    <div className="link-strip link-strip-closed" role="alert">
      <span>{text[state.fault]}</span>
      {state.fault === "replaced" && (
        <button type="button" className="btn btn-small" onClick={again}>
          {t("link.connect")}
        </button>
      )}
    </div>
  );
}

/** A path without its trailing separator, so `/home/me/` and `/home/me` compare equal. */
const trimmed = (path: string): string => (path.length > 1 ? path.replace(/[\\/]+$/, "") : path);

type Asked = { title: string; resolve: (path: string | null) => void };

/** The chooser of a folder on the server, opened by `chooseFolder`. */
function FolderLayer() {
  const [asked, setAsked] = useState<Asked | null>(null);
  useEffect(() => serveFolderRequests(setAsked), []);
  if (asked === null) return null;
  return (
    <FolderChooser
      title={asked.title}
      onDone={(path) => {
        asked.resolve(path);
        setAsked(null);
      }}
    />
  );
}

function FolderChooser({ title, onDone }: { title: string; onDone: (path: string | null) => void }) {
  const [at, setAt] = useState<string | null>(null);
  const [hidden, setHidden] = useState(false);
  const [listing, setListing] = useState<DirListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [naming, setNaming] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setListing(null);
    setError(null);
    listDir(at, hidden).then(
      (next) => live && setListing(next),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [at, hidden]);

  const broad =
    listing !== null && (listing.parent === null || trimmed(listing.path) === trimmed(listing.home));

  const create = async () => {
    if (listing === null || naming === null) return;
    try {
      const made = await makeDir(listing.path, naming);
      setNaming(null);
      setAt(made);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="folder-scrim" role="dialog" aria-modal="true" aria-label={title}>
      <div className="folder-card">
        <h2 className="folder-title">{title}</h2>
        <div className="folder-path">{listing === null ? t("link.pick_loading") : plainPath(listing.path)}</div>

        <div className="link-row">
          <button
            type="button"
            className="btn btn-small"
            disabled={listing?.parent == null}
            onClick={() => setAt(listing?.parent ?? null)}
          >
            {t("link.pick_up")}
          </button>
          <button
            type="button"
            className="btn btn-small"
            disabled={listing === null}
            onClick={() => setAt(listing?.home ?? null)}
          >
            {t("link.pick_home")}
          </button>
          <button type="button" className="btn btn-small" disabled={listing === null} onClick={() => setNaming("")}>
            {t("link.pick_new")}
          </button>
          <label className="folder-hidden">
            <input type="checkbox" checked={hidden} onChange={(e) => setHidden(e.target.checked)} />
            {t("link.pick_hidden")}
          </label>
        </div>

        {naming !== null && (
          <form
            className="link-row"
            onSubmit={(e) => {
              e.preventDefault();
              void create();
            }}
          >
            <input
              className="prov-input"
              autoFocus
              value={naming}
              placeholder={t("link.pick_new_name")}
              onChange={(e) => setNaming(e.target.value)}
            />
            <button type="submit" className="btn btn-small btn-accent" disabled={naming.trim() === ""}>
              {t("link.pick_create")}
            </button>
          </form>
        )}

        {error !== null && <p className="connect-error">{error}</p>}

        {/* The picker opens on the home folder, and one tap on "Choose" there
            makes the whole home the project: everything in it is the agent's
            to read and edit, and it is far past what a snapshot can hold. */}
        {broad && (
          <p className="folder-warn" role="status">
            {t("link.pick_broad")}
          </p>
        )}

        <ul className="folder-list">
          {listing?.dirs.map((dir) => (
            <li key={dir.path}>
              <button type="button" className="folder-item" onClick={() => setAt(dir.path)}>
                {dir.name}
              </button>
            </li>
          ))}
          {listing !== null && listing.dirs.length === 0 && (
            <li className="folder-empty">{t("link.pick_empty")}</li>
          )}
        </ul>

        <div className="link-row folder-actions">
          <button type="button" className="btn" onClick={() => onDone(null)}>
            {t("link.pick_cancel")}
          </button>
          <button
            type="button"
            className="btn btn-accent"
            disabled={listing === null}
            onClick={() => listing !== null && onDone(listing.path)}
          >
            {t("link.pick_choose")}
          </button>
        </div>
      </div>
    </div>
  );
}
