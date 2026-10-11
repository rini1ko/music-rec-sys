import { matchPath } from "react-router";

/** Pages that work with one selected listener. Their URL is `/<section>/<user>`. */
export type ListenerSection = "recommendations" | "player" | "sandbox" | "radio" | "history";

export const LISTENER_SECTIONS: readonly { id: ListenerSection; label: string }[] = [
  { id: "recommendations", label: "Recommendations" },
  { id: "player", label: "Player" },
  { id: "radio", label: "Radio" },
  { id: "history", label: "History" },
  { id: "sandbox", label: "Weights" },
];

export const paths = {
  home: "/",
  models: "/models",
  dashboard: "/dashboard",
  artists: "/artists",
  artist: (artistIndex: number) => `/artists/${artistIndex}`,
  /** `trail` is the chain of tracks the reader came through, oldest first. */
  track: (trackIndex: number, trail: readonly number[] = []) =>
    `/tracks/${trackIndex}${trail.length > 0 ? `?trail=${trail.join(",")}` : ""}`,
  listener: (section: ListenerSection, user: number | null, search = "") =>
    `/${section}${user === null ? "" : `/${user}`}${search}`,
};

/**
 * Reads an index (of a user, a track or an artist) from the URL. `undefined`
 * means the URL has none, `null` means it has something that is not an index.
 */
export function parseIndex(raw: string | undefined): number | null | undefined {
  if (raw === undefined) return undefined;
  if (!/^\d{1,9}$/.test(raw)) return null;
  return Number(raw);
}

/** Kept for the listener pages, which read `:userId`. */
export const parseUserIndex = parseIndex;

/** A list of indexes like `12,40,7`; broken parts are dropped. */
export function parseIndexList(raw: string | null): number[] {
  if (raw === null || raw === "") return [];
  return raw
    .split(",")
    .map((part) => parseIndex(part))
    .filter((value): value is number => typeof value === "number");
}

/** The listener section and user of a pathname, if it belongs to one. */
export function matchListener(
  pathname: string,
): { section: ListenerSection; user: number | null } | null {
  for (const { id } of LISTENER_SECTIONS) {
    const match = matchPath(`/${id}/:user?`, pathname);
    if (match) {
      return { section: id, user: parseUserIndex(match.params.user) ?? null };
    }
  }
  return null;
}
