import type { Comparison, Recommendation } from "../../api/types";
import { labelClasses } from "../../lib/styles";
import { RecommendationCard } from "../RecommendationCard";
import { MovementBadge } from "./MovementBadge";

const maxScore = (items: Recommendation[]) => Math.max(0, ...items.map((item) => item.score));

/** The default list and the list with the reader's weights, side by side. */
export function ComparisonView({ comparison }: { comparison: Comparison }) {
  const { base, custom, up, down, added, gone } = comparison;
  return (
    <div className="space-y-4">
      <p role="status" className="text-sm text-fg-muted">
        {up + down + added + gone === 0
          ? "Your weights give the same list in the same order."
          : `${up} up, ${down} down, ${added} new, ${gone} gone.`}
      </p>
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="min-w-0">
          <h3 className={`${labelClasses} mb-3`}>Default weights</h3>
          <ol className="space-y-3">
            {base.map((item, index) => (
              <RecommendationCard key={item.track_index} rank={index + 1} item={item} maxScore={maxScore(base)} />
            ))}
          </ol>
        </div>
        <div className="min-w-0">
          <h3 className={`${labelClasses} mb-3`}>Your weights</h3>
          {custom.length === 0 && <p className="text-sm text-fg-subtle">No tracks. Turn at least one model on.</p>}
          <ol className="space-y-3">
            {custom.map((item, index) => (
              <RecommendationCard
                key={item.track_index}
                rank={index + 1}
                item={item}
                maxScore={maxScore(custom)}
                marker={<MovementBadge movement={item.movement} />}
              />
            ))}
          </ol>
        </div>
      </div>
    </div>
  );
}
