import { Link, useParams, useSearchParams } from "react-router";
import { api } from "../api/client";
import type { Recommendation, Track } from "../api/types";
import { ErrorBox } from "../components/Feedback";
import { RecommendationCard } from "../components/RecommendationCard";
import { Card } from "../components/ui/Card";
import { EmptyState } from "../components/ui/EmptyState";
import { Skeleton, SkeletonList } from "../components/ui/Skeleton";
import { accentLinkClasses, linkClasses } from "../components/ui/styles";
import { useAsync } from "../hooks/useAsync";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { artistName, trackTitle } from "../lib/format";
import { parseIndex, parseIndexList, paths } from "../lib/paths";

/** Older steps fall off the trail, so the URL stays short. */
const MAX_TRAIL = 8;
const TOP = 10;

export function SimilarPage() {
  const trackIndex = parseIndex(useParams().trackId);
  const [search] = useSearchParams();
  const trail = parseIndexList(search.get("trail")).slice(-MAX_TRAIL);

  if (typeof trackIndex !== "number") {
    return (
      <EmptyState
        title="This is not a track number"
        action={
          <Link to={paths.dashboard} className={accentLinkClasses}>
            Pick a track on the dashboard
          </Link>
        }
      />
    );
  }
  return <Similar key={trackIndex} trackIndex={trackIndex} trail={trail} />;
}

function Similar({ trackIndex, trail }: { trackIndex: number; trail: number[] }) {
  const track = useAsync(() => api.track(trackIndex), `track-${trackIndex}`);
  const similar = useAsync(() => api.similar(trackIndex, TOP), `similar-${trackIndex}`);
  useDocumentTitle(track.data ? `After ${trackTitle(track.data.title)}` : "Similar tracks");

  // a click on a similar track moves one step further down the chain
  const nextTrail = [...trail, trackIndex].slice(-MAX_TRAIL);

  if (track.error) {
    return <ErrorBox message={track.error} onRetry={track.reload} />;
  }

  return (
    <div className="space-y-6">
      {trail.length > 0 && <Trail trail={trail} />}

      <Card
        title={track.data ? trackTitle(track.data.title) : <Skeleton className="h-6 w-56" />}
        description={track.data ? <ArtistLink track={track.data} /> : undefined}
        actions={
          trail.length > 0 ? (
            <Link to={paths.track(trackIndex)} className={`${accentLinkClasses} text-sm`}>
              Start the chain here
            </Link>
          ) : undefined
        }
      >
        <p className="text-sm text-fg-muted">
          Tracks that listeners play right after this one or in the same sessions. Open one to
          keep going down the chain.
        </p>
      </Card>

      {similar.error && <ErrorBox message={similar.error} onRetry={similar.reload} />}
      {!similar.data && !similar.error && <SkeletonList rows={5} label="Looking for similar tracks..." />}
      {similar.data && similar.data.length === 0 && (
        <EmptyState
          title="No similar tracks"
          description="Nobody played anything right after this track or in the same sessions."
        />
      )}
      {similar.data && similar.data.length > 0 && (
        <SimilarList items={similar.data} nextTrail={nextTrail} />
      )}
    </div>
  );
}

function SimilarList({ items, nextTrail }: { items: Recommendation[]; nextTrail: number[] }) {
  const maxScore = Math.max(0, ...items.map((item) => item.score));
  return (
    <ol className="space-y-3" aria-label="Similar tracks">
      {items.map((item, index) => (
        <RecommendationCard
          key={item.track_index}
          rank={index + 1}
          item={item}
          maxScore={maxScore}
          trackHref={paths.track(item.track_index, nextTrail)}
        />
      ))}
    </ol>
  );
}

function ArtistLink({ track }: { track: Track }) {
  return (
    <Link to={paths.artist(track.artist_index)} className={linkClasses}>
      {artistName(track.artist)}
    </Link>
  );
}

/** The tracks the reader came through. Each step opens the chain at that point. */
function Trail({ trail }: { trail: number[] }) {
  return (
    <nav aria-label="Chain of tracks">
      <ol className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-fg-muted">
        {trail.map((index, position) => (
          <li key={`${index}-${position}`} className="flex items-center gap-2">
            <TrailStep trackIndex={index} href={paths.track(index, trail.slice(0, position))} />
            <span aria-hidden="true" className="text-fg-subtle">
              →
            </span>
          </li>
        ))}
        <li aria-current="page" className="text-fg">
          this track
        </li>
      </ol>
    </nav>
  );
}

function TrailStep({ trackIndex, href }: { trackIndex: number; href: string }) {
  const track = useAsync(() => api.track(trackIndex), `track-${trackIndex}`);
  return (
    <Link to={href} className={`${linkClasses} max-w-48 truncate`}>
      {track.data ? trackTitle(track.data.title) : `#${trackIndex}`}
    </Link>
  );
}
