import { DotsThree, Copy } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { t, type Key } from "../../i18n";
import type { PluginContents } from "../../core/bindings/PluginContents";
import { useCopy } from "../useCopy";

export function PluginChip({ children, tone }: { children: ReactNode; tone?: "warn" | "error" | "accent" }) {
  return <span className="plugin-chip" data-tone={tone}>{children}</span>;
}

export function PluginMenu({ label, disabled, items }: {
  label: string; disabled?: boolean;
  items: { label: string; onSelect?: () => void; disabled?: boolean; hint?: string }[];
}) {
  return <DropdownMenu.Root>
    <DropdownMenu.Trigger asChild>
      <button type="button" className="btn btn-row plugin-menu-trigger" aria-label={label} title={label} disabled={disabled}>
        <DotsThree size={20} weight="bold" />
      </button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Portal><DropdownMenu.Content className="pop plugin-menu" align="end" sideOffset={5}>
      {items.map((item) => <DropdownMenu.Item key={item.label} className="pop-item" disabled={item.disabled} onSelect={item.onSelect}>
        <span>{item.label}{item.hint && <small>{item.hint}</small>}</span>
      </DropdownMenu.Item>)}
    </DropdownMenu.Content></DropdownMenu.Portal>
  </DropdownMenu.Root>;
}

export function PluginDescription({ text }: { text: string | null }) {
  const [expanded, setExpanded] = useState(false);
  const [long, setLong] = useState(false);
  const ref = useRef<HTMLParagraphElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || expanded) return;
    const measure = () => setLong(el.scrollHeight > el.clientHeight + 1);
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  }, [text, expanded]);
  if (!text) return null;
  return <div className="plugin-description">
    <p ref={ref} data-expanded={expanded}>{text}</p>
    {(long || expanded) && <button type="button" className="link" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
      {t(expanded ? "plug.less" : "plug.more")}
    </button>}
  </div>;
}

const COUNTS: { field: keyof PluginContents; label: Key }[] = [
  { field: "skills", label: "plug.skills" }, { field: "commands", label: "plug.commands" },
  { field: "mcpServers", label: "plug.mcp" }, { field: "hooks", label: "plug.hooks" },
];

export function PluginCounts({ contents }: { contents: PluginContents }) {
  const counts = COUNTS.filter(({ field }) => contents[field].length > 0);
  return <span className="plugin-counts">{counts.map(({ field, label }) => `${t(label)}: ${contents[field].length}`).join(" · ")}</span>;
}

export function PluginContentsView({ contents }: { contents: PluginContents }) {
  return <div className="plugin-contents">
    {contents.skills.length > 0 && <section><h3>{t("plug.skills")}</h3><div className="plugin-skill-list">
      {contents.skills.map((s) => <code key={s}>{s}</code>)}
    </div></section>}
    {contents.commands.length > 0 && <section><h3>{t("plug.commands")}</h3>
      {contents.commands.map((c) => <p key={c.name}><code>/{c.name}</code> {c.description}</p>)}
    </section>}
    {contents.hooks.length > 0 && <section><h3>{t("plug.hooks")}</h3>
      {contents.hooks.map((h, i) => <div className="plugin-launch" key={i}><span>{h.event} <small>{h.runner}</small></span><pre>{h.command}</pre></div>)}
    </section>}
    {contents.mcpServers.length > 0 && <section><h3>{t("plug.mcp")}</h3>
      {contents.mcpServers.map((s) => <div className="plugin-launch" key={s.name}><span>{s.name}</span><pre>{s.command}</pre></div>)}
    </section>}
    {COUNTS.every(({ field }) => contents[field].length === 0) && <p className="plugin-muted">{t("plug.no_components")}</p>}
  </div>;
}

export function PluginOriginal({ text }: { text: string }) {
  const { copied, copy } = useCopy();
  return <details className="plugin-original"><summary>{t("plug.details")}</summary>
    <pre>{text}</pre><button type="button" className="btn btn-small" onClick={() => copy(text)}><Copy size={13} />{t(copied ? "plug.copied" : "plug.copy")}</button>
  </details>;
}

export function PluginConfirm({ text, label, busy, onConfirm, onCancel }: { text: string; label?: string; busy?: boolean; onConfirm: () => void; onCancel: () => void }) {
  return <div className="plugin-warning plugin-confirm" role="alert">
    <p>{text}</p><div className="plugin-actions">
      <button type="button" className="btn btn-small btn-danger" disabled={busy} onClick={onConfirm}>{label ?? t("plug.remove")}</button>
      <button type="button" className="btn btn-small" disabled={busy} onClick={onCancel}>{t("plug.cancel")}</button>
    </div>
  </div>;
}
