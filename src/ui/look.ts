/**
 * Что лежит по пути вложения: картинка, файл, папка или уже ничего.
 *
 * Спрашивается у ядра, а не угадывается по имени: `.png`, в котором лежит
 * HTML, — файл, а не картинка, и папку от файла без расширения по имени не
 * отличить вовсе. Вебвью диска не читает, поэтому вместе с ответом приезжает
 * и миниатюра — готовым `data:`-адресом.
 */

import { useContext, useEffect, useState } from "react";

import { attachmentLook, type AttachmentLook } from "../core/client";
import { PreviewRoot } from "./viewer/context";

/** Один вопрос на путь: лента перерисовывается чаще, чем меняются файлы. */
const asked = new Map<string, Promise<AttachmentLook>>();

/** Ядро не ответило — значит, показать можно только имя. */
const UNKNOWN: AttachmentLook = { kind: "file", preview: null };

function lookOf(root: string, path: string): Promise<AttachmentLook> {
  const key = `${root}\n${path}`;
  let answer = asked.get(key);
  if (answer === undefined) {
    answer = attachmentLook(root, path).catch(() => UNKNOWN);
    asked.set(key, answer);
  }
  return answer;
}

/** `undefined` — ответ ещё едет. */
export function useLook(path: string): AttachmentLook | undefined {
  const root = useContext(PreviewRoot);
  const [look, setLook] = useState<AttachmentLook | undefined>(undefined);

  useEffect(() => {
    if (root === null) return setLook(UNKNOWN);
    let alive = true;
    setLook(undefined);
    void lookOf(root, path).then((answer) => {
      if (alive) setLook(answer);
    });
    return () => {
      alive = false;
    };
  }, [root, path]);

  return look;
}
