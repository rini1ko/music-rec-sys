import type { Recommendation } from "../api/types";

export type Movement =
  | { kind: "same" }
  | { kind: "up"; by: number }
  | { kind: "down"; by: number }
  | { kind: "new" };

export interface RankedItem {
  item: Recommendation;
  /** 1-based position in its list. */
  rank: number;
}

export interface ComparedItem extends RankedItem {
  /** How the position changed against the baseline list. */
  movement: Movement;
  /** Position in the baseline list, null if the track was not there. */
  baseRank: number | null;
}

export interface Comparison {
  /** The new list, each item with its movement. */
  items: ComparedItem[];
  /** Tracks of the baseline list that the new list lost. */
  dropped: RankedItem[];
  risen: ComparedItem[];
  fallen: ComparedItem[];
  added: ComparedItem[];
  unchanged: number;
}

function movementOf(rank: number, baseRank: number | null): Movement {
  if (baseRank === null) return { kind: "new" };
  if (rank < baseRank) return { kind: "up", by: baseRank - rank };
  if (rank > baseRank) return { kind: "down", by: rank - baseRank };
  return { kind: "same" };
}

/** Compares a list of recommendations with a baseline one, track by track. */
export function compare(base: Recommendation[], next: Recommendation[]): Comparison {
  const baseRanks = new Map(base.map((item, index) => [item.track_index, index + 1]));
  const nextTracks = new Set(next.map((item) => item.track_index));

  const items = next.map((item, index): ComparedItem => {
    const rank = index + 1;
    const baseRank = baseRanks.get(item.track_index) ?? null;
    return { item, rank, baseRank, movement: movementOf(rank, baseRank) };
  });

  const dropped = base
    .map((item, index) => ({ item, rank: index + 1 }))
    .filter(({ item }) => !nextTracks.has(item.track_index));

  const byDistance = (left: ComparedItem, right: ComparedItem) =>
    distance(right.movement) - distance(left.movement) || left.rank - right.rank;

  return {
    items,
    dropped,
    risen: items.filter((entry) => entry.movement.kind === "up").sort(byDistance),
    fallen: items.filter((entry) => entry.movement.kind === "down").sort(byDistance),
    added: items.filter((entry) => entry.movement.kind === "new"),
    unchanged: items.filter((entry) => entry.movement.kind === "same").length,
  };
}

function distance(movement: Movement): number {
  return movement.kind === "up" || movement.kind === "down" ? movement.by : 0;
}

/** Text of a movement for screen readers and tooltips. */
export function describeMovement(movement: Movement): string {
  switch (movement.kind) {
    case "same":
      return "same place";
    case "new":
      return "new in the list";
    case "up":
      return `up ${movement.by} ${movement.by === 1 ? "place" : "places"}`;
    case "down":
      return `down ${movement.by} ${movement.by === 1 ? "place" : "places"}`;
  }
}
