import type {
  EventCreated,
  Health,
  ModelStatus,
  NewEvent,
  RecommendationsResponse,
  Track,
  UserProfile,
  UsersResponse,
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

export const api = {
  health: () => request<Health>("/health"),
  users: (limit = 50, offset = 0) =>
    request<UsersResponse>(`/users?limit=${limit}&offset=${offset}`),
  user: (userIndex: number) => request<UserProfile>(`/users/${userIndex}`),
  recommendations: (user: number, top = 10) =>
    request<RecommendationsResponse>(`/recommendations?user=${user}&top=${top}`),
  searchTracks: (text: string, limit = 20) =>
    request<Track[]>(`/tracks?q=${encodeURIComponent(text)}&limit=${limit}`),
  addEvent: (event: NewEvent) =>
    request<EventCreated>("/events", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(event),
    }),
  model: () => request<ModelStatus>("/model"),
  retrain: () => request<ModelStatus>("/model/retrain", { method: "POST" }),
};