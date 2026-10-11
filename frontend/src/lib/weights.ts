import type { WeightKey, Weights } from "../api/types";
import { WEIGHT_KEYS } from "./labels";

/** The server accepts weights from 0 to 10. */
export const MAX_WEIGHT = 10;
/** The sliders cover the useful part of the range, the number inputs all of it. */
export const SLIDER_MAX = 1;
export const WEIGHT_STEP = 0.05;

export const TOP_OPTIONS = [10, 20, 50] as const;
export type TopOption = (typeof TOP_OPTIONS)[number];

function parseWeight(raw: string | null): number | null {
  if (raw === null || raw.trim() === "") return null;
  const value = Number(raw);
  return Number.isFinite(value) && value >= 0 && value <= MAX_WEIGHT ? value : null;
}

/** Rounds to the slider step and drops float noise like 0.30000000000000004. */
export function roundWeight(value: number): number {
  const clamped = Math.min(MAX_WEIGHT, Math.max(0, value));
  return Number((Math.round(clamped / WEIGHT_STEP) * WEIGHT_STEP).toFixed(2));
}

/** Weights from the URL. A missing or broken value falls back to the default one. */
export function weightsFromSearch(search: URLSearchParams, defaults: Weights): Weights {
  const entries = WEIGHT_KEYS.map((key) => [key, parseWeight(search.get(key)) ?? defaults[key]]);
  return Object.fromEntries(entries) as Weights;
}

/** Writes only the weights that differ from the defaults, so a shared link stays short. */
export function weightsToSearch(search: URLSearchParams, weights: Weights, defaults: Weights): URLSearchParams {
  const next = new URLSearchParams(search);
  for (const key of WEIGHT_KEYS) {
    if (weights[key] === defaults[key]) next.delete(key);
    else next.set(key, String(weights[key]));
  }
  return next;
}

export function topFromSearch(search: URLSearchParams): TopOption {
  const value = Number(search.get("top"));
  return TOP_OPTIONS.find((option) => option === value) ?? TOP_OPTIONS[0];
}

export function sameWeights(left: Weights, right: Weights): boolean {
  return WEIGHT_KEYS.every((key) => left[key] === right[key]);
}

/** Share of each weight in their sum, from 0 to 1. All zeros give all zeros. */
export function weightShares(weights: Weights): Record<WeightKey, number> {
  const total = WEIGHT_KEYS.reduce((sum, key) => sum + weights[key], 0);
  const entries = WEIGHT_KEYS.map((key) => [key, total > 0 ? weights[key] / total : 0]);
  return Object.fromEntries(entries) as Record<WeightKey, number>;
}

/** A stable string for cache keys of requests. `weightsFromKey` reads it back. */
export function weightsKey(weights: Weights): string {
  return WEIGHT_KEYS.map((key) => weights[key]).join(",");
}

export function weightsFromKey(text: string): Weights {
  const values = text.split(",").map(Number);
  const entries = WEIGHT_KEYS.map((key, index) => [key, values[index] ?? 0]);
  return Object.fromEntries(entries) as Weights;
}
