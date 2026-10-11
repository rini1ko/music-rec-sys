import type { ReactNode } from "react";
import { Link } from "react-router";
import type { Recommendation } from "../api/types";
import { artistName, trackTitle } from "../lib/format";
import { paths } from "../lib/paths";
import { ReasonBadge } from "./ReasonBadge";
import { linkClasses } from "./ui/styles";

interface Props {
  rank: number;
  item: Recommendation;
  maxScore: number;
  /** Extra marker next to the rank, e.g. how the position changed. */
  marker?: ReactNode;
  /** A smaller card without the score bar, for side-by-side lists. */
  compact?: boolean;
  /** Where the title leads. By default, to the tracks similar to this one. */
  trackHref?: string;
  /** Buttons on the right, e.g. Listen. */
  actions?: ReactNode;
}

export function RecommendationCard({
  rank,
  item,
  maxScore,
  marker,
  compact = false,
  trackHref,
  actions,
}: Props) {
  const width = maxScore > 0 ? Math.round((item.score / maxScore) * 100) : 0;

  return (
    <li className="flex gap-4 rounded-xl border border-line bg-surface/60 p-4">
      <div className="flex w-8 shrink-0 flex-col items-end gap-1 pt-0.5">
        <span className="text-sm tabular-nums text-fg-subtle">{rank}</span>
        {marker}
      </div>

      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">
          <Link
            to={trackHref ?? paths.track(item.track_index)}
            title="Tracks that fit after this one"
            className={linkClasses}
          >
            {trackTitle(item.title)}
          </Link>
        </p>
        <p className="truncate text-sm text-fg-muted">
          {item.artist_index === null ? (
            artistName(item.artist)
          ) : (
            <Link to={paths.artist(item.artist_index)} className={linkClasses}>
              {artistName(item.artist)}
            </Link>
          )}
        </p>
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
      {actions !== undefined && <div className="flex shrink-0 items-center">{actions}</div>}
    </li>
  );
}
