import type { Source, WeightKey } from "../api/types";

/** Short names of the sources, shown on badges, sliders and the dashboard. */
export const SOURCE_LABELS: Record<Source, string> = {
  trends: "Trending",
  markov: "Next track",
  co_listening: "Played together",
  cf: "Similar listeners",
  mf: "Hidden taste",
  artist: "Favorite artist",
  country: "Popular nearby",
  popular: "Popular",
};

/** The models with a weight, in the order they are shown. */
export const WEIGHT_KEYS: readonly WeightKey[] = ["trends", "markov", "co_listening", "cf", "mf"];
