export type Source = "trends" | "markov" | "co_listening" | "cf" | "mf" | "artist" | "country" | "popular";

/** Models whose weight can be changed, a subset of `Source`. */
export type WeightKey = "trends" | "markov" | "co_listening" | "cf" | "mf";

export type Weights = Record<WeightKey, number>;

export interface Reason {
  source: Source;
  text: string;
  contribution: number;
}

export interface Track {
  track_index: number;
  title: string | null;
  artist: string | null;
}

export interface Recommendation extends Track {
  score: number;
  reasons: Reason[];
}

export interface RecommendationsResponse {
  user: number;
  history_len: number;
  items: Recommendation[];
}

export type Movement = { kind: "same" | "new" } | { kind: "up" | "down"; by: number };

/** `/recommendations/compare`: the default list and the list with other weights. */
export interface Comparison {
  history_len: number;
  base: Recommendation[];
  custom: (Recommendation & { movement: Movement })[];
  up: number;
  down: number;
  added: number;
  gone: number;
}

export interface UsersResponse {
  total: number;
  items: { user_index: number; country: string | null; plays: number }[];
}

export interface UserProfile {
  user_index: number;
  country: string | null;
  history_len: number;
}

export interface ModelStatus {
  training: boolean;
  trained_at: string;
  train_seconds: number;
  events: number;
  sessions: number;
  last_error: string | null;
  weights: Weights;
}

export interface ModelScore {
  model: WeightKey | "markov_2" | "hybrid" | "hybrid_rules";
  ndcg: number;
  hit_rate: number;
  coverage: number;
  novelty: number;
  diversity: number | null;
}

export interface Evaluation {
  running: boolean;
  k: number;
  users: number;
  last_error: string | null;
  models: ModelScore[];
}

export interface Overview {
  from: string | null;
  to: string | null;
  plays: number;
  top_artists: { artist_index: number; name: string | null; plays: number; listeners: number }[];
  /** Every day of the window, quiet days with 0. */
  by_day: { day: string; plays: number }[];
  /** 24 numbers, index is the hour. */
  by_hour: number[];
}
