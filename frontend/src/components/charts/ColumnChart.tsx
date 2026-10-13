import { useState } from "react";
import { useElementWidth } from "../../hooks/useElementWidth";
import { moveCursor, niceCeil } from "../../lib/chart";
import { compact } from "../../lib/format";

export interface ChartPoint {
  /** Short label under the column. */
  label: string;
  /** Longer label for the tooltip. */
  detail: string;
  value: number;
}

interface Props {
  points: ChartPoint[];
  /** What the chart shows, read by screen readers. */
  label: string;
  unit: string;
  /** At most this many labels under the columns, so they do not overlap. */
  maxLabels?: number;
}

const HEIGHT = 180;
const MARGIN = { top: 12, right: 8, bottom: 24, left: 40 };
const AXIS_TEXT = { fontSize: 11, fill: "var(--fg-subtle)" };

/** A column rounded at the data end (4px) and square at the baseline. */
function columnPath(x: number, top: number, width: number, baseline: number): string {
  const r = Math.min(4, width / 2, baseline - top);
  return `M${x},${baseline} V${top + r} Q${x},${top} ${x + r},${top} H${x + width - r} Q${x + width},${top} ${x + width},${top + r} V${baseline} Z`;
}

/**
 * One series of ordered buckets (days, hours). Hovering a column shows its
 * value; with focus, the arrow keys read the columns out one by one.
 */
export function ColumnChart({ points, label, unit, maxLabels = 12 }: Props) {
  const { ref, width } = useElementWidth<HTMLDivElement>(600);
  const [active, setActive] = useState<number | null>(null);

  const top = niceCeil(Math.max(0, ...points.map((point) => point.value)));
  const innerHeight = HEIGHT - MARGIN.top - MARGIN.bottom;
  const baseline = MARGIN.top + innerHeight;
  const band = Math.max(1, width - MARGIN.left - MARGIN.right) / Math.max(1, points.length);
  // columns are at most 24px thick, the rest of the band is a gap
  const barWidth = Math.max(2, Math.min(24, band - 2));
  const y = (value: number) => baseline - (value / top) * innerHeight;
  const labelEvery = Math.max(1, Math.ceil(points.length / maxLabels));
  const focused = active === null ? null : points[active];

  return (
    <div ref={ref}>
      <svg
        width={width}
        height={HEIGHT}
        role="img"
        aria-label={`${label}. Use the arrow keys to read the values.`}
        tabIndex={0}
        onKeyDown={(event) => moveCursor(event, active, points.length, setActive)}
        onBlur={() => setActive(null)}
        className="block rounded-md focus-visible:outline-2 focus-visible:outline-accent"
      >
        {[0, top / 2, top].map((tick) => (
          <g key={tick}>
            <line x1={MARGIN.left} x2={width - MARGIN.right} y1={y(tick)} y2={y(tick)} stroke="var(--line)" />
            <text x={MARGIN.left - 6} y={y(tick)} dy="0.32em" textAnchor="end" {...AXIS_TEXT}>
              {compact(tick)}
            </text>
          </g>
        ))}
        {points.map((point, index) => (
          <g key={point.detail}>
            {/* the whole band is the hover target, its title is the tooltip */}
            <rect x={MARGIN.left + band * index} y={MARGIN.top} width={band} height={innerHeight} fill="transparent">
              <title>{`${point.detail}: ${point.value.toLocaleString("en")} ${unit}`}</title>
            </rect>
            {point.value > 0 && (
              <path
                d={columnPath(MARGIN.left + band * index + (band - barWidth) / 2, y(point.value), barWidth, baseline)}
                fill="var(--chart-1)"
                opacity={active === null || active === index ? 1 : 0.45}
                pointerEvents="none"
              />
            )}
            {index % labelEvery === 0 && (
              <text x={MARGIN.left + band * index + band / 2} y={baseline + 16} textAnchor="middle" {...AXIS_TEXT}>
                {point.label}
              </text>
            )}
          </g>
        ))}
      </svg>
      <p className="sr-only" aria-live="polite">
        {focused ? `${focused.detail}: ${focused.value} ${unit}` : ""}
      </p>
    </div>
  );
}
