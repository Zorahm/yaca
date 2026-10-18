/**
 * Taking the chats of the shell back after the window was reloaded.
 *
 * The shell keeps sessions running through a reload, so what the window has to
 * get right is only the join: the snapshot it is handed, then the frames of the
 * bridge that follows it, each event once. These are the pure parts of that.
 */

import { describe, expect, it } from "vitest";

import type { AgentEvent } from "../core/bindings/AgentEvent";
import type { AttachedChat } from "../core/bindings/AttachedChat";
import type { EventFrame } from "../core/bindings/EventFrame";
import { apply, chatOf, install, type Chat } from "./chats";

const started: AgentEvent = { type: "turnStarted", turnId: "t1", at: 1 };

const said = (text: string): AgentEvent => ({
  type: "textDelta",
  turnId: "t1",
  blockId: "b1",
  text,
});

function attached(id: string, epoch: number, ...events: AgentEvent[]): AttachedChat {
  return { id, project: "C:\\Kod", epoch, events };
}

function frame(session: string, epoch: number, ...events: AgentEvent[]): EventFrame {
  return { session, epoch, events };
}

/** What the agent has said in a chat's feed. */
function spoken(chat: Chat): string {
  return chat.feed.blocks.flatMap((block) => (block.kind === "agent" ? [block.text] : [])).join("");
}

describe("a chat taken back from the shell", () => {
  it("is the fold of the events it was handed, with the project it was opened in", () => {
    const chat = chatOf(attached("ses_a", 1, started, said("halfway ")));

    expect(chat.id).toBe("ses_a");
    expect(chat.project).toBe("C:\\Kod");
    expect(chat.epoch).toBe(1);
    expect(spoken(chat)).toBe("halfway ");
  });

  it("keeps a turn that is still running running", () => {
    const chat = chatOf(attached("ses_a", 1, started, said("x")));

    // A replay of a dead run would have closed it; this one is alive.
    expect(chat.feed.turn).not.toBeNull();
    expect(chat.feed.blocks.some((block) => block.kind === "stopped")).toBe(false);
  });

  it("takes the live state from the events, with no copy beside them", () => {
    const state = {
      projectRoot: "C:\\Kod",
      provider: "anthropic",
      model: "m",
      mode: "manual",
      effort: "medium",
      busy: true,
      bypass: false,
    } as const;
    const chat = chatOf(
      attached(
        "ses_a",
        1,
        { type: "stateChanged", state },
        { type: "queueUpdated", queued: [{ id: "q1", text: "next", attachments: [] }] },
        {
          type: "checkpointsUpdated",
          checkpoints: [
            { id: "ckpt_1", at: 1, reason: { type: "message" }, tracked: 3 },
          ],
        },
      ),
    );

    expect(chat.feed.state?.busy).toBe(true);
    expect(chat.feed.queued).toHaveLength(1);
    expect(chat.feed.checkpoints).toHaveLength(1);
  });
});

describe("frames after the snapshot", () => {
  it("are folded onto the snapshot of their own bridge", () => {
    const chats = install([], [attached("ses_a", 2, started, said("one "))]);

    const after = apply(chats, frame("ses_a", 2, said("two")));

    expect(spoken(after[0])).toBe("one two");
  });

  it("are dropped when they come from a bridge that has been replaced", () => {
    const chats = install([], [attached("ses_a", 2, started, said("one "))]);

    // The old bridge's last frame, still on its way when the new snapshot
    // landed: its events are already in the snapshot.
    const after = apply(chats, frame("ses_a", 1, said("one ")));

    expect(spoken(after[0])).toBe("one ");
  });

  it("are dropped for a chat the window does not have", () => {
    const chats = install([], [attached("ses_a", 1, started)]);
    expect(apply(chats, frame("ses_gone", 1, said("x")))).toEqual(chats);
  });

  it("reach a chat the window opened itself, which is at epoch 0", () => {
    const opened = chatOf(attached("ses_a", 0, started));
    expect(spoken(apply([opened], frame("ses_a", 0, said("hi")))[0])).toBe("hi");
  });
});

describe("installing the shell's chats", () => {
  it("keeps the order the chats were given in", () => {
    const chats = install([], [attached("ses_a", 1), attached("ses_b", 1), attached("ses_c", 1)]);
    expect(chats.map((chat) => chat.id)).toEqual(["ses_a", "ses_b", "ses_c"]);
  });

  it("replaces a chat only with a snapshot of a later bridge, and keeps its title", () => {
    const first = install([], [attached("ses_a", 1, started, said("old "))]);
    first[0].title = "Fix the parser";

    const later = install(first, [attached("ses_a", 2, started, said("new "))]);
    expect(spoken(later[0])).toBe("new ");
    expect(later[0].title).toBe("Fix the parser");

    // The same bridge's snapshot arriving again, or an older one, is not news.
    const same = install(later, [attached("ses_a", 2, started, said("stale "))]);
    expect(spoken(same[0])).toBe("new ");
    const older = install(later, [attached("ses_a", 1, started, said("older "))]);
    expect(spoken(older[0])).toBe("new ");
  });

  it("adds to the chats the window already has", () => {
    const mine = install([], [attached("ses_mine", 0)]);
    const all = install(mine, [attached("ses_shell", 1)]);
    expect(all.map((chat) => chat.id)).toEqual(["ses_mine", "ses_shell"]);
  });
});
