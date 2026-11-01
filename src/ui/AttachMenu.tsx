/**
 * The menu behind `＋` in the composer's bottom row (§7.1): attach a file, or
 * pick an MCP server or a plugin to name in the message.
 *
 * The lists open in place, under a "back" row, rather than as a fly-out: the
 * composer is narrow, and a fly-out has nowhere to go but over the sidebar.
 * A row exists only when it has something behind it — no files dialog, no row;
 * no servers, no row — so nothing in the menu is a control that does nothing.
 * A server that is not up is listed with its state and cannot be picked.
 */

import { CaretLeft, CaretRight, Paperclip, PlugsConnected, PuzzlePiece } from "@phosphor-icons/react";
import { useState } from "react";

import type { McpServerSummary, PluginSummary } from "../core/client";
import { t } from "../i18n";
import { pickable, type MentionKind } from "./attachMention";
import { MCP_STATUS } from "./mcpStatus";

type View = "root" | "mcp" | "plugins";

export function AttachMenu({
  onFiles,
  servers,
  plugins,
  onMention,
}: {
  /** No files dialog on this screen — no row. */
  onFiles: (() => void) | undefined;
  servers: McpServerSummary[];
  plugins: PluginSummary[];
  onMention: (kind: MentionKind, name: string) => void;
}) {
  const [view, setView] = useState<View>("root");

  if (view === "root") {
    return (
      <div className="menu menu-attach">
        {onFiles && (
          <button className="menu-item" onClick={onFiles}>
            <Paperclip size={14} />
            <span className="menu-text">{t("composer.attach")}</span>
          </button>
        )}
        {servers.length > 0 && (
          <button className="menu-item" onClick={() => setView("mcp")}>
            <PlugsConnected size={14} />
            <span className="menu-text">{t("mcp.title")}</span>
            <CaretRight size={12} />
          </button>
        )}
        {plugins.length > 0 && (
          <button className="menu-item" onClick={() => setView("plugins")}>
            <PuzzlePiece size={14} />
            <span className="menu-text">{t("plug.title")}</span>
            <CaretRight size={12} />
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="menu menu-attach">
      <button className="menu-item menu-back" onClick={() => setView("root")}>
        <CaretLeft size={12} />
        <span className="menu-text">{t("composer.add_back")}</span>
        <span className="menu-note">{t(view === "mcp" ? "mcp.title" : "plug.title")}</span>
      </button>
      <div className="menu-sep" />
      <div className="menu-scroll menu-attach-list">
        {view === "mcp"
          ? servers.map((server) => (
              <button
                className="menu-item"
                key={server.name}
                disabled={!pickable(server.status)}
                onClick={() => onMention("mcp", server.name)}
              >
                <span className="menu-text">
                  <span>{server.name}</span>
                  {!pickable(server.status) && (
                    <span className="menu-note">{t(MCP_STATUS[server.status])}</span>
                  )}
                </span>
              </button>
            ))
          : plugins.map((plugin) => (
              <button
                className="menu-item"
                key={plugin.id}
                title={plugin.description ?? undefined}
                onClick={() => onMention("plugin", plugin.name)}
              >
                <span className="menu-text">
                  <span>{plugin.name}</span>
                  {plugin.description !== null && (
                    <span className="menu-note menu-clip">{plugin.description.split("\n")[0]}</span>
                  )}
                </span>
              </button>
            ))}
      </div>
    </div>
  );
}
