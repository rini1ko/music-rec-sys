import type { ReactNode } from "react";
import type { Recommendation } from "../api/types";
import { ReasonBadge } from "./ReasonBadge";

interface Props {
  rank: number;
  item: Recommendation;
  maxScore: number;
  /** Extra marker next to the rank, e.g. how the position changed. */
  marker?: ReactNode;
  /** A smaller card without the score bar, for side-by-side lists. */
  compact?: boolean;
}

export function RecommendationCard({ rank, item, maxScore, marker, compact = false }: Props) {
  const width = maxScore > 0 ? Math.round((item.score / maxScore) * 100) : 0;

  return (
    <li className="flex gap-4 rounded-xl border border-line bg-surface/60 p-4">
      <div className="flex w-8 shrink-0 flex-col items-end gap-1 pt-0.5">
        <span className="text-sm tabular-nums text-fg-subtle">{rank}</span>
        {marker}
      </div>

      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{item.title ?? "Unknown track"}</p>
        <p className="truncate text-sm text-fg-muted">{item.artist ?? "Unknown artist"}</p>
        <div className="mt-3 flex flex-wrap gap-2">
          {item.reasons.map((reason) => (
            <ReasonBadge key={reason.source} reason={reason} />
          ))}
        </div>
      </div>

      {!compact && (
        <div className="w-24 shrink-0 text-right">
          <p className="text-sm tabular-nums text-fg">{item.score.toFixed(3)}</p>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-raised">
            <div className="h-full rounded-full bg-accent" style={{ width: `${width}%` }} />
          </div>
        </div>
      )}
    </li>
  );
}
