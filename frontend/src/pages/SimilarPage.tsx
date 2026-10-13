import { useParams } from "react-router";
import { api } from "../api/client";
import { Empty, ErrorBox, Spinner } from "../components/Feedback";
import { RecommendationCard } from "../components/RecommendationCard";
import { useAsync } from "../hooks/useAsync";
import { artistName, trackTitle } from "../lib/format";
import { parseIndex } from "../lib/paths";
import { cardClasses } from "../lib/styles";

/**
 * Tracks that fit after one track (C5). Every title on the list opens the
 * same page for that track, so the reader walks down a chain; Back walks up.
 */
export function SimilarPage() {
  const trackIndex = parseIndex(useParams().trackId);
  if (typeof trackIndex !== "number") return <Empty title="This is not a track number" />;
  return <Similar key={trackIndex} trackIndex={trackIndex} />;
}

function Similar({ trackIndex }: { trackIndex: number }) {
  const track = useAsync(() => api.track(trackIndex), `track-${trackIndex}`);
  const similar = useAsync(() => api.similar(trackIndex), `similar-${trackIndex}`);

  if (track.error) return <ErrorBox message={track.error} onRetry={track.reload} />;
  const items = similar.data ?? [];
  const maxScore = Math.max(0, ...items.map((item) => item.score));

  return (
    <div className="space-y-6">
      <section className={cardClasses}>
        <h1 className="text-lg font-semibold">{track.data ? trackTitle(track.data.title) : "..."}</h1>
        <p className="text-sm text-fg-muted">{track.data && artistName(track.data.artist)}</p>
        <p className="mt-3 text-sm text-fg-muted">
          Tracks that listeners play right after this one or in the same sessions. Open one to keep going.
        </p>
      </section>
      {similar.error && <ErrorBox message={similar.error} onRetry={similar.reload} />}
      {!similar.data && !similar.error && <Spinner label="Looking for similar tracks..." />}
      {similar.data && items.length === 0 && (
        <Empty title="No similar tracks">Nobody played anything after this track or in its sessions.</Empty>
      )}
      <ol aria-label="Similar tracks" className="space-y-3">
        {items.map((item, index) => (
          <RecommendationCard key={item.track_index} rank={index + 1} item={item} maxScore={maxScore} />
        ))}
      </ol>
    </div>
  );
}
