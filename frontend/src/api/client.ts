import type {
  EventCreated,
  Health,
  ModelStatus,
  NewEvent,
  NewUser,
  RecommendationsResponse,
  Track,
  UserProfile,
  UsersResponse,
  Weights,
} from "./types";

const BASE_URL: string = import.meta.env.VITE_API_URL ?? "http://127.0.0.1:3000";

export class ApiError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE_URL}${path}`, init);
  } catch {
    throw new ApiError(0, "Cannot reach the server. Is it running?");
  }

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new ApiError(response.status, body?.error ?? `Request failed with status ${response.status}`);
  }
  return (await response.json()) as T;
}

function postJson<T>(path: string, body: unknown): Promise<T> {
  return request<T>(path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

/** Query string of a recommendations request. Missing weights keep the server defaults. */
export function recommendationsQuery(user: number, top: number, weights?: Partial<Weights>): string {
  const params = new URLSearchParams({ user: String(user), top: String(top) });
  for (const [key, value] of Object.entries(weights ?? {})) {
    if (value !== undefined) params.set(key, String(value));
  }
  return params.toString();
}

export const api = {
  health: () => request<Health>("/health"),
  users: (limit = 50, offset = 0) =>
    request<UsersResponse>(`/users?limit=${limit}&offset=${offset}`),
  user: (userIndex: number) => request<UserProfile>(`/users/${userIndex}`),
  createUser: (user: NewUser) => postJson<UserProfile>("/users", user),
  recommendations: (user: number, top = 10, weights?: Partial<Weights>) =>
    request<RecommendationsResponse>(`/recommendations?${recommendationsQuery(user, top, weights)}`),
  searchTracks: (text: string, limit = 20) =>
    request<Track[]>(`/tracks?q=${encodeURIComponent(text)}&limit=${limit}`),
  addEvent: (event: NewEvent) => postJson<EventCreated>("/events", event),
  model: () => request<ModelStatus>("/model"),
  retrain: () => request<ModelStatus>("/model/retrain", { method: "POST" }),
};