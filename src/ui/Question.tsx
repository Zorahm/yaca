/**
 * Question with options, `AskUser` (a block like §5.7).
 *
 * While it waits, it is the same red box as a permission request: both wait for
 * a decision. An answer or a refusal turns it into one line of summary — what
 * was picked — and the box goes. The line stays in the conversation: the
 * question and its answer are part of what was being discussed.
 *
 * Digits `1`–`6` pick an option, `⏎` answers, `Esc` skips. While the human types
 * their own answer in the field, the digits belong to the field: it is text, not
 * a pick. Composing with an IME, `⏎` does not answer — it confirms a word, not
 * the question.
 */

import { useEffect, useState } from "react";

import type { QuestionReply, QuestionRequest } from "../core/client";
import type { QuestionStatus } from "../core/feed";
import { t } from "../i18n";

export function Question({
  request,
  status,
  reply,
  onAnswer,
}: {
  request: QuestionRequest;
  status: QuestionStatus;
  reply: QuestionReply | null;
  onAnswer: (reply: QuestionReply) => void;
}) {
  if (status !== "open") {
    return <Settled request={request} status={status} reply={reply} />;
  }
  return <Open request={request} onAnswer={onAnswer} />;
}

function Open({
  request,
  onAnswer,
}: {
  request: QuestionRequest;
  onAnswer: (reply: QuestionReply) => void;
}) {
  const [picked, setPicked] = useState<number[]>([]);
  const [text, setText] = useState("");

  const own = text.trim();
  const ready = picked.length > 0 || own !== "";

  // Radio: text the human wrote replaces the pick, so the pick is not sent
  // along. Checkboxes: the ticked options and the text go together.
  const overridden = !request.multiSelect && own !== "";

  const submit = () => {
    if (!ready) return;
    onAnswer({
      type: "answered",
      selected: overridden ? [] : [...picked].sort((a, b) => a - b),
      other: own === "" ? null : own,
    });
  };

  // Both kinds only mark the pick; the answer goes with Send or Enter, so a
  // pick can still be joined by text.
  const pick = (index: number) => {
    if (!request.multiSelect) {
      setPicked([index]);
      return;
    }
    setPicked((current) =>
      current.includes(index) ? current.filter((i) => i !== index) : [...current, index],
    );
  };

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      // The Russian keyboard's composition uses Enter to take a word.
      if (event.isComposing) return;

      if (event.key === "Escape") {
        event.preventDefault();
        onAnswer({ type: "dismissed" });
        return;
      }

      if (event.key === "Enter" && !event.shiftKey) {
        if (ready) {
          event.preventDefault();
          submit();
        }
        return;
      }

      const typing =
        event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
      if (typing) return;

      const at = Number(event.key) - 1;
      if (Number.isInteger(at) && at >= 0 && at < request.options.length) {
        event.preventDefault();
        pick(at);
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="ask question">
      <div className="ask-head">
        <div className="ask-question">{request.question}</div>
      </div>

      {request.multiSelect && <div className="ask-target">{t("question.multi_hint")}</div>}

      <div
        className="question-options"
        role={request.multiSelect ? "group" : "radiogroup"}
        aria-label={request.question}
      >
        {request.options.map((option, index) => {
          const on = picked.includes(index);
          return (
            <button
              key={index}
              className={`question-option${overridden && on ? " is-overridden" : ""}`}
              role={request.multiSelect ? "checkbox" : "radio"}
              aria-checked={on}
              onClick={() => pick(index)}
            >
              <span
                className={`question-mark ${request.multiSelect ? "is-check" : "is-radio"}`}
                aria-hidden="true"
              />
              <span className="keycap">{index + 1}</span>
              <span className="question-option-text">
                <span className="question-option-label">{option.label}</span>
                {option.description && <span className="ask-note">{option.description}</span>}
              </span>
            </button>
          );
        })}
      </div>

      {request.allowOther && (
        <div className="ask-deny">
          <input
            value={text}
            placeholder={t("question.other_placeholder")}
            aria-label={t("question.other_placeholder")}
            onChange={(e) => setText(e.target.value)}
          />
        </div>
      )}

      <div className="ask-actions">
        <button className="btn" onClick={() => onAnswer({ type: "dismissed" })}>
          {t("question.skip")}
          <span className="keycap">Esc</span>
        </button>
        <div className="ask-group ask-group-allow">
          <button className="btn btn-accent" disabled={!ready} onClick={submit}>
            {t("question.send")}
            <span className="keycap">⏎</span>
          </button>
        </div>
      </div>
    </div>
  );
}

/** The one-line record of what was answered, left in the conversation. */
function Settled({
  request,
  status,
  reply,
}: {
  request: QuestionRequest;
  status: QuestionStatus;
  reply: QuestionReply | null;
}) {
  return (
    <div className="question-settled">
      <div className="question-settled-q">{request.question}</div>
      <div className="question-settled-a">{summary(request, status, reply)}</div>
    </div>
  );
}

function summary(
  request: QuestionRequest,
  status: QuestionStatus,
  reply: QuestionReply | null,
): string {
  if (status === "dismissed") return t("question.dismissed");
  if (status === "unanswered" || reply === null || reply.type !== "answered") {
    return t("question.unanswered");
  }
  const parts: string[] = [];
  const labels = reply.selected
    .map((i) => request.options[i]?.label)
    .filter((label): label is string => label !== undefined);
  if (labels.length > 0) parts.push(labels.join(", "));
  if (reply.other) parts.push(t("question.own", { text: reply.other }));
  return t("question.answered", { answer: parts.join(" · ") });
}
