import { Link, useParams } from "react-router";
import { api } from "../api/client";
import { BarList } from "../components/charts/BarList";
import { ChartCard } from "../components/charts/ChartCard";
import { ErrorBox } from "../components/Feedback";
import { EmptyState } from "../components/ui/EmptyState";
import { SkeletonList } from "../components/ui/Skeleton";
import { StatTile } from "../components/ui/StatTile";
import { accentLinkClasses, linkClasses } from "../components/ui/styles";
import { useAsync } from "../hooks/useAsync";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { artistName, trackTitle } from "../lib/format";
import { paths, parseIndex } from "../lib/paths";

const TOP_TRACKS = 20;

export function ArtistPage() {
  const artistIndex = parseIndex(useParams().artistId);
  if (typeof artistIndex !== "number") {
    return (
      <EmptyState
        title="This is not an artist number"
        action={
          <Link to={paths.artists} className={accentLinkClasses}>
            Search for an artist
          </Link>
        }
      />
    );
  }
  return <Artist key={artistIndex} artistIndex={artistIndex} />;
}

function Artist({ artistIndex }: { artistIndex: number }) {
  const artist = useAsync(() => api.artist(artistIndex, TOP_TRACKS), `artist-${artistIndex}`);
  useDocumentTitle(artist.data ? artistName(artist.data.name) : "Artist");

  if (artist.error) {
    return <ErrorBox message={artist.error} onRetry={artist.reload} />;
  }
  if (!artist.data) {
    return <SkeletonList rows={3} label="Loading the artist..." rowClassName="h-24" />;
  }

  const profile = artist.data;
  return (
    <div className="space-y-6">
      <div>
        <Link to={paths.artists} className={`${accentLinkClasses} text-sm`}>
          ← Artists
        </Link>
        <h1 className="mt-2 text-xl font-semibold">{artistName(profile.name)}</h1>
      </div>

      <dl className="grid gap-4 sm:grid-cols-3">
        <StatTile label="Plays" value={profile.plays} />
        <StatTile label="Listeners" value={profile.listeners} />
        <StatTile label="Tracks" value={profile.tracks} />
      </dl>

      <ChartCard
        title="Most played tracks"
        description="Open a track to see what fits after it."
        table={{
          columns: ["Track", "Plays"],
          rows: profile.top_tracks.map((track) => [trackTitle(track.title), track.plays]),
        }}
      >
        <BarList
          label="Tracks of the artist by plays"
          unit="plays"
          items={profile.top_tracks.map((track) => ({
            key: track.track_index,
            title: trackTitle(track.title),
            label: (
              <Link to={paths.track(track.track_index)} className={linkClasses}>
                {trackTitle(track.title)}
              </Link>
            ),
            value: track.plays,
          }))}
        />
      </ChartCard>
    </div>
  );
}
