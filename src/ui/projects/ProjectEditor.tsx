import { FolderSimple, Plus, X } from "@phosphor-icons/react";
import { Dialog } from "radix-ui";
import { useState } from "react";

import { chooseFolders, hasFileDialogs, saveProjectEntry, type ProjectEntry } from "../../core/client";
import { t } from "../../i18n";
import { baseName, plainPath } from "../format";
import { useBackClose } from "../back";
import "./projects.css";

export function ProjectEditor({ entry, onSaved, onClose }: {
  entry: ProjectEntry | null;
  onSaved: (entries: ProjectEntry[], path: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(entry?.name ?? "");
  const [folders, setFolders] = useState(entry?.folders ?? []);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState(false);
  const close = () => { if (!busy && !picking) onClose(); };
  useBackClose(true, close);

  const addFolders = async () => {
    setPicking(true);
    try {
      const picked = await chooseFolders(t("project.folders"));
      setFolders((now) => [...new Set([...now, ...picked])]);
      if (!name.trim() && picked.length > 0) setName(baseName(picked[0]));
      setError(null);
    } catch (e) { setError(String(e)); }
    finally { setPicking(false); }
  };

  const save = async () => {
    if (busy || picking || !name.trim() || folders.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const entries = await saveProjectEntry(entry?.path ?? null, name, folders);
      const saved = entry === null ? entries[0] : entries.find((item) => item.path === entry.path);
      if (saved) onSaved(entries, saved.path);
      onClose();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };

  return <Dialog.Root open onOpenChange={(open) => !open && close()}>
    <Dialog.Portal>
      <Dialog.Overlay className="project-scrim" />
      <Dialog.Content className="project-editor" onEscapeKeyDown={(e) => { e.stopPropagation(); if (busy || picking) e.preventDefault(); }}>
        <header className="project-editor-head">
          <Dialog.Title>{t(entry === null ? "project.create" : "project.edit")}</Dialog.Title>
          <button type="button" className="top-icon" disabled={busy || picking} onClick={close} aria-label={t("common.close")}><X size={16} /></button>
        </header>
        <Dialog.Description className="project-hint">{t("project.description")}</Dialog.Description>
        <form onSubmit={(e) => { e.preventDefault(); void save(); }}>
          <fieldset disabled={busy || picking}>
            <label className="project-name-field">
              <span>{t("project.name")}</span>
              <input className="prov-input" value={name} onChange={(e) => setName(e.target.value)} required autoFocus />
            </label>
            <div className="project-folder-heading">
              <span>{t("project.folders")}</span>
              <button type="button" className="btn btn-small" onClick={() => void addFolders()} disabled={!hasFileDialogs || busy || picking}><Plus size={13} />{t("project.add_folder")}</button>
            </div>
            <ProjectFolders folders={folders} onRemove={(index) => setFolders((now) => now.filter((_, at) => at !== index))} lockPrimary={entry !== null} />
            {folders.length === 0 && <p className="project-hint">{t("project.no_folders")}</p>}
            <p className="project-hint">{t("project.primary_hint")}</p>
            {entry !== null && <p className="project-hint">{t("project.future_chats")}</p>}
            {folders.length > 1 && <p className="project-hint">{t("project.rollback_hint")}</p>}
          </fieldset>
          {error !== null && <div className="project-error" role="alert">{error}</div>}
          <footer className="project-editor-actions">
            <button type="button" className="btn" disabled={busy || picking} onClick={close}>{t("project.cancel")}</button>
            <button type="submit" className="btn btn-accent" disabled={busy || picking || !name.trim() || folders.length === 0}>{t(busy ? "project.saving" : entry === null ? "project.create" : "project.save")}</button>
          </footer>
        </form>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}

export function ProjectFolders({ folders, onRemove, lockPrimary = true }: {
  folders: string[];
  onRemove?: (index: number) => void;
  lockPrimary?: boolean;
}) {
  return <ul className="project-folders">
    {folders.map((path, index) => <li key={path}>
      <FolderSimple size={16} aria-hidden="true" />
      <span className="project-folder-path" title={plainPath(path)}>{plainPath(path)}</span>
      {index === 0 && <span className="project-primary">{t("project.primary")}</span>}
      {onRemove && (!lockPrimary || index > 0) && <button type="button" className="top-icon" onClick={() => onRemove(index)} aria-label={t("project.remove_folder", { path: plainPath(path) })}><X size={14} /></button>}
    </li>)}
  </ul>;
}
