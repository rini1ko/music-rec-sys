interface Props {
  /** Point the tooltip stands on, in pixels from the chart's top left corner. */
  x: number;
  y: number;
  /** Width of the chart, so the tooltip never leaves it. */
  width: number;
  value: string;
  label: string;
}

const HALF_WIDTH = 70;

/** The value first and strong, what it belongs to under it. */
export function ChartTooltip({ x, y, width, value, label }: Props) {
  const left = Math.min(Math.max(x, HALF_WIDTH), Math.max(HALF_WIDTH, width - HALF_WIDTH));
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-full whitespace-nowrap rounded-md border border-line-strong bg-surface-raised px-2 py-1 text-xs shadow-lg"
      style={{ left, top: y - 8 }}
    >
      <p className="font-semibold text-fg">{value}</p>
      <p className="text-fg-muted">{label}</p>
    </div>
  );
}
