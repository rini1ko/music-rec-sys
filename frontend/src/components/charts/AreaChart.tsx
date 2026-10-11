import { useState, type PointerEvent } from "react";
import { useElementWidth } from "../../hooks/useElementWidth";
import { moveCursor, ticksFor } from "../../lib/chart";
import { compact, whole } from "../../lib/format";
import { ChartTooltip } from "./ChartTooltip";
import type { ChartPoint } from "./ColumnChart";

interface Props {
  points: ChartPoint[];
  label: string;
  unit: string;
  /** How many labels to show under the line at most. */
  maxLabels?: number;
  height?: number;
}

const MARGIN = { top: 20, right: 16, bottom: 24, left: 40 };

/** One series over time: a 2px line over a light wash, with a crosshair. */
export function AreaChart({ points, label, unit, maxLabels = 6, height = 200 }: Props) {
  const { ref, width } = useElementWidth<HTMLDivElement>(600);
  const [active, setActive] = useState<number | null>(null);

  const values = points.map((point) => point.value);
  const ticks = ticksFor(Math.max(0, ...values));
  const scaleTop = ticks[ticks.length - 1];
  const innerWidth = Math.max(1, width - MARGIN.left - MARGIN.right);
  const innerHeight = height - MARGIN.top - MARGIN.bottom;
  const baseline = MARGIN.top + innerHeight;
  const step = points.length > 1 ? innerWidth / (points.length - 1) : 0;
  const x = (index: number) => (points.length > 1 ? MARGIN.left + step * index : MARGIN.left + innerWidth / 2);
  const y = (value: number) => baseline - (value / scaleTop) * innerHeight;

  const line = points.map((point, index) => `${index === 0 ? "M" : "L"}${x(index)},${y(point.value)}`).join(" ");
  const area = points.length > 0 ? `${line} L${x(points.length - 1)},${baseline} L${x(0)},${baseline} Z` : "";
  const labelEvery = Math.max(1, Math.ceil(points.length / maxLabels));
  const last = points.length - 1;
  const focused = active === null ? null : points[active];

  // the crosshair snaps to the nearest day, the reader aims at a date, not at the line
  function track(event: PointerEvent<SVGRectElement>) {
    if (points.length === 0) return;
    const box = event.currentTarget.getBoundingClientRect();
    const offset = event.clientX - box.left;
    const index = step > 0 ? Math.round(offset / step) : 0;
    setActive(Math.min(last, Math.max(0, index)));
  }

  return (
    <div ref={ref} className="relative">
      <svg
        width={width}
        height={height}
        role="img"
        aria-label={`${label}. Use the arrow keys to read the values.`}
        tabIndex={0}
        onKeyDown={(event) => moveCursor(event, active, points.length, setActive)}
        onBlur={() => setActive(null)}
        className="block rounded-md focus-visible:outline-2 focus-visible:outline-accent"
      >
        {ticks.map((tick) => (
          <g key={tick}>
            <line
              x1={MARGIN.left}
              x2={width - MARGIN.right}
              y1={y(tick)}
              y2={y(tick)}
              stroke="var(--chart-grid)"
              strokeWidth={1}
            />
            <text
              x={MARGIN.left - 6}
              y={y(tick)}
              dy="0.32em"
              textAnchor="end"
              fontSize={11}
              fill="var(--fg-subtle)"
              className="tabular-nums"
            >
              {compact(tick)}
            </text>
          </g>
        ))}

        <path d={area} fill="var(--chart-1)" opacity={0.1} />
        <path d={line} fill="none" stroke="var(--chart-1)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />

        {points.map((point, index) =>
          index % labelEvery === 0 || index === last ? (
            <text
              key={point.label}
              x={x(index)}
              y={baseline + 16}
              textAnchor={index === 0 ? "start" : index === last ? "end" : "middle"}
              fontSize={11}
              fill="var(--fg-subtle)"
            >
              {point.label}
            </text>
          ) : null,
        )}

        {active !== null && focused !== null && (
          <>
            <line
              x1={x(active)}
              x2={x(active)}
              y1={MARGIN.top}
              y2={baseline}
              stroke="var(--fg-subtle)"
              strokeWidth={1}
            />
            <circle cx={x(active)} cy={y(focused.value)} r={4} fill="var(--chart-1)" stroke="var(--surface)" strokeWidth={2} />
          </>
        )}

        {last >= 0 && active === null && (
          <>
            <circle cx={x(last)} cy={y(values[last])} r={4} fill="var(--chart-1)" stroke="var(--surface)" strokeWidth={2} />
            <text
              x={x(last) - 8}
              y={y(values[last]) - 10}
              textAnchor="end"
              fontSize={11}
              fill="var(--fg-muted)"
              className="tabular-nums"
            >
              {compact(values[last])}
            </text>
          </>
        )}

        <rect
          x={MARGIN.left - step / 2}
          y={MARGIN.top}
          width={innerWidth + step}
          height={innerHeight}
          fill="transparent"
          onPointerMove={track}
          onPointerLeave={() => setActive(null)}
        />
      </svg>

      {focused !== null && active !== null && (
        <ChartTooltip
          x={x(active)}
          y={y(focused.value)}
          width={width}
          value={`${whole(focused.value)} ${unit}`}
          label={focused.detail ?? focused.label}
        />
      )}
      <p className="sr-only" aria-live="polite">
        {focused === null ? "" : `${focused.detail ?? focused.label}: ${whole(focused.value)} ${unit}`}
      </p>
    </div>
  );
}
