interface Props {
  items: { key: string | number; label: string; note?: string; value: number }[];
  /** Read by screen readers before the list. */
  label: string;
  format?: (value: number) => string;
}

/** A ranked list with a horizontal bar per row and the value at the bar's tip. */
export function BarList({ items, label, format = (value) => value.toLocaleString("en") }: Props) {
  const max = Math.max(0, ...items.map((item) => item.value));
  return (
    <ol aria-label={label} className="space-y-2.5">
      {items.map((item) => {
        const percent = max > 0 ? (item.value / max) * 100 : 0;
        return (
          <li key={item.key} className="grid grid-cols-[minmax(0,10rem)_minmax(0,1fr)] items-center gap-3 text-sm">
            <div className="min-w-0">
              <div className="truncate">{item.label}</div>
              {item.note && <div className="truncate text-xs text-fg-subtle">{item.note}</div>}
            </div>
            <div className="flex min-w-0 items-center gap-2">
              {/* rounded at the data end, square at the baseline; 4rem is kept for the value */}
              <div
                className="h-2.5 shrink-0 rounded-r-sm bg-chart-1"
                style={{ width: `calc(${percent}% - ${(percent / 100) * 4}rem)` }}
              />
              <span className="shrink-0 text-xs tabular-nums text-fg-muted">{format(item.value)}</span>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
