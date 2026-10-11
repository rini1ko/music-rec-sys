import { useState } from "react";
import { useElementWidth } from "../../hooks/useElementWidth";
import { indexOfMax, moveCursor, ticksFor } from "../../lib/chart";
import { compact, whole } from "../../lib/format";
import { ChartTooltip } from "./ChartTooltip";

export interface ChartPoint {
  /** Short label under the column. */
  label: string;
  value: number;
  /** Longer label for the tooltip, the label is used when missing. */
  detail?: string;
}

interface Props {
  points: ChartPoint[];
  /** What the chart shows, read by screen readers. */
  label: string;
  /** Unit of the values, e.g. "plays". */
  unit: string;
  /** Show every n-th label under the columns, so they do not overlap. */
  labelEvery?: number;
  height?: number;
}

const MARGIN = { top: 20, right: 8, bottom: 24, left: 40 };
const MAX_BAR = 24;
const GAP = 2;
const RADIUS = 4;

/** A column rounded at the data end and square at the baseline. */
function columnPath(x: number, top: number, width: number, baseline: number): string {
  const radius = Math.min(RADIUS, width / 2, baseline - top);
  return [
    `M${x},${baseline}`,
    `V${top + radius}`,
    `Q${x},${top} ${x + radius},${top}`,
    `H${x + width - radius}`,
    `Q${x + width},${top} ${x + width},${top + radius}`,
    `V${baseline}`,
    "Z",
  ].join(" ");
}

/** Columns for a handful of ordered buckets: hours, weekdays. */
export function ColumnChart({ points, label, unit, labelEvery = 1, height = 180 }: Props) {
  const { ref, width } = useElementWidth<HTMLDivElement>(600);
  const [active, setActive] = useState<number | null>(null);

  const values = points.map((point) => point.value);
  const ticks = ticksFor(Math.max(0, ...values));
  const scaleTop = ticks[ticks.length - 1];
  const innerWidth = Math.max(1, width - MARGIN.left - MARGIN.right);
  const innerHeight = height - MARGIN.top - MARGIN.bottom;
  const baseline = MARGIN.top + innerHeight;
  const band = innerWidth / Math.max(1, points.length);
  const barWidth = Math.max(2, Math.min(MAX_BAR, band - GAP));
  const y = (value: number) => baseline - (value / scaleTop) * innerHeight;
  const center = (index: number) => MARGIN.left + band * index + band / 2;
  const peak = indexOfMax(values);
  const focused = active === null ? null : points[active];

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
        onPointerLeave={() => setActive(null)}
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

        {points.map((point, index) => {
          const x = center(index) - barWidth / 2;
          return (
            <g key={point.label}>
              {point.value > 0 && (
                <path
                  d={columnPath(x, y(point.value), barWidth, baseline)}
                  fill="var(--chart-1)"
                  opacity={active === null || active === index ? 1 : 0.45}
                />
              )}
              {index % labelEvery === 0 && (
                <text
                  x={center(index)}
                  y={baseline + 16}
                  textAnchor="middle"
                  fontSize={11}
                  fill="var(--fg-subtle)"
                >
                  {point.label}
                </text>
              )}
              {/* the whole band is the hover target, not only the painted column */}
              <rect
                x={MARGIN.left + band * index}
                y={MARGIN.top}
                width={band}
                height={innerHeight}
                fill="transparent"
                onPointerEnter={() => setActive(index)}
              />
            </g>
          );
        })}

        {peak >= 0 && values[peak] > 0 && active === null && (
          <text
            x={center(peak)}
            y={y(values[peak]) - 6}
            textAnchor="middle"
            fontSize={11}
            fill="var(--fg-muted)"
            className="tabular-nums"
          >
            {compact(values[peak])}
          </text>
        )}
      </svg>

      {focused !== null && active !== null && (
        <ChartTooltip
          x={center(active)}
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
