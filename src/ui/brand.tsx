/**
 * Чужие знаки: логотипы провайдеров и протокола MCP.
 *
 * Свои иконки интерфейса — Phosphor, и у них один штрих на всё окно. Логотип
 * нарисован не нами, и перерисовывать его тем же штрихом значило бы показать
 * знак, которого у его владельца нет. Поэтому знаки берутся из Devicons, из
 * одноцветного набора: он красится `currentColor` и встаёт на бумагу теми же
 * чернилами, что и всё остальное (§2), без фирменных цветов посреди списка.
 *
 * Все они проходят через этот файл, чтобы второй набор иконок не расползся по
 * окну: где знак уместен, решается здесь, а не в каждом разделе.
 */

// The `…Icon` components are the marks; the bare names are wordmarks, which at
// 15 px are a hairline.
import { AnthropicIcon, McpIcon, OpenaiIcon } from "@dev.icons/react/mono";

import type { Protocol } from "../core/client";
import { BUILTIN_PROVIDER_ID, OPENAI_PROVIDER_ID } from "../core/provider";

/** Знак протокола MCP. Декоративный: рядом всегда стоит подпись. */
export function McpMark({ size }: { size: number }) {
  return <McpIcon size={size} aria-hidden="true" />;
}

/**
 * Знак предустановленного провайдера, или `null` у своего.
 *
 * По идентификатору, а не по адресу или названию: предустановленных два, и
 * узнаются они ровно так же, как их узнаёт ядро. Свой шлюз с адресом OpenAI
 * остаётся без знака — он чей-то ещё, и чужой логотип над ним соврал бы.
 */
export function ProviderMark({ id, size }: { id: string; size: number }) {
  if (id === BUILTIN_PROVIDER_ID) return <AnthropicIcon size={size} aria-hidden="true" />;
  if (id === OPENAI_PROVIDER_ID) return <OpenaiIcon size={size} aria-hidden="true" />;
  return null;
}

/**
 * Протокол так, как его пишет владелец. Имена собственные и не переводятся,
 * поэтому они здесь, а не в словарях: в обоих языках это одно и то же слово.
 */
export const PROTOCOL_NAME: Record<Protocol, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
};

/** Знак протокола: у формы запроса тот же владелец, что у знака. */
export function ProtocolMark({ protocol, size }: { protocol: Protocol; size: number }) {
  return protocol === "anthropic" ? (
    <AnthropicIcon size={size} aria-hidden="true" />
  ) : (
    <OpenaiIcon size={size} aria-hidden="true" />
  );
}
