import type { Recommendation } from "../api/types";
import { ReasonBadge } from "./ReasonBadge";

interface Props {
  rank: number;
  item: Recommendation;
  maxScore: number;
}

export function RecommendationCard({ rank, item, maxScore }: Props) {
  const width = maxScore > 0 ? Math.round((item.score / maxScore) * 100) : 0;

  return (
    <li className="flex gap-4 rounded-xl border border-slate-800 bg-slate-900/60 p-4">
      <span className="w-6 pt-0.5 text-right text-sm tabular-nums text-slate-500">{rank}</span>

      <div className="min-w-0 flex-1">
        <p className="truncate font-medium">{item.title ?? "Unknown track"}</p>
        <p className="truncate text-sm text-slate-400">{item.artist ?? "Unknown artist"}</p>
        <div className="mt-3 flex flex-wrap gap-2">
          {item.reasons.map((reason) => (
            <ReasonBadge key={reason.source} reason={reason} />
          ))}
        </div>
      </div>

      <div className="w-24 shrink-0 text-right">
        <p className="text-sm tabular-nums text-slate-300">{item.score.toFixed(3)}</p>
        <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-800">
          <div className="h-full rounded-full bg-emerald-400" style={{ width: `${width}%` }} />
        </div>
      </div>
    </li>
  );
}