import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { api } from "../api/client";
import type { Artist } from "../api/types";
import { ErrorBox } from "../components/Feedback";
import { Card } from "../components/ui/Card";
import { EmptyState } from "../components/ui/EmptyState";
import { SkeletonList } from "../components/ui/Skeleton";
import { inputClasses, labelClasses, linkClasses } from "../components/ui/styles";
import { useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { useDocumentTitle } from "../hooks/useDocumentTitle";
import { artistName, whole } from "../lib/format";
import { paths } from "../lib/paths";

const MIN_LENGTH = 2;

export function ArtistsPage() {
  useDocumentTitle("Artists");
  const [search, setSearch] = useSearchParams();
  // typing goes to state first: the router updates the URL in a transition
  const [text, setText] = useState(() => search.get("q") ?? "");
  const query = useDebounced(text.trim(), 300);
  const active = query.length >= MIN_LENGTH;

  const results = useAsync<Artist[]>(
    () => (active ? api.searchArtists(query, 30) : Promise.resolve([])),
    `artists-${active ? query : ""}`,
  );

  function change(next: string) {
    setText(next);
    setSearch(next.trim() === "" ? {} : { q: next }, { replace: true });
  }

  return (
    <Card title="Artists" description="Find an artist to see their most played tracks.">
      <label htmlFor="artist-search" className={labelClasses}>
        Artist name
      </label>
      <input
        id="artist-search"
        type="search"
        value={text}
        onChange={(event) => change(event.target.value)}
        placeholder="For example, Radiohead"
        className={`${inputClasses} mt-2`}
      />

      <div className="mt-4">
        {!active && <p className="text-sm text-fg-subtle">Type at least {MIN_LENGTH} characters.</p>}
        {active && results.error && <ErrorBox message={results.error} onRetry={results.reload} />}
        {active && !results.error && !results.data && (
          <SkeletonList rows={4} label="Searching..." rowClassName="h-12" />
        )}
        {active && results.data && results.data.length === 0 && (
          <EmptyState title={`No artists match "${query}"`} />
        )}
        {active && results.data && results.data.length > 0 && (
          <ul className="divide-y divide-line rounded-lg border border-line">
            {results.data.map((artist) => (
              <li key={artist.artist_index} className="flex items-center justify-between gap-3 px-3 py-2.5">
                <Link to={paths.artist(artist.artist_index)} className={`${linkClasses} min-w-0 truncate`}>
                  {artistName(artist.name)}
                </Link>
                <span className="shrink-0 text-xs tabular-nums text-fg-subtle">
                  {whole(artist.plays)} plays · {whole(artist.tracks)} tracks
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
