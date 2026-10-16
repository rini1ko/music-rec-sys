import { useState } from "react";
import { api } from "../api/client";
import type { Track } from "../api/types";
import { messageOf, useAsync } from "../hooks/useAsync";
import { artistName, trackTitle } from "../lib/format";
import { cardClasses } from "../lib/styles";
import { ErrorBox, Spinner, StatusLine } from "./Feedback";
import { TrackSearch } from "./TrackSearch";

interface Props {
  userId: number;
  onListened: () => void;
}

export function PlayerPanel({ userId, onListened }: Props) {
  const [refresh, setRefresh] = useState(0);
  const [busyTrack, setBusyTrack] = useState<number | null>(null);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const profile = useAsync(() => api.user(userId), `profile-${userId}`, refresh);

  async function listen(track: Track) {
    setBusyTrack(track.track_index);
    setMessage(null);
    try {
      await api.addEvent(userId, track.track_index);
      setMessage({ ok: true, text: `Played ${trackTitle(track.title)} by ${artistName(track.artist)}` });
      setRefresh((value) => value + 1);
      onListened();
    } catch (error) {
      setMessage({ ok: false, text: messageOf(error) });
    } finally {
      setBusyTrack(null);
    }
  }

  if (profile.error) return <ErrorBox message={profile.error} onRetry={profile.reload} />;
  if (!profile.data) return <Spinner label="Loading the listener..." />;

  const user = profile.data;
  return (
    <section aria-label="Player" className={`${cardClasses} space-y-6`}>
      <div>
        <h2 className="text-lg font-semibold">User {user.user_index}</h2>
        <p className="text-sm text-fg-muted">
          {user.country ?? "Country not set"} · {user.history_len.toLocaleString()} plays
        </p>
      </div>
      <TrackSearch busyTrack={busyTrack} onListen={listen} />
      <StatusLine message={message} />
    </section>
  );
}
