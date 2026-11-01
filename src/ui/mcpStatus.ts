/** The words for a server's state, shared by the `/mcp` panel and the `＋` menu. */

import type { McpStatus } from "../core/client";
import type { Key } from "../i18n";

export const MCP_STATUS: Record<McpStatus, Key> = {
  connecting: "mcp.status_connecting",
  ready: "mcp.status_ready",
  failed: "mcp.status_failed",
  disabled: "mcp.status_disabled",
  needsApproval: "mcp.status_needs_approval",
  needsLogin: "mcp.status_needs_login",
  loggingIn: "mcp.status_logging_in",
};
