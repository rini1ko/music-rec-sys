import { api } from "../api/client";
import { useAsync } from "../hooks/useAsync";
import { ColdStartNotice } from "./ColdStartNotice";
import { Empty, ErrorBox, Spinner } from "./Feedback";
import { RecommendationCard } from "./RecommendationCard";

interface Props {
  userId: number;
  /** Change it to load the recommendations again, e.g. after a new play. */
  refresh?: number;
  /** False when the page already shows the Player next to the list. */
  linkToPlayer?: boolean;
}

export function RecommendationList({ userId, refresh = 0, linkToPlayer = true }: Props) {
  const result = useAsync(() => api.recommendations(userId), `recommendations-${userId}`, refresh);

  if (result.error) return <ErrorBox message={result.error} onRetry={result.reload} />;
  if (!result.data) return <Spinner label="Ranking tracks..." />;

  const { user, history_len, items } = result.data;
  const maxScore = Math.max(0, ...items.map((item) => item.score));

  return (
    <section aria-label="Recommendations">
      <div className="mb-4 flex items-baseline justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Recommended for user {user}</h2>
          <p className="text-sm text-fg-muted">{history_len.toLocaleString()} tracks in their history</p>
        </div>
        {result.loading && <span className="text-xs text-fg-subtle">Updating...</span>}
      </div>

      {history_len === 0 && <ColdStartNotice user={user} items={items} showPlayerLink={linkToPlayer} />}

      {items.length === 0 ? (
        <Empty title="Nothing to recommend yet">Train the models again after new plays.</Empty>
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
