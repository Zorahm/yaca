import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { emptyFeed, type Feed } from "../../core/feed";
import { t } from "../../i18n";
import { PanelFrameContext, type PanelFrame } from "./Panel";
import { ProcessesPanel } from "./ProcessesPanel";
import { demoProcessCommand } from "../../core/demoProcesses";

const frame: PanelFrame = { current: "processes", filePath: null, hasAgents: false, width: 380, initialWidth: 380, bounds: null, onSwitch: () => {}, onClose: () => {}, onResize: () => {} };
const feed: Feed = { ...emptyFeed, processes: [
  { id: 1, command: "npm run dev", cwd: ".", state: { type: "running" }, startedAt: Date.now() - 60000, finishedAt: null, lines: 1, dropped: 0 },
  { id: 2, command: "npm test", cwd: ".", state: { type: "exited", code: 0 }, startedAt: 1000, finishedAt: 5000, lines: 1, dropped: 0 },
], processOutput: { id: 1, text: "preview ready", dropped: 0 } };
const markup = (watched: number | null) => renderToStaticMarkup(createElement(PanelFrameContext.Provider, { value: frame }, createElement(ProcessesPanel, { feed, watched, onWatch: () => {}, onStop: () => {} })));

describe("background command cards", () => {
  it("starts collapsed and shows command, completion state, and stop control", () => {
    const html = markup(null);
    expect(html).toContain("npm run dev");
    expect(html).toContain("npm test");
    expect(html).toContain(t("proc.state_completed"));
    expect(html).toContain(t("proc.stop"));
    expect(html).not.toContain("preview ready");
    expect(html.match(/aria-expanded="false"/g)).toHaveLength(2);
  });

  it("puts output inside its own expanded card", () => {
    const html = markup(1);
    expect(html).toContain('id="process-output-1"');
    expect(html).toContain("preview ready");
    expect(html).not.toContain('id="process-output-2"');
  });

  it("does not show the previous command's buffer while switching subscriptions", () => {
    const html = markup(2);
    expect(html).toContain(t("proc.loading"));
    expect(html).not.toContain("preview ready");
  });

  it("lets the stand switch output and stop a command through the public commands", () => {
    const selected = demoProcessCommand("process-test", { type: "watchProcess", id: 2 });
    expect(selected?.[0]).toMatchObject({ type: "processOutputReset", id: 2 });
    const stopped = demoProcessCommand("process-test", { type: "stopProcess", id: 1 });
    expect(stopped?.[0]).toMatchObject({ type: "processesUpdated", processes: expect.arrayContaining([expect.objectContaining({ id: 1, state: { type: "stopped" }, finishedAt: expect.any(Number) })]) });
  });
});
