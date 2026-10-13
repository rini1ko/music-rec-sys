import type {
  Comparison,
  Evaluation,
  ModelStatus,
  Overview,
  Recommendation,
  RecommendationsResponse,
  Track,
  UserProfile,
  UsersResponse,
  Weights,
} from "./types";

const BASE_URL: string = import.meta.env.VITE_API_URL ?? "http://127.0.0.1:3000";
/** Sent with writes when the server is started with an API key (`--api-key`). */
const API_KEY: string | undefined = import.meta.env.VITE_API_KEY;

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE_URL}${path}`, init);
  } catch {
    throw new Error("Cannot reach the server. Is it running?");
  }
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? `Request failed with status ${response.status}`);
  }
  return (await response.json()) as T;
}

function post<T>(path: string, body?: unknown): Promise<T> {
  return request<T>(path, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...(API_KEY ? { "x-api-key": API_KEY } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

export const api = {
  users: (limit = 50) => request<UsersResponse>(`/users?limit=${limit}`),
  user: (userIndex: number) => request<UserProfile>(`/users/${userIndex}`),
  createUser: (country?: string) => post<UserProfile>("/users", { country }),
  recommendations: (user: number) => request<RecommendationsResponse>(`/recommendations?user=${user}`),
  compare: (user: number, weights: Weights) => {
    const params = new URLSearchParams({ user: String(user) });
    for (const [key, value] of Object.entries(weights)) params.set(key, String(value));
    return request<Comparison>(`/recommendations/compare?${params}`);
  },
  track: (trackIndex: number) => request<Track>(`/tracks/${trackIndex}`),
  similar: (trackIndex: number) => request<Recommendation[]>(`/tracks/${trackIndex}/similar?top=10`),
  searchTracks: (text: string) => request<Track[]>(`/tracks?q=${encodeURIComponent(text)}&limit=10`),
  addEvent: (user: number, track: number) => post<unknown>("/events", { user, track }),
  model: () => request<ModelStatus>("/model"),
  retrain: () => post<ModelStatus>("/model/retrain"),
  evaluation: () => request<Evaluation>("/model/evaluation"),
  evaluate: () => post<Evaluation>("/model/evaluate"),
  overview: (days: number) => request<Overview>(`/stats/overview?days=${days}&top=8`),
};
