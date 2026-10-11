import type { KeyboardEvent } from "react";
import type { DayPlays } from "../api/types";

/** The smallest "round" number (1, 2, 2.5, 5 times a power of ten) not below `value`. */
export function niceCeil(value: number): number {
  if (value <= 0) return 1;
  const power = 10 ** Math.floor(Math.log10(value));
  const step = [1, 2, 2.5, 5, 10].find((factor) => factor * power >= value) ?? 10;
  return step * power;
}

/** Three gridlines: zero, half and the rounded maximum. */
export function ticksFor(max: number): number[] {
  const top = niceCeil(max);
  return [0, top / 2, top];
}

/** Index of the largest value; the first one wins a tie. -1 for an empty list. */
export function indexOfMax(values: readonly number[]): number {
  return values.reduce((best, value, index) => (best === -1 || value > values[best] ? index : best), -1);
}

/**
 * Moves the highlighted data point with the keyboard: arrows step, Home and
 * End jump, Escape clears. Returns true when the key was handled.
 */
export function moveCursor(
  event: KeyboardEvent,
  active: number | null,
  count: number,
  setActive: (index: number | null) => void,
): boolean {
  if (count === 0) return false;
  const current = active ?? -1;
  const targets: Record<string, number | null> = {
    ArrowRight: Math.min(count - 1, current + 1),
    ArrowLeft: Math.max(0, current - 1),
    Home: 0,
    End: count - 1,
    Escape: null,
  };
  if (!(event.key in targets)) return false;
  event.preventDefault();
  setActive(targets[event.key]);
  return true;
}

/**
 * Plays for every day from `from` to `to`, with zeros for days the API left
 * out. Without zeros a quiet day would be drawn as a straight line between
 * its neighbours.
 */
export function fillDays(byDay: readonly DayPlays[], from: string | null, to: string | null): DayPlays[] {
  if (from === null || to === null) return [...byDay];
  const plays = new Map(byDay.map((item) => [item.day, item.plays]));
  const days: DayPlays[] = [];
  const cursor = new Date(`${from.slice(0, 10)}T00:00:00Z`);
  const last = new Date(`${to.slice(0, 10)}T00:00:00Z`);
  while (cursor <= last) {
    const day = cursor.toISOString().slice(0, 10);
    days.push({ day, plays: plays.get(day) ?? 0 });
    cursor.setUTCDate(cursor.getUTCDate() + 1);
  }
  return days;
}
