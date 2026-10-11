export type Source = "trends" | "markov" | "co_listening" | "cf" | "artist" | "country" | "popular";

/** Models whose weight can be changed, a subset of `Source`. */
export type WeightKey = "trends" | "markov" | "co_listening" | "cf";

export type Weights = Record<WeightKey, number>;

export interface Reason {
  source: Source;
  text: string;
  contribution: number;
}

export interface Recommendation {
  track_index: number;
  title: string | null;
  artist: string | null;
  /** Null only when the track is missing from the catalog. */
  artist_index: number | null;
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
  artist_index: number;
}

export interface TrackPlays extends Track {
  plays: number;
}

export interface Artist {
  artist_index: number;
  name: string | null;
  /** All tracks of the artist in a search, played ones in the statistics. */
  tracks: number;
  plays: number;
  listeners: number;
}

export interface ArtistProfile extends Artist {
  top_tracks: TrackPlays[];
}

export interface DayPlays {
  /** `YYYY-MM-DD`. */
  day: string;
  plays: number;
}

export interface CountryPlays {
  country: string | null;
  listeners: number;
  plays: number;
}

export interface Overview {
  days: number;
  from: string | null;
  to: string | null;
  plays: number;
  listeners: number;
  tracks: number;
  artists: number;
  top_tracks: TrackPlays[];
  top_artists: Artist[];
  /** Days without plays are missing. */
  by_day: DayPlays[];
  /** 24 numbers, index is the hour. */
  by_hour: number[];
  /** 7 numbers, index 0 is Sunday. */
  by_weekday: number[];
  countries: CountryPlays[];
}

export interface HistoryEntry extends Track {
  timestamp: string;
}

export interface HistoryResponse {
  user: number;
  total: number;
  limit: number;
  offset: number;
  items: HistoryEntry[];
}

export interface RadioResponse {
  user: number;
  history_len: number;
  items: Recommendation[];
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
  /** Used when a request does not override them. */
  weights: Weights;
}

export interface NewUser {
  country?: string;
  /** From 1 to 120. */
  age?: number;
  gender?: string;
}