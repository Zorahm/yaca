import type { Effort, ModelInfo } from "./client";
import type { Key } from "../i18n";

export const EFFORT_LEVELS: Effort[] = ["none", "minimal", "low", "medium", "high", "extra", "max"];

export const EFFORT_KEY: Record<Effort, Key> = {
  auto: "effort.auto",
  none: "effort.none",
  minimal: "effort.minimal",
  low: "effort.low",
  medium: "effort.medium",
  high: "effort.high",
  extra: "effort.extra",
  max: "effort.max",
};

/** A reasoning tag alone says nothing about which depths the endpoint accepts. */
export function effortLevels(model: ModelInfo | undefined): Effort[] {
  if (!model?.supportsEffort) return [];
  return EFFORT_LEVELS.filter((level) =>
    model.reasoning?.supportedEfforts?.includes(level) &&
    !(level === "none" && model.reasoning.mandatory),
  );
}

export function effectiveEffort(model: ModelInfo | undefined, effort: Effort): Effort {
  return effortLevels(model).includes(effort) ? effort : "auto";
}
