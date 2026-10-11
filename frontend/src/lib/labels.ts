import type { Source, WeightKey } from "../api/types";

/** Short names of the sources, shown on badges and next to the weight sliders. */
export const SOURCE_LABELS: Record<Source, string> = {
  trends: "Trending",
  markov: "Next track",
  co_listening: "Played together",
  cf: "Similar listeners",
  artist: "Favorite artist",
  country: "Popular nearby",
  popular: "Popular",
};

/** The models with a weight, in the order they are shown. */
export const WEIGHT_KEYS: readonly WeightKey[] = ["trends", "markov", "co_listening", "cf"];

export const MODEL_DESCRIPTIONS: Record<WeightKey, string> = {
  trends: "Tracks played the most during the last day of the data.",
  markov: "What listeners usually play right after the last track.",
  co_listening: "Tracks often played in the same sessions as the recent ones.",
  cf: "Tracks from listeners whose history looks like this one.",
};

/** How many plays the onboarding of a new listener asks for. */
export const ONBOARDING_PLAYS = 3;
