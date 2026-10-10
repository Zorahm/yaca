/**
 * Что нужно знать, чтобы показать картинку из любого места окна.
 *
 * Отдельным файлом, потому что нужно это двоим, и друг другу тоже: плитка
 * открывает просмотрщик, просмотрщик спрашивает тот же корень, что и плитка.
 */

import { createContext, useContext } from "react";

/** Корень проекта, от которого записаны пути в ленте и в композере. */
export const PreviewRoot = createContext<string | null>(null);

/** Открыть картинку в просмотрщике. Вне него — ничего не делает. */
export const OpenImage = createContext<(path: string) => void>(() => {});

export function useOpenImage(): (path: string) => void {
  return useContext(OpenImage);
}
