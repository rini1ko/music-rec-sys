import { api } from "../api/client";
import type { RecommendationsResponse } from "../api/types";
import { useAsync } from "../hooks/useAsync";
import { ErrorBox, Spinner } from "./Feedback";
import { RecommendationCard } from "./RecommendationCard";

export function RecommendationList({ userId }: { userId: number | null }) {
  const result = useAsync<RecommendationsResponse | null>(
    () => (userId === null ? Promise.resolve(null) : api.recommendations(userId, 10)),
    `recommendations-${userId}`,
  );

  if (userId === null) {
    return <p className="text-slate-400">Pick a listener to see what they should hear next.</p>;
  }
  if (result.error) {
    return <ErrorBox message={result.error} onRetry={result.reload} />;
  }
  if (!result.data) {
    return <Spinner label="Ranking tracks..." />;
  }

  const { user, history_len, items } = result.data;
  const maxScore = Math.max(0, ...items.map((item) => item.score));

  return (
    <section aria-label="Recommendations">
      <div className="mb-4">
        <h2 className="text-lg font-semibold">For user {user}</h2>
        <p className="text-sm text-slate-400">
          {history_len.toLocaleString()} tracks in their history
        </p>
      </div>

      {items.length === 0 ? (
        <p className="text-slate-400">Nothing to recommend yet.</p>
      ) : (
        <ol className="space-y-3">
          {items.map((item, index) => (
            <RecommendationCard key={item.track_index} rank={index + 1} item={item} maxScore={maxScore} />
          ))}
        </ol>
      )}
    </section>
  );
}