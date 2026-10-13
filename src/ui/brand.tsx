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
// 15 px are a hairline. DeepSeek and z.ai ship one drawing each, under the
// bare name, and it is the mark.
import {
  AnthropicIcon,
  Deepseek,
  GoogleAiStudioIcon,
  McpIcon,
  OpenaiIcon,
  OpenrouterIcon,
  Zai,
} from "@dev.icons/react/mono";

import type { Protocol } from "../core/client";
import { BUILTIN_PROVIDER_ID, OPENAI_PROVIDER_ID } from "../core/provider";

/** By preset id (`core/provider` `PRESETS`). A map and not an object: a
    provider of one's own may be called `constructor`. */
const PROVIDER_MARKS = new Map<string, typeof OpenaiIcon>([
  [OPENAI_PROVIDER_ID, OpenaiIcon],
  [BUILTIN_PROVIDER_ID, AnthropicIcon],
  ["google", GoogleAiStudioIcon],
  ["openrouter", OpenrouterIcon],
  ["deepseek", Deepseek],
  ["zai", Zai],
]);

/** Знак протокола MCP. Декоративный: рядом всегда стоит подпись. */
export function McpMark({ size }: { size: number }) {
  return <McpIcon size={size} aria-hidden="true" />;
}

/**
 * Знак предустановленного провайдера, или `null` у своего.
 *
 * По идентификатору, а не по адресу или названию: предустановленные
 * узнаются ровно так же, как их узнаёт ядро. Свой шлюз с адресом OpenAI
 * остаётся без знака — он чей-то ещё, и чужой логотип над ним соврал бы.
 */
export function ProviderMark({ id, size }: { id: string; size: number }) {
  const Mark = PROVIDER_MARKS.get(id);
  return Mark === undefined ? null : <Mark size={size} aria-hidden="true" />;
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
