import { useState } from "react";
import { api } from "../api/client";
import type { Track } from "../api/types";
import { useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { ErrorBox } from "./Feedback";
import { Button } from "./ui/Button";
import { SkeletonList } from "./ui/Skeleton";
import { inputClasses, labelClasses } from "./ui/styles";

const MIN_LENGTH = 2;

interface Props {
  busyTrack: number | null;
  onListen: (track: Track) => void;
}

export function TrackSearch({ busyTrack, onListen }: Props) {
  const [text, setText] = useState("");
  const query = useDebounced(text.trim(), 300);
  const active = query.length >= MIN_LENGTH;

  const results = useAsync<Track[]>(
    () => (active ? api.searchTracks(query, 10) : Promise.resolve([])),
    `search-${active ? query : ""}`,
  );

  return (
    <div>
      <label htmlFor="track-search" className={labelClasses}>
        Find a track to play
      </label>
      <input
        id="track-search"
        type="search"
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder="Title or artist"
        className={`${inputClasses} mt-2`}
      />

      <div className="mt-3">
        {!active && <p className="text-sm text-fg-subtle">Type at least {MIN_LENGTH} characters.</p>}
        {active && results.error && <ErrorBox message={results.error} onRetry={results.reload} />}
        {active && !results.error && !results.data && (
          <SkeletonList rows={3} label="Searching..." rowClassName="h-12" />
        )}
        {active && results.data && results.data.length === 0 && (
          <p className="text-sm text-fg-subtle">No tracks match "{query}".</p>
        )}
        {active && results.data && results.data.length > 0 && (
          <ul className="divide-y divide-line rounded-lg border border-line">
            {results.data.map((track) => (
              <li key={track.track_index} className="flex items-center justify-between gap-3 px-3 py-2">
                <div className="min-w-0">
                  <p className="truncate text-sm">{track.title ?? "Unknown track"}</p>
                  <p className="truncate text-xs text-fg-subtle">{track.artist ?? "Unknown artist"}</p>
                </div>
                <Button
                  variant="primary"
                  size="sm"
                  disabled={busyTrack !== null}
                  busy={busyTrack === track.track_index}
                  onClick={() => onListen(track)}
                  aria-label={`Listen to ${track.title ?? "this track"}`}
                >
                  {busyTrack === track.track_index ? "Saving..." : "Listen"}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
