/** 950, 1.2K, 34K, 1.5M: short numbers for axes. */
export function compact(value: number): string {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

// the data has no time zone, so dates are read and shown in UTC and never shifted
const DAY = new Intl.DateTimeFormat("en", { month: "short", day: "numeric", timeZone: "UTC" });

/** `2009-05-04` → `May 4`. */
export function formatDay(day: string): string {
  return DAY.format(new Date(`${day.slice(0, 10)}T00:00:00Z`));
}

export const trackTitle = (title: string | null) => title ?? "Unknown track";
export const artistName = (name: string | null) => name ?? "Unknown artist";
