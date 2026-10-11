import type { ReactNode } from "react";
import { whole } from "../../lib/format";

export interface BarListItem {
  key: string | number;
  /** Text or a link. */
  label: ReactNode;
  /** Plain text of the label, for the tooltip. */
  title: string;
  value: number;
  /** Small text under the label, e.g. the artist of a track. */
  note?: ReactNode;
}

interface Props {
  items: BarListItem[];
  unit: string;
  /** Read by screen readers before the list. */
  label: string;
}

/** A ranked list with a horizontal bar per row and the value at the bar's tip. */
export function BarList({ items, unit, label }: Props) {
  const max = Math.max(0, ...items.map((item) => item.value));

  return (
    <ol aria-label={label} className="space-y-2.5">
      {items.map((item) => {
        const percent = max > 0 ? (item.value / max) * 100 : 0;
        return (
          <li
            key={item.key}
            title={`${item.title}: ${whole(item.value)} ${unit}`}
            className="grid grid-cols-[minmax(0,11rem)_minmax(0,1fr)] items-center gap-3 text-sm sm:grid-cols-[minmax(0,14rem)_minmax(0,1fr)]"
          >
            <div className="min-w-0">
              <div className="truncate">{item.label}</div>
              {item.note !== undefined && <div className="truncate text-xs text-fg-subtle">{item.note}</div>}
            </div>
            <div className="flex min-w-0 items-center gap-2">
              {/* rounded at the data end, square at the baseline */}
              <div
                className="h-2.5 shrink-0 rounded-r-sm bg-chart-1"
                style={{ width: `calc(${percent}% - ${(percent / 100) * 4}rem)` }}
              />
              <span className="shrink-0 text-xs tabular-nums text-fg-muted">
                {whole(item.value)}
                <span className="sr-only"> {unit}</span>
              </span>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
