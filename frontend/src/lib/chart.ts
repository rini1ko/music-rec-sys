import type { KeyboardEvent } from "react";

/** The smallest "round" number (1, 2, 2.5, 5 times a power of ten) not below `value`. */
export function niceCeil(value: number): number {
  if (value <= 0) return 1;
  const power = 10 ** Math.floor(Math.log10(value));
  return ([1, 2, 2.5, 5, 10].find((factor) => factor * power >= value) ?? 10) * power;
}

/** Moves the highlighted column with the keyboard: arrows step, Home and End jump, Escape clears. */
export function moveCursor(
  event: KeyboardEvent,
  active: number | null,
  count: number,
  setActive: (index: number | null) => void,
) {
  const current = active ?? -1;
  const targets: Record<string, number | null> = {
    ArrowRight: Math.min(count - 1, current + 1),
    ArrowLeft: Math.max(0, current - 1),
    Home: 0,
    End: count - 1,
    Escape: null,
  };
  if (count === 0 || !(event.key in targets)) return;
  event.preventDefault();
  setActive(targets[event.key]);
}
