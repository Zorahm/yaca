import { createElement, useContext } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import type { SentFile as Delivery } from "../core/bindings/SentFile";
import { SentFile } from "./SentFile";
import { Pictures } from "./viewer/Viewer";
import { PreviewRoot } from "./viewer/context";

vi.mock("@tauri-apps/api/core", async (original) => ({
  ...await original<typeof import("@tauri-apps/api/core")>(),
  convertFileSrc: (id: string, protocol: string) => `${protocol}://localhost/${id}`,
}));

const project = "C:/Projects/example";
const privateRoot = "C:/Chats/workspaces/private";
const encodedRoot = (root: string) => Array.from(new TextEncoder().encode(root), (b) => b.toString(16).padStart(2, "0")).join("");
const file: Delivery = {
  id: "delivery_00000000000000000000000000000001",
  name: "result.png", title: "Result", caption: null,
  mime: "image/png", size: 42, sha256: "abc", renderType: "inline",
};

function AttachmentRoot() {
  return createElement("output", null, useContext(PreviewRoot));
}

function markup(deliveryRoot?: string) {
  return renderToStaticMarkup(createElement(Pictures, {
    root: project, deliveryRoot, gallery: [],
    children: [
      createElement(SentFile, { key: "delivery", file, onOpen: () => {} }),
      createElement(AttachmentRoot, { key: "attachment" }),
    ],
  }));
}

describe("delivery roots", () => {
  it("loads a sent image from the private chat while attachments keep their project root", () => {
    const html = markup(privateRoot);
    expect(html).toContain(`sent://localhost/${file.id}?root=${encodedRoot(privateRoot)}`);
    expect(html).not.toContain(`?root=${encodedRoot(project)}`);
    expect(html).toContain(`<output>${project}</output>`);
  });

  it("uses the project for code sessions and state from older versions", () => {
    expect(markup()).toContain(`sent://localhost/${file.id}?root=${encodedRoot(project)}`);
  });
});
