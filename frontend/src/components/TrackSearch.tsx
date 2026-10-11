import { useState } from "react";
import { api } from "../api/client";
import type { Track } from "../api/types";
import { useAsync } from "../hooks/useAsync";
import { useDebounced } from "../hooks/useDebounced";
import { ErrorBox, Spinner } from "./Feedback";

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
      <label htmlFor="track-search" className="text-xs font-medium uppercase tracking-wider text-slate-400">
        Find a track to play
      </label>
      <input
        id="track-search"
        type="search"
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder="Title or artist"
        className="mt-2 w-full rounded-lg border border-slate-800 bg-slate-900 px-3 py-2 text-sm placeholder:text-slate-600 focus:border-emerald-500 focus:outline-none"
      />

      <div className="mt-3">
        {!active && <p className="text-sm text-slate-500">Type at least {MIN_LENGTH} characters.</p>}
        {active && results.error && <ErrorBox message={results.error} onRetry={results.reload} />}
        {active && !results.error && !results.data && <Spinner label="Searching..." />}
        {active && results.data && results.data.length === 0 && (
          <p className="text-sm text-slate-500">No tracks match "{query}".</p>
        )}
        {active && results.data && results.data.length > 0 && (
          <ul className="divide-y divide-slate-800 rounded-lg border border-slate-800">
            {results.data.map((track) => (
              <li key={track.track_index} className="flex items-center justify-between gap-3 px-3 py-2">
                <div className="min-w-0">
                  <p className="truncate text-sm">{track.title ?? "Unknown track"}</p>
                  <p className="truncate text-xs text-slate-500">{track.artist ?? "Unknown artist"}</p>
                </div>
                <button
                  type="button"
                  disabled={busyTrack !== null}
                  onClick={() => onListen(track)}
                  className="shrink-0 rounded-md bg-emerald-500 px-3 py-1 text-xs font-medium text-slate-950 hover:bg-emerald-400 disabled:opacity-50"
                >
                  {busyTrack === track.track_index ? "Saving..." : "Listen"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}