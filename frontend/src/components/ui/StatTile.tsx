import { compact, whole } from "../../lib/format";

interface Props {
  label: string;
  value: number;
  /** Small text under the value, e.g. what the number covers. */
  hint?: string;
}

/** One headline number. Large values are shortened, the full one is in the tooltip. */
export function StatTile({ label, value, hint }: Props) {
  return (
    <div className="rounded-xl border border-line bg-surface/60 p-4">
      <dt className="text-xs text-fg-muted">{label}</dt>
      <dd className="mt-1 text-2xl font-semibold" title={whole(value)}>
        {value >= 10_000 ? compact(value) : whole(value)}
      </dd>
      {hint !== undefined && <dd className="mt-1 text-xs text-fg-subtle">{hint}</dd>}
    </div>
  );
}
