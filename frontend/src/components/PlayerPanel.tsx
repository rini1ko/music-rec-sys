import { useState } from "react";
import { api } from "../api/client";
import type { Track, UserProfile } from "../api/types";
import { messageOf, useAsync } from "../hooks/useAsync";
import { useToast } from "../hooks/useToast";
import { ErrorBox } from "./Feedback";
import { OnboardingProgress } from "./OnboardingProgress";
import { TrackSearch } from "./TrackSearch";
import { Card } from "./ui/Card";
import { Skeleton } from "./ui/Skeleton";
import { labelClasses } from "./ui/styles";

interface Props {
  userId: number;
  onListened: () => void;
}

function describe(profile: UserProfile): string {
  return [profile.country, profile.age !== null ? `${profile.age} y.o.` : null, profile.gender]
    .filter((part): part is string => part !== null)
    .join(" · ");
}

export function PlayerPanel({ userId, onListened }: Props) {
  const toast = useToast();
  const [refresh, setRefresh] = useState(0);
  const [busyTrack, setBusyTrack] = useState<number | null>(null);

  const profile = useAsync(() => api.user(userId), `profile-${userId}`, refresh);

  async function listen(track: Track) {
    setBusyTrack(track.track_index);
    try {
      await api.addEvent({ user: userId, track: track.track_index });
      toast.show("success", `Played ${track.title ?? "a track"} by ${track.artist ?? "unknown artist"}`);
      setRefresh((value) => value + 1);
      onListened();
    } catch (error) {
      toast.show("error", messageOf(error));
    } finally {
      setBusyTrack(null);
    }
  }

  if (profile.error) {
    return <ErrorBox message={profile.error} onRetry={profile.reload} />;
  }
  if (!profile.data) {
    return (
      <div role="status" aria-label="Loading the listener..." className="space-y-3">
        <Skeleton className="h-6 w-40" />
        <Skeleton className="h-4 w-64" />
        <Skeleton className="h-24 w-full" />
      </div>
    );
  }

  const user = profile.data;
  return (
    <Card
      title={`User ${user.user_index}`}
      description={`${describe(user) || "No profile details"} · ${user.history_len.toLocaleString()} plays`}
    >
      <div className="space-y-6">
        <OnboardingProgress plays={user.history_len} />

        <div>
          <h3 className={`${labelClasses} mb-2`}>Recently played</h3>
          {user.recent.length === 0 ? (
            <p className="text-sm text-fg-subtle">Nothing yet.</p>
          ) : (
            <ol className="space-y-1 text-sm">
              {user.recent.slice(0, 5).map((track, index) => (
                <li key={`${track.track_index}-${index}`} className="truncate">
                  {track.title ?? "Unknown track"}
                  <span className="text-fg-subtle"> · {track.artist ?? "Unknown artist"}</span>
                </li>
              ))}
            </ol>
          )}
        </div>

        <TrackSearch busyTrack={busyTrack} onListen={listen} />
      </div>
    </Card>
  );
}
