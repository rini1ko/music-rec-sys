import { useState } from "react";
import { api } from "../api/client";
import type { Track, UserProfile } from "../api/types";
import { messageOf, useAsync } from "../hooks/useAsync";
import { ErrorBox, Spinner } from "./Feedback";
import { TrackSearch } from "./TrackSearch";

interface Props {
  userId: number | null;
  onListened: () => void;
}

interface Message {
  kind: "ok" | "error";
  text: string;
}

function describe(profile: UserProfile): string {
  return [profile.country, profile.age !== null ? `${profile.age} y.o.` : null, profile.gender]
    .filter((part): part is string => part !== null)
    .join(" · ");
}

export function PlayerPanel({ userId, onListened }: Props) {
  const [refresh, setRefresh] = useState(0);
  const [busyTrack, setBusyTrack] = useState<number | null>(null);
  const [message, setMessage] = useState<Message | null>(null);

  const profile = useAsync<UserProfile | null>(
    () => (userId === null ? Promise.resolve(null) : api.user(userId)),
    `profile-${userId}`,
    refresh,
  );

  async function listen(track: Track) {
    if (userId === null) return;
    setBusyTrack(track.track_index);
    setMessage(null);
    try {
      await api.addEvent({ user: userId, track: track.track_index });
      setMessage({ kind: "ok", text: `Saved: ${track.title ?? "track"} by ${track.artist ?? "unknown"}` });
      setRefresh((value) => value + 1);
      onListened();
    } catch (error) {
      setMessage({ kind: "error", text: messageOf(error) });
    } finally {
      setBusyTrack(null);
    }
  }

  if (userId === null) {
    return <p className="text-slate-400">Pick a listener, then play a track for them.</p>;
  }
  if (profile.error) {
    return <ErrorBox message={profile.error} onRetry={profile.reload} />;
  }
  if (!profile.data) {
    return <Spinner label="Loading the listener..." />;
  }

  const user = profile.data;
  return (
    <section aria-label="Player" className="space-y-6 rounded-xl border border-slate-800 p-5">
      <div>
        <h2 className="text-lg font-semibold">User {user.user_index}</h2>
        <p className="text-sm text-slate-400">
          {describe(user) || "No profile details"} · {user.history_len.toLocaleString()} plays
        </p>
      </div>

      <div>
        <h3 className="mb-2 text-xs font-medium uppercase tracking-wider text-slate-400">
          Recently played
        </h3>
        {user.recent.length === 0 ? (
          <p className="text-sm text-slate-500">Nothing yet.</p>
        ) : (
          <ol className="space-y-1 text-sm">
            {user.recent.slice(0, 5).map((track, index) => (
              <li key={`${track.track_index}-${index}`} className="truncate text-slate-300">
                {track.title ?? "Unknown track"}
                <span className="text-slate-500"> · {track.artist ?? "Unknown artist"}</span>
              </li>
            ))}
          </ol>
        )}
      </div>

      <TrackSearch busyTrack={busyTrack} onListen={listen} />

      {message && (
        <p
          role="status"
          className={`text-sm ${message.kind === "ok" ? "text-emerald-300" : "text-rose-300"}`}
        >
          {message.text}
        </p>
      )}
    </section>
  );
}