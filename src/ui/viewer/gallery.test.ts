import { describe, expect, it } from "vitest";

import type { AgentEvent } from "../../core/bindings/AgentEvent";
import { emptyFeed, reduceAll } from "../../core/feed";
import { galleryOf } from "./gallery";

const said = (attachments: string[]): AgentEvent => ({
  type: "userMessage",
  turnId: "t1",
  text: "вот",
  attachments,
});

const looked = (callId: string, path: string, parent: string | null = null): AgentEvent[] => [
  {
    type: "toolCallStarted",
    turnId: "t1",
    callId,
    tool: "ReadImage",
    description: null,
    target: path,
    input: { path },
    parent,
  },
  {
    type: "toolCallFinished",
    callId,
    meta: { lines: 1, items: null, truncated: false, durationMs: 3 },
  },
];

describe("галерея просмотрщика", () => {
  it("идёт по разговору: и приложенное, и открытое моделью", () => {
    const feed = reduceAll(emptyFeed, [
      said(["shot.png", "notes.md"]),
      ...looked("i1", "docs/mock.jpg"),
      said(["C:/Users/me/Desktop/after.png"]),
    ]);

    // Файл-не-картинка в листание не попадает: смотреть в нём нечего.
    expect(galleryOf(feed)).toEqual([
      "shot.png",
      "docs/mock.jpg",
      "C:/Users/me/Desktop/after.png",
    ]);
  });

  it("одна картинка — одна остановка, сколько бы раз её ни показали", () => {
    const feed = reduceAll(emptyFeed, [said(["shot.png"]), ...looked("i1", "shot.png")]);
    expect(galleryOf(feed)).toEqual(["shot.png"]);
  });

  it("берёт и то, на что смотрел подагент", () => {
    const feed = reduceAll(emptyFeed, [
      {
        type: "toolCallStarted",
        turnId: "t1",
        callId: "t1",
        tool: "Task",
        description: null,
        target: null,
        input: {},
        parent: null,
      },
      ...looked("i1", "ui/home.png", "t1"),
    ]);
    expect(galleryOf(feed)).toEqual(["ui/home.png"]);
  });

  it("упавший вызов показывать нечем", () => {
    const [started] = looked("i1", "gone.png");
    const feed = reduceAll(emptyFeed, [
      started,
      { type: "toolCallFailed", callId: "i1", message: "не найдено" },
    ]);
    expect(galleryOf(feed)).toEqual([]);
    expect(galleryOf(null)).toEqual([]);
  });
});
