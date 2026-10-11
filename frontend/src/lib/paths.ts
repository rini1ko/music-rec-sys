import { matchPath } from "react-router";

/** Pages that work with one selected listener. Their URL is `/<section>/<user>`. */
export type ListenerSection = "recommendations" | "player" | "sandbox";

export const LISTENER_SECTIONS: readonly { id: ListenerSection; label: string }[] = [
  { id: "recommendations", label: "Recommendations" },
  { id: "player", label: "Player" },
  { id: "sandbox", label: "Weights" },
];

export const paths = {
  home: "/",
  models: "/models",
  listener: (section: ListenerSection, user: number | null, search = "") =>
    `/${section}${user === null ? "" : `/${user}`}${search}`,
};

/**
 * Reads a user index from the URL. `undefined` means the URL has none,
 * `null` means it has something that is not a valid index.
 */
export function parseUserIndex(raw: string | undefined): number | null | undefined {
  if (raw === undefined) return undefined;
  if (!/^\d{1,9}$/.test(raw)) return null;
  return Number(raw);
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
