/** 950, 1.2K, 34K, 1.5M: short numbers for axes and tiles. */
export function compact(value: number): string {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

/** Full number with thousands separators. */
export function whole(value: number): string {
  return value.toLocaleString("en");
}

const DATE = new Intl.DateTimeFormat("en", { year: "numeric", month: "short", day: "numeric" });
const DAY = new Intl.DateTimeFormat("en", { month: "short", day: "numeric", timeZone: "UTC" });
const DATE_TIME = new Intl.DateTimeFormat("en", {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

/**
 * The API sends times without a zone (the data has none), so they are shown
 * as they are, never shifted to the viewer's zone.
 */
function parseLocal(text: string): Date {
  return new Date(text.includes("T") ? text : `${text}T00:00:00`);
}

export function formatDate(text: string): string {
  return DATE.format(parseLocal(text));
}

export function formatDateTime(text: string): string {
  return DATE_TIME.format(parseLocal(text));
}

/** `2009-05-04` → `May 4`. */
export function formatDay(day: string): string {
  return DAY.format(new Date(`${day}T00:00:00Z`));
}

/** Index 0 is Sunday, as in the API. */
export const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

export function hourLabel(hour: number): string {
  return `${String(hour).padStart(2, "0")}:00`;
}

export function trackTitle(title: string | null): string {
  return title ?? "Unknown track";
}

export function artistName(name: string | null): string {
  return name ?? "Unknown artist";
}
