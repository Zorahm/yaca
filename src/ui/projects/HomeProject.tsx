import { CaretDown, Check, FolderSimple, Plus } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { useState } from "react";

import { hasFileDialogs, type ProjectEntry } from "../../core/client";
import { t } from "../../i18n";
import { useBackClose } from "../back";
import { plainPath } from "../format";

export function HomeProject({ project, entry, projects, onSelect, onCreate }: {
  project: string | null;
  entry: ProjectEntry | null;
  projects: ProjectEntry[];
  onSelect: (path: string) => void;
  onCreate: () => void;
}) {
  const [open, setOpen] = useState(false);
  useBackClose(open, () => setOpen(false));
  const name = entry?.name ?? (project === null ? t("home.not_chosen") : plainPath(project));
  const folders = entry?.folders ?? (project === null ? [] : [project]);

  return <section className="home-pick" aria-label={t("home.pick_title")}>
    <div className="home-pick-text">
      <div className="home-pick-title">
        <span>{t("home.pick_title")}:</span>
        <span className="home-project-name" title={name}>{name}</span>
      </div>
      {folders.length > 0 && <ul className="home-project-folders" aria-label={t("project.folders")}>
        {folders.map((folder) => <li key={folder}>
          <FolderSimple size={14} aria-hidden="true" />
          <span className="home-pick-path" title={plainPath(folder)}>{plainPath(folder)}</span>
        </li>)}
      </ul>}
    </div>
    {projects.length === 0 ? (
      <button type="button" className="btn btn-small" onClick={onCreate} disabled={!hasFileDialogs}>
        {t("project.create")}
      </button>
    ) : (
      <DropdownMenu.Root open={open} onOpenChange={setOpen}>
        <DropdownMenu.Trigger asChild>
          <button type="button" className="btn btn-small">
            {t("home.pick_project")}<CaretDown size={12} aria-hidden="true" />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="pop home-project-menu" align="end" sideOffset={6} collisionPadding={16}>
            <div className="home-project-options">
              <DropdownMenu.RadioGroup value={project ?? ""} onValueChange={onSelect}>
                {projects.map((item) => <DropdownMenu.RadioItem
                  key={item.path}
                  value={item.path}
                  textValue={item.name}
                  className="pop-item home-project-option"
                >
                  <FolderSimple size={16} aria-hidden="true" />
                  <span className="home-project-option-text">
                    <span className="home-project-name" title={item.name}>{item.name}</span>
                    <span className="home-pick-path" title={plainPath(item.path)}>{plainPath(item.path)}</span>
                  </span>
                  <DropdownMenu.ItemIndicator><Check size={14} weight="bold" /></DropdownMenu.ItemIndicator>
                </DropdownMenu.RadioItem>)}
              </DropdownMenu.RadioGroup>
            </div>
            <DropdownMenu.Separator className="pop-sep" />
            <DropdownMenu.Item className="pop-item" onSelect={onCreate} disabled={!hasFileDialogs}>
              <Plus size={16} aria-hidden="true" />{t("project.create")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    )}
  </section>;
}
