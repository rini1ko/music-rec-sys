export type Source = "trends" | "markov" | "co_listening" | "cf";

export interface Reason {
  source: Source;
  text: string;
  contribution: number;
}

export interface Recommendation {
  track_index: number;
  title: string | null;
  artist: string | null;
  score: number;
  reasons: Reason[];
}

export interface RecommendationsResponse {
  user: number;
  history_len: number;
  items: Recommendation[];
}

export interface Track {
  track_index: number;
  title: string | null;
  artist: string | null;
}

export interface UserSummary {
  user_index: number;
  gender: string | null;
  age: number | null;
  country: string | null;
  plays: number;
}

export interface UsersResponse {
  total: number;
  items: UserSummary[];
}

export interface UserProfile {
  user_index: number;
  gender: string | null;
  age: number | null;
  country: string | null;
  history_len: number;
  recent: Track[];
}

export interface NewEvent {
  user: number;
  track: number;
  timestamp?: string;
}

export interface EventCreated {
  user: number;
  track: number;
  timestamp: string;
}

export interface Health {
  status: string;
  users: number;
  tracks: number;
  events: number;
  sessions: number;
}

export interface ModelStatus {
  training: boolean;
  trained_at: string;
  train_seconds: number;
  events: number;
  sessions: number;
  last_error: string | null;
}