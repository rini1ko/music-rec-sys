import type { ReactNode } from "react";
import { Link } from "react-router";
import type { Recommendation } from "../api/types";
import { artistName, trackTitle } from "../lib/format";
import { paths } from "../lib/paths";
import { linkClasses } from "../lib/styles";
import { ReasonBadge } from "./ReasonBadge";

interface Props {
  rank: number;
  item: Recommendation;
  /** Highest score in the list, the score bar is relative to it. */
  maxScore: number;
  /** Extra marker under the rank, e.g. how the position changed. */
  marker?: ReactNode;
}

export function RecommendationCard({ rank, item, maxScore, marker }: Props) {
  const width = maxScore > 0 ? Math.round((item.score / maxScore) * 100) : 0;

  return (
    <li className="flex gap-4 rounded-xl border border-line bg-surface/60 p-4">
      <div className="flex w-8 shrink-0 flex-col items-end gap-1 pt-0.5 text-sm tabular-nums text-fg-subtle">
        {rank}
        {marker}
      </div>
      <div className="min-w-0 flex-1">
        <Link
          to={paths.track(item.track_index)}
          title="Tracks that fit after this one"
          className={`${linkClasses} block truncate font-medium`}
        >
          {trackTitle(item.title)}
        </Link>
        <p className="truncate text-sm text-fg-muted">{artistName(item.artist)}</p>
        <div className="mt-3 flex flex-wrap gap-2">
          {item.reasons.map((reason) => (
            <ReasonBadge key={reason.source} reason={reason} />
          ))}
        </div>
      </div>
      <div className="w-20 shrink-0 text-right">
        <p className="text-sm tabular-nums">{item.score.toFixed(3)}</p>
        <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-raised">
          <div className="h-full rounded-full bg-accent" style={{ width: `${width}%` }} />
        </div>
      </div>
    </li>
  );
}
