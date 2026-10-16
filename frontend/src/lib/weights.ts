import type { Weights } from "../api/types";
import { WEIGHT_KEYS } from "./labels";

/** The server accepts 0 to 10; the sliders cover the useful part of it. */
export const SLIDER_MAX = 1;
export const WEIGHT_STEP = 0.05;

/** Weights from the URL. A missing or broken value falls back to the default one. */
export function weightsFromSearch(search: URLSearchParams, defaults: Weights): Weights {
  const value = (raw: string | null) => {
    const number = Number(raw ?? "");
    return raw !== null && raw !== "" && number >= 0 && number <= 10 ? number : null;
  };
  return Object.fromEntries(
    WEIGHT_KEYS.map((key) => [key, value(search.get(key)) ?? defaults[key]]),
  ) as Weights;
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

