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
  dashboard: "/dashboard",
  track: (trackIndex: number) => `/tracks/${trackIndex}`,
  listener: (section: ListenerSection, user: number | null, search = "") =>
    `/${section}${user === null ? "" : `/${user}`}${search}`,
};

/** An index from the URL. `undefined`: the URL has none, `null`: it is not a number. */
export function parseIndex(raw: string | undefined): number | null | undefined {
  if (raw === undefined) return undefined;
  return /^\d{1,9}$/.test(raw) ? Number(raw) : null;
}

/** The listener section and user of a pathname, if it belongs to one. */
export function matchListener(pathname: string): { section: ListenerSection; user: number | null } | null {
  for (const { id } of LISTENER_SECTIONS) {
    const match = matchPath(`/${id}/:user?`, pathname);
    if (match) return { section: id, user: parseIndex(match.params.user) ?? null };
  }
  return null;
}
