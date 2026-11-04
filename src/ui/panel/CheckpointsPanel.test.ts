import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptyFeed, reduceAll, type Feed } from "../../core/feed";
import { demoCheckpoints, demoCheckpointCommand } from "../../core/demoCheckpoints";
import { t, tp } from "../../i18n";
import { checkpointMessages, willDo } from "../checkpoint";
import { PanelFrameContext, type PanelFrame } from "./Panel";
import { CheckpointsPanel } from "./CheckpointsPanel";

const frame: PanelFrame = { current: "checkpoints", filePath: null, hasAgents: false, width: 380, initialWidth: 380, bounds: null, onSwitch: () => {}, onClose: () => {}, onResize: () => {} };
const feed: Feed = { ...emptyFeed, checkpoints: demoCheckpoints };
const markup = (patch: Partial<Feed> = {}) => renderToStaticMarkup(createElement(PanelFrameContext.Provider, { value: frame }, createElement(CheckpointsPanel, { feed: { ...feed, ...patch }, onPreview: () => {}, onRestore: () => {}, onCancel: () => {} })));

describe("checkpoint cards", () => {
  it("shows newest first and distinguishes the snapshot size from rollback changes", () => {
    const html = markup();
    expect(html.indexOf("npm test -- segments")).toBeLessThan(html.indexOf("npm run dev"));
    expect(html).toContain(t("ckpt.snapshot", { files: tp("row.files", 150) }));
    expect(html).toContain(t("ckpt.view_changes"));
    expect(html).not.toContain(t("ckpt.restore_here"));
  });

  it("offers no rollback for an identical snapshot", () => {
    const html = markup({ preview: { id: "ckpt_3", files: [] } });
    expect(html).toContain(t("ckpt.confirm_nothing"));
    expect(html).not.toContain(t("ckpt.restore_here"));
  });

  it("shows every rollback action before offering the separate restore button", () => {
    const events = demoCheckpointCommand("checkpoint-preview-test", { type: "previewCheckpoint", id: "ckpt_1" }) ?? [];
    const preview = reduceAll(emptyFeed, events).preview;
    const html = markup({ preview });
    expect(preview?.files).toHaveLength(3);
    expect(html).toContain(t("ckpt.affected", { files: tp("row.files", 3) }));
    for (const file of preview?.files ?? []) {
      expect(html).toContain(file.path);
      expect(html).toContain(willDo(file));
    }
    expect(html).toContain(t("ckpt.restore_here"));
  });

  it("associates the human message with its actual checkpoint", () => {
    const state = reduceAll(emptyFeed, [
      { type: "userMessage", turnId: "t", text: "Fix the date parser", attachments: [] },
      { type: "checkpointCreated", checkpoint: demoCheckpoints[0] },
    ]);
    expect(checkpointMessages(state).get("ckpt_1")).toBe("Fix the date parser");
    expect(markup({ blocks: state.blocks })).toContain("Fix the date parser");
    expect(checkpointMessages({ ...state, blocks: [...state.blocks].reverse() }).get("ckpt_1")).toBe("Fix the date parser");
  });

  it("requires preview in the stand and records a reversible checkpoint after restore", () => {
    const session = "checkpoint-restore-test";
    expect(demoCheckpointCommand(session, { type: "restoreCheckpoint", id: "ckpt_1" })).toEqual([]);
    demoCheckpointCommand(session, { type: "previewCheckpoint", id: "ckpt_1" });
    const restored = demoCheckpointCommand(session, { type: "restoreCheckpoint", id: "ckpt_1" }) ?? [];
    expect(restored[0]).toMatchObject({ type: "checkpointRestored", id: "ckpt_1", files: 3 });
    const created = restored.find((event) => event.type === "checkpointCreated");
    if (created?.type !== "checkpointCreated") throw new Error("missing safety checkpoint");
    expect(created.checkpoint.reason).toEqual({ type: "rollback" });
    expect(demoCheckpointCommand(session, { type: "previewCheckpoint", id: "ckpt_1" })?.[0]).toMatchObject({ type: "checkpointPreviewed", files: [] });
    expect(demoCheckpointCommand(session, { type: "previewCheckpoint", id: created.checkpoint.id })?.[0]).toMatchObject({ type: "checkpointPreviewed", files: expect.arrayContaining([expect.objectContaining({ path: "scripts/check-date.ts", kind: "created" })]) });
  });
});
