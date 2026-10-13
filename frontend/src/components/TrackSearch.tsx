import { useState } from "react";
import { api } from "../api/client";
import type { Track } from "../api/types";
import { useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { artistName, trackTitle } from "../lib/format";
import { buttonClasses, inputClasses, labelClasses } from "../lib/styles";
import { ErrorBox } from "./Feedback";

interface Props {
  busyTrack: number | null;
  onListen: (track: Track) => void;
}

export function TrackSearch({ busyTrack, onListen }: Props) {
  const [text, setText] = useState("");
  const query = useDebounced(text.trim(), 300);
  const active = query.length >= 2;
  const results = useAsync(() => (active ? api.searchTracks(query) : Promise.resolve([])), `search-${active ? query : ""}`);
  const hint = !active
    ? "Type at least 2 characters."
    : !results.data
      ? "Searching..."
      : results.data.length === 0
        ? `No tracks match "${query}".`
        : null;

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
      {results.error && <ErrorBox message={results.error} onRetry={results.reload} />}
      {hint && !results.error && <p className="mt-3 text-sm text-fg-subtle">{hint}</p>}
      {active && results.data && results.data.length > 0 && (
        <ul className="mt-3 divide-y divide-line rounded-lg border border-line">
          {results.data.map((track) => (
            <li key={track.track_index} className="flex items-center justify-between gap-3 px-3 py-2">
              <div className="min-w-0">
                <p className="truncate text-sm">{trackTitle(track.title)}</p>
                <p className="truncate text-xs text-fg-subtle">{artistName(track.artist)}</p>
              </div>
              <button
                type="button"
                disabled={busyTrack !== null}
                onClick={() => onListen(track)}
                aria-label={`Listen to ${trackTitle(track.title)}`}
                className={buttonClasses.small}
              >
                {busyTrack === track.track_index ? "Saving..." : "Listen"}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
